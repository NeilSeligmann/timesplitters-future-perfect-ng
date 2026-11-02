//! Recording toolkit: a `GuestCall` that answers the original page toolkit entry points
//! (docs/t-ui-data-model.md, docs/t-ui-model.md "UI-6 slice 1") and feeds a `PageBuilder`.
//! Everything outside the toolkit (text lookup, player count, unlock predicates) is a fixture
//! (`Scenario`). Calls that only change layout, scroll or draw state are counted, not modelled.

use crate::image::XbeImage;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall};
use tsfp_ports::c2r;
use tsfp_ports::memory::GuestMemory;
use tsfp_ui_model::{
    BuilderError, HandlerRef, HandlerSlot, PageBuilder, PageId, TextId, TextRef, UiOption, Widget,
    WidgetId, WidgetKind,
};

/// Sparse guest memory shared between the builder and the toolkit (clones share storage).
#[derive(Default, Clone)]
pub struct Ram {
    bytes: Rc<RefCell<BTreeMap<u32, u8>>>,
    /// Static data of the owner's image behind the written bytes (None: purely synthetic).
    image: Option<std::sync::Arc<XbeImage>>,
    /// Dword addresses read while no byte of them was ever written (the fixture misses them).
    missed: Rc<RefCell<BTreeMap<u32, u32>>>,
}
impl Ram {
    /// Distinct unwritten dwords that the builder read, with read counts (a coverage measure).
    pub fn missed(&self) -> BTreeMap<u32, u32> {
        self.missed.borrow().clone()
    }
}
impl GuestMemory for Ram {
    fn read8(&self, a: u32) -> u8 {
        if let Some(v) = self.bytes.borrow().get(&a) {
            return *v;
        }
        if let Some(v) = self.image.as_ref().and_then(|i| i.byte(a)) {
            return v;
        }
        *self.missed.borrow_mut().entry(a & !3).or_default() += 1;
        0
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.bytes.borrow_mut().insert(a, v);
    }
}

pub fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: 0x7fff0,
        ebx: 0,
        ebp: 0,
        esi: 0,
        edi: 0,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0x27f,
        },
        xmm: Default::default(),
        xmm_hi: Default::default(),
    }
}

/// Synthetic text pointers: `TEXT_BASE + id * 8`. Any other non-zero pointer is runtime text.
pub const TEXT_BASE: u32 = 0x4000_0000;
const TEXT_LIMIT: u32 = TEXT_BASE + 0x10000 * 8;

/// Toolkit entry points (original virtual addresses).
pub mod va {
    pub const TEXT_BY_ID: u32 = 0x3d7b0;
    pub const BEGIN_BUILD: u32 = 0x75b70;
    pub const ADD_TEXT_ITEM: u32 = 0x79d60;
    pub const ADD_DIVIDER_BY_INDEX: u32 = 0x79710;
    pub const ADD_DIVIDER: u32 = 0x78540;
    pub const ADD_OPTION: u32 = 0x717d0;
    pub const ADD_ON_OFF_OPTIONS: u32 = 0x71830;
    pub const SELECT_ROW_BY_ID: u32 = 0x77110;
    pub const CONTROLS_ROWS: u32 = 0x2ea3f0;
    /// Ported row-adding helpers a builder calls (run recursively against the same toolkit).
    pub const ROW_HELPERS: &[u32] = &[0x2ea3f0, 0x1cc080, 0x2c9f00, 0x2ca650, 0x2c78f0];
    /// Typed items of the toolkit other than text and divider: (address, node type).
    pub const TYPED_ITEMS: &[(u32, u32)] = &[
        (0x793e0, 8),
        (0x79460, 5),
        (0x795b0, 10),
        (0x795f0, 10),
        (0x79640, 10),
    ];
    /// Predicate or lookup helpers answered by the scenario (default 0).
    pub const PREDICATES: &[u32] = &[
        0x1cd890, 0x1cd8f0, 0x1cd560, 0x1cd120, 0x35e960, 0x2cb040, 0x2e7eb0, 0x2e7ef0, 0x1cdae0,
        0x1cd900, 0x2e4990, 0x32030, 0x1cd980, 0x1cda20, 0x22df10, 0x22d790,
    ];
    pub const PLAYER_COUNT: u32 = 0x1cdae0;
    /// Layout, scroll, hint, style and draw state: no model content.
    pub const IGNORED: &[u32] = &[
        0x71630, 0x71730, 0x71c70, 0x71b10, 0x71c50, 0x771d0, 0x75e70, 0x71be0, 0x71e00, 0x71e10,
        0x78050, 0x781a0, 0x7d480, 0x2e4620, 0x2e45a0, 0x77140, 0x74a00, 0x71940, 0x2c3300,
        0x2e9a80, 0x780e0, 0x770e0, 0x71e20, 0x75400, 0x77190, 0x71740, 0x19d70, 0x22d7c0, 0x96410,
        0x389b0, 0x1cc5e0, 0x80af0, 0x803c0, 0x80300, 0x80370, 0xa6f90, 0x2c4310, 0x2be460,
        0x69a60, 0x1cd170, 0x77250,
    ];
    /// Scroll list begin: from here on every text item after the first is preceded by a divider.
    pub const LIST_BEGIN: &[u32] = &[0x780e0, 0x78050];
    pub const LIST_END: u32 = 0x781a0;
}

/// Fixture state around the builder: guest words and answers of predicate helpers.
#[derive(Clone, Debug, Default)]
pub struct Scenario {
    pub words: Vec<(u32, u32)>,
    /// Viewport field of the synthetic root widget (0..4).
    pub page_slot: u32,
    pub returns: BTreeMap<u32, u32>,
    /// Static image data read behind the synthetic memory (private, optional).
    pub image: Option<std::sync::Arc<XbeImage>>,
}
impl Scenario {
    /// One local player, no unlocks, language 0, cursor remembered on no item.
    pub fn single_player() -> Self {
        Self::players(1)
    }
    /// `count` local players (1 to 4), no unlocks.
    pub fn players(count: u32) -> Self {
        let mut s = Scenario::default();
        s.words.push((0x5236c4, count));
        s.words.push((0x761400, 0xffff_ffff));
        // Widget heap capacity (node count) of the toolkit: a divisor of the context stack push.
        s.words.push((0x6fe920, 0xc8));
        s.returns.insert(va::PLAYER_COUNT, count);
        s
    }
    /// Every unlock predicate answers true (FABRICATED fixture state, not a measured save).
    pub fn unlocked(mut self) -> Self {
        for &p in va::PREDICATES {
            if p != va::PLAYER_COUNT {
                self.returns.insert(p, 1);
            }
        }
        self
    }
    /// Waiting-panel branch of 0x193BF0. The state-to-branch mapping is deliberately
    /// FABRICATED; only this known branch body is being exercised without the image.
    pub fn save_panel_waiting(count: u32, slot: u32) -> Self {
        assert!((1..=4).contains(&count) && slot < count);
        let mut s = Self::players(count);
        s.page_slot = slot;
        s.words.extend([
            (0x74e934, count),
            (0x74f9ec + slot * 4, 1),
            (0x194940, 0x193c4f),
            (0x7ba9e0, slot),
        ]);
        s
    }
    /// Runtime dialog globals, supplied by its opener (FABRICATED fixture values).
    pub fn confirmation(mut self, first: u32, second: Option<u32>, yes: u32, no: u32) -> Self {
        self.words.extend([
            (0x762a18, first),
            (0x762a1c, second.unwrap_or(0)),
            (0x761c5c, yes),
            (0x761c60, no),
        ]);
        self
    }
    /// Setup rows in the 0x20-byte table. Only caller-supplied rows are populated;
    /// these are synthetic descriptors, never copied original data.
    pub fn setup_options(mut self, mode: u32, rows: &[(u32, u16)]) -> Self {
        self.words.push((0x78ad2c, mode));
        for &(index, text) in rows {
            assert!((2..0x16).contains(&index));
            self.words.extend([
                (0x4d1f78 + index * 0x20, 1),
                (0x4d1f80 + index * 0x20, 0),
                (0x4d1f90 + index * 0x20, u32::from(text)),
            ]);
        }
        self
    }
    pub fn with_image(mut self, image: Option<std::sync::Arc<XbeImage>>) -> Self {
        self.image = image;
        self
    }
    /// The fixture states every page is tried under, with report labels.
    pub fn all() -> Vec<(&'static str, Scenario)> {
        Self::all_with(None)
    }
    /// As `all`, with the static image mapped behind the synthetic memory.
    pub fn all_with(image: Option<std::sync::Arc<XbeImage>>) -> Vec<(&'static str, Scenario)> {
        let mut v = vec![
            ("p1", Self::players(1)),
            ("p2", Self::players(2)),
            ("p3", Self::players(3)),
            ("p4", Self::players(4)),
            ("p1-unlocked", Self::players(1).unlocked()),
            ("p2-unlocked", Self::players(2).unlocked()),
            ("p4-unlocked", Self::players(4).unlocked()),
        ];
        for (_, s) in &mut v {
            s.image = image.clone();
        }
        for (label, count) in [
            ("save-waiting-p1", 1),
            ("save-waiting-p2", 2),
            ("save-waiting-p3", 3),
            ("save-waiting-p4", 4),
        ] {
            v.push((label, Self::save_panel_waiting(count, 0)));
        }
        let mut save_active = Self::single_player();
        save_active.words.extend([(0x74fa0c, 1), (0x74e940, 4)]);
        v.push(("save-active-empty", save_active));
        for (label, mode) in [
            ("setup-offline", 0),
            ("setup-mode11", 11),
            ("setup-mode12", 12),
            ("setup-mode13", 13),
        ] {
            v.push((
                label,
                Self::single_player()
                    .unlocked()
                    .setup_options(mode, &[(3, 901), (4, 902)]),
            ));
        }
        v
    }
}

pub fn text_ref(pointer: u32) -> TextRef {
    match pointer {
        0 => TextRef::None,
        p if (TEXT_BASE..TEXT_LIMIT).contains(&p) => TextRef::Id(TextId((p - TEXT_BASE) / 8)),
        _ => TextRef::Dynamic,
    }
}

pub struct Toolkit<'a, B: PageBuilder> {
    pub rec: &'a mut B,
    pub mem: Ram,
    pub page: PageId,
    pub scenario: Scenario,
    pub rows: usize,
    /// The newest row, recorded when the next row, option or page call needs it.
    pub pending: Option<(Widget, Option<u32>)>,
    /// Calls made so far (runaway guard).
    pub calls: u64,
    /// Descriptors of child pages already built in this run.
    pub opened: std::collections::BTreeSet<u32>,
    /// Child pages built through page open calls.
    pub children: u32,
    /// Inside a scroll list: dividers between items.
    pub list_dividers: bool,
    /// Nesting depth of ported helpers (recursion guard).
    pub depth: u32,
    /// Calls the toolkit has no model for, by address (a measure of coverage).
    pub unmodelled: BTreeMap<u32, u32>,
    /// Calls deliberately ignored (layout, scroll, hints), by address.
    pub ignored: BTreeMap<u32, u32>,
    pub error: Option<BuilderError>,
}

impl<B: PageBuilder> Toolkit<'_, B> {
    fn arg(&self, r: &ExtRegisters, i: u32) -> u32 {
        self.mem.read32(r.esp.wrapping_add(4 * i))
    }
    fn note(&mut self, res: Result<(), BuilderError>) {
        if let Err(e) = res {
            self.error.get_or_insert(e);
        }
    }
    fn widget_id(&self, raw: u32) -> WidgetId {
        // Same rule as the loader: a negative (or repeated, see the recorder) id is anonymous.
        if (raw as i32) < 0 {
            WidgetId::anon(self.rows)
        } else {
            WidgetId(raw as u64)
        }
    }
    /// Adds a row. The previous row is recorded first; this one stays pending until the next
    /// call that needs it recorded, so fields the builder stores into its node afterwards
    /// (activate handler, draw handler, text) are read back from the synthetic memory.
    fn row(&mut self, widget: Widget, node: Option<u32>) {
        self.flush_row();
        self.rows += 1;
        self.pending = Some((widget, node));
    }
    fn flush_row(&mut self) {
        let Some((mut w, node)) = self.pending.take() else {
            return;
        };
        if let Some(addr) = node {
            let activate = self.mem.read32(addr + 0x38);
            if activate != 0 {
                w.activate = Some(HandlerRef::Legacy(activate));
                if w.kind == WidgetKind::Label {
                    w.kind = WidgetKind::Button;
                }
            }
            let draw = self.mem.read32(addr + 0x50);
            if draw != 0 {
                w.draw = Some(HandlerRef::Legacy(draw));
            }
            let text = self.mem.read32(addr + 0x20);
            if text != 0 {
                w.text = text_ref(text);
            }
        }
        let res = self.rec.record_row(w);
        self.note(res);
    }
}

/// Runs the machine-translated original function at `va` (any c2r module) on `m`.
fn dispatch_ported<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) -> bool {
    c2r::run(va, m, guest, r)
}

/// The GuestCall boundary has arguments at ESP; a translated callee expects a
/// return slot followed by those arguments. Match the host bridge for nested calls.
fn call_ported<M: GuestMemory, C: GuestCall>(
    target: u32,
    memory: &mut M,
    guest: &mut C,
    registers: &mut ExtRegisters,
) -> bool {
    let arguments = registers.esp;
    registers.esp = arguments.wrapping_sub(4);
    memory.write32(registers.esp, 0);
    let ran = dispatch_ported(target, memory, guest, registers);
    registers.esp = arguments;
    ran
}

/// Calls one builder run may make before it is declared runaway (a fixture state that sends a
/// loop into a table the fixture lacks).
pub const CALL_BUDGET: u64 = 20_000;

impl<B: PageBuilder> GuestCall for Toolkit<'_, B> {
    fn call(&mut self, target: u32, r: &mut ExtRegisters) {
        self.calls += 1;
        if std::env::var_os("TSFP_TRACE_CALLS").is_some() {
            eprintln!("call {target:#x} depth {}", self.depth);
        }
        assert!(
            self.calls <= CALL_BUDGET,
            "builder exceeded the call budget"
        );
        match target {
            va::TEXT_BY_ID => r.eax = TEXT_BASE + self.arg(r, 0).wrapping_mul(8),
            va::BEGIN_BUILD => {
                // (parent, title text, title size, select handler, back handler)
                self.flush_row();
                let res = self.rec.push_page(self.page, 1);
                self.note(res);
                for (slot, token) in [
                    (HandlerSlot::Select, self.arg(r, 3)),
                    (HandlerSlot::Back, self.arg(r, 4)),
                ] {
                    if token != 0 {
                        self.flush_row();
                        let res = self.rec.set_handler(slot, HandlerRef::Legacy(token));
                        self.note(res);
                    }
                }
            }
            t if va::LIST_BEGIN.contains(&t) => {
                self.list_dividers = true;
                *self.ignored.entry(t).or_default() += 1;
            }
            va::LIST_END => {
                self.list_dividers = false;
                *self.ignored.entry(va::LIST_END).or_default() += 1;
            }
            // Unported leaf functions that only read tables of the image (readable ports).
            0x22e050 => r.eax = crate::tables::descriptor(&self.mem, self.arg(r, 0)),
            0x22e070 => r.eax = crate::tables::record(self.arg(r, 0)),
            0x69620 => r.eax = crate::tables::profile(self.arg(r, 0)),
            0x66470 => {
                let id = crate::tables::text_id(&self.mem, self.arg(r, 0));
                r.eax = TEXT_BASE.wrapping_add(id.wrapping_mul(8));
            }
            0x71cd0 => {
                // set or clear bit 0 of the flag word at +0xc
                let (object, on) = (self.arg(r, 0), self.arg(r, 1));
                let word = self.mem.read32(object + 0xc) & !1;
                self.mem.write32(object + 0xc, word | u32::from(on != 0));
            }
            0x78e50 | 0x79b70 => {
                // (parent, page descriptor, ...): build the child page from its descriptor's
                // builder as a nested page of the same recording, once per run.
                let descriptor = self.arg(r, 1);
                let builder = self.mem.read32(descriptor);
                if descriptor != 0
                    && builder != 0
                    && self.opened.insert(descriptor)
                    && self.depth < 4
                {
                    let parent = self.page;
                    self.page = PageId(u64::from(descriptor));
                    let mut m = self.mem.clone();
                    self.depth += 1;
                    let mut child_regs = regs();
                    let ran = dispatch_ported(builder, &mut m, self, &mut child_regs);
                    self.depth -= 1;
                    if ran {
                        self.flush_row();
                        let res = self.rec.pop_page();
                        self.note(res);
                        self.children += 1;
                    } else {
                        *self.unmodelled.entry(builder).or_default() += 1;
                    }
                    self.page = parent;
                }
                r.eax = 0x7000_8000;
            }
            0x69640 => r.eax = 0x7bee40, // profile record of local slot 0 (no network session)
            0x356870 => r.eax = self.arg(r, 0), // player slot field: the argument when offline
            va::ADD_TEXT_ITEM => {
                if self.list_dividers && self.rows > 0 {
                    let mut w = Widget::new(WidgetId::anon(self.rows), WidgetKind::Panel);
                    w.navigable = false;
                    self.row(w, None);
                }
                // (text, row id, flags, group, activate handler)
                let (text, id, flags, activate) = (
                    self.arg(r, 0),
                    self.arg(r, 1),
                    self.arg(r, 2),
                    self.arg(r, 4),
                );
                let mut w = Widget::new(
                    self.widget_id(id),
                    if activate != 0 {
                        WidgetKind::Button
                    } else {
                        WidgetKind::Label
                    },
                );
                w.text = text_ref(text);
                w.enabled = flags & 1 == 0;
                w.navigable = flags & 4 == 0;
                w.activate = (activate != 0).then_some(HandlerRef::Legacy(activate));
                // The original returns the new node and builders store into it: a node in the
                // synthetic memory, read back when the row is recorded.
                let node = 0x7000_0000 + (self.rows as u32 + 1) * 0x200;
                self.mem.write32(node + 0x20, text);
                self.mem.write32(node + 0x38, activate);
                self.row(w, Some(node));
                r.eax = node;
            }
            va::ADD_DIVIDER_BY_INDEX | va::ADD_DIVIDER => {
                let mut w = Widget::new(WidgetId::anon(self.rows), WidgetKind::Panel);
                w.navigable = false;
                self.row(w, None);
            }
            va::ADD_OPTION => {
                // (text pointer): a value of the row just added, id -1 as in the dump
                let option = UiOption {
                    id: u32::MAX,
                    text: text_ref(self.arg(r, 0)),
                    enabled: true,
                };
                self.flush_row();
                let res = self.rec.record_option(option);
                self.note(res);
            }
            va::ADD_ON_OFF_OPTIONS => {
                for id in [0x1837u32, 0x1836] {
                    let option = UiOption {
                        id: u32::MAX,
                        text: TextRef::Id(TextId(id)),
                        enabled: true,
                    };
                    self.flush_row();
                    let res = self.rec.record_option(option);
                    self.note(res);
                }
            }
            va::SELECT_ROW_BY_ID => {
                let res = self.rec.set_initial_focus(WidgetId(self.arg(r, 1) as u64));
                self.note(res);
            }
            t if va::ROW_HELPERS.contains(&t) => {
                let mut m = self.mem.clone();
                if !call_ported(target, &mut m, self, r) {
                    *self.unmodelled.entry(target).or_default() += 1;
                }
            }
            t if va::TYPED_ITEMS.iter().any(|(a, _)| *a == t) => {
                let node_type = va::TYPED_ITEMS
                    .iter()
                    .find(|(a, _)| *a == t)
                    .map_or(0, |x| x.1);
                let mut w = Widget::new(WidgetId::anon(self.rows), WidgetKind::Custom(node_type));
                w.navigable = false;
                let node = 0x7000_0000 + (self.rows as u32 + 1) * 0x200;
                self.row(w, Some(node));
                r.eax = node;
            }
            t if va::IGNORED.contains(&t) => *self.ignored.entry(t).or_default() += 1,
            t => {
                if let Some(v) = self.scenario.returns.get(&t) {
                    r.eax = *v;
                } else if va::PREDICATES.contains(&t) {
                    r.eax = 0;
                } else {
                    // Any other original function runs as ported code on the synthetic memory.
                    let mut m = self.mem.clone();
                    if self.depth < 8 {
                        self.depth += 1;
                        let ran = call_ported(t, &mut m, self, r);
                        self.depth -= 1;
                        if !ran {
                            *self.unmodelled.entry(t).or_default() += 1;
                            r.eax = 0;
                        }
                    } else {
                        *self.unmodelled.entry(t).or_default() += 1;
                        r.eax = 0;
                    }
                }
            }
        }
    }
}

/// Why a page could not be built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildError {
    /// The address is not a ported builder in the ui menu or settings modules.
    NotPorted(u32),
    /// The recorder rejected the call sequence (including a builder that never begins a page).
    Recorder(BuilderError),
}

pub struct PageRun {
    pub unmodelled: BTreeMap<u32, u32>,
    pub ignored: BTreeMap<u32, u32>,
    /// Child pages built through page open calls (they are in the recorded model).
    pub children: u32,
    /// Dwords the builder read from unwritten fixture memory (address, read count).
    pub missed: BTreeMap<u32, u32>,
}

/// Runs the ported builder at `builder` for page `page` into `rec` (push, build, pop).
pub fn build_page<B: PageBuilder>(
    rec: &mut B,
    page: PageId,
    builder: u32,
    scenario: &Scenario,
) -> Result<PageRun, BuildError> {
    let mut mem = Ram {
        image: scenario.image.clone(),
        ..Ram::default()
    };
    for (a, v) in &scenario.words {
        mem.write32(*a, *v);
    }
    // A real root argument avoids aliasing widget fields with address zero.
    let root = 0x7000_8000;
    mem.write32(regs().esp + 4, root);
    mem.write32(root + 0x38, scenario.page_slot);
    mem.write32(0x7bb1e8, root);
    let mut kit = Toolkit {
        rec,
        mem: mem.clone(),
        page,
        scenario: scenario.clone(),
        rows: 0,
        pending: None,
        calls: 0,
        opened: Default::default(),
        children: 0,
        list_dividers: false,
        depth: 0,
        unmodelled: BTreeMap::new(),
        ignored: BTreeMap::new(),
        error: None,
    };
    let mut r = regs();
    if !dispatch_ported(builder, &mut mem, &mut kit, &mut r) {
        return Err(BuildError::NotPorted(builder));
    }
    kit.flush_row();
    let res = kit.rec.pop_page();
    kit.note(res);
    match kit.error {
        Some(e) => Err(BuildError::Recorder(e)),
        None => Ok(PageRun {
            unmodelled: kit.unmodelled,
            ignored: kit.ignored,
            children: kit.children,
            missed: mem.missed(),
        }),
    }
}
