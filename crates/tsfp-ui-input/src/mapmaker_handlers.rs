//! T2333 Map Maker callback frontier. Decisions use owned typed editor state.
//! Guest import/export is a TEMPORARY-SHIM under T2044 S3. Neighbouring map,
//! storage and page constructors are explicit services. Cold candidates opt in.
use crate::native_page::{GuestUi, PageRef};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorAction {
    Resume,
    Preview,
    Undo,
    Redo,
    MapSettings,
    EditorSettings,
    ResetView,
    Clear,
    Load,
    Save,
    Controls,
    Quit,
}
impl EditorAction {
    pub fn from_row(row: u32) -> Option<Self> {
        Some(match row {
            0 => Self::Resume,
            1 => Self::Preview,
            2 => Self::Undo,
            3 => Self::Redo,
            4 => Self::MapSettings,
            5 => Self::EditorSettings,
            6 => Self::ResetView,
            7 => Self::Clear,
            8 => Self::Load,
            9 => Self::Save,
            10 => Self::Controls,
            11 => Self::Quit,
            _ => return None,
        })
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EditorState {
    pub flags: u32,
    pub view_flags: u32,
    pub mode: u32,
    pub requested_mode: u32,
    pub dirty: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorPage {
    Preview,
    MapSettings,
    EditorSettings,
    Controls,
}
impl EditorPage {
    fn descriptor(self) -> u32 {
        match self {
            Self::Preview => 0x52D494,
            Self::MapSettings => 0x52D474,
            Self::EditorSettings => 0x52D484,
            Self::Controls => 0x52D510,
        }
    }
}
/// Native extension point: implementations act directly on editor/map services.
pub trait EditorServices {
    fn publish(&mut self, state: &EditorState);
    fn open(&mut self, page: EditorPage);
    fn resume(&mut self);
    fn history(&mut self, direction: u32);
    fn refresh(&mut self);
    fn reset_view(&mut self);
    fn confirm(&mut self, text: u32, yes: u32, no: u32);
    fn load(&mut self);
    /// Returns true when this host owns and handled the Load action.
    fn native_load(&mut self) -> bool {
        false
    }
    /// Returns true when this host owns and handled the Save action.
    fn native_save(&mut self) -> bool {
        false
    }
    fn save(&mut self);
    fn quit(&mut self);
    fn mark_changed(&mut self, kind: u32);
    fn set_map_mode(&mut self, _mode: i32) {}
    fn pop(&mut self);
}
pub fn editor_action(
    state: &mut EditorState,
    action: EditorAction,
    services: &mut dyn EditorServices,
) {
    match action {
        EditorAction::Resume => services.resume(),
        EditorAction::Preview => services.open(EditorPage::Preview),
        EditorAction::MapSettings => services.open(EditorPage::MapSettings),
        EditorAction::EditorSettings => services.open(EditorPage::EditorSettings),
        EditorAction::Controls => services.open(EditorPage::Controls),
        EditorAction::Undo | EditorAction::Redo => {
            services.history(if action == EditorAction::Undo { 1 } else { 2 });
            services.resume();
            services.refresh();
        }
        EditorAction::ResetView => {
            services.reset_view();
            services.resume();
        }
        EditorAction::Clear => services.confirm(0xAE1, 0x311AA0, 0),
        EditorAction::Load => {
            if services.native_load() {
                return;
            }
            state.flags |= 0x400;
            services.publish(state);
            services.load();
        }
        EditorAction::Save => {
            if services.native_save() {
                return;
            }
            state.flags |= 0x400;
            services.publish(state);
            services.save();
        }
        EditorAction::Quit if state.dirty => services.confirm(0xAFA, 0x312DF0, 0),
        EditorAction::Quit => services.quit(),
    }
}
fn mark_changed(state: &mut EditorState, kind: u32, services: &mut dyn EditorServices) {
    if state.flags & 0x20 == 0 {
        services.mark_changed(kind);
        state.flags |= 0x20;
        services.publish(state);
    }
}
pub fn editor_settings_back(
    state: &mut EditorState,
    rows_changed: bool,
    services: &mut dyn EditorServices,
) {
    if rows_changed {
        mark_changed(state, 0x1B, services);
    }
    if state.mode == state.requested_mode {
        services.pop();
    } else if state.requested_mode == 1 {
        state.mode = 1;
        services.publish(state);
        mark_changed(state, 0x1B, services);
        services.pop();
    } else {
        services.confirm(0xAE1, 0x312120, 0x361710);
    }
}
pub fn accept_mode_change(state: &mut EditorState, services: &mut dyn EditorServices) {
    state.mode = state.requested_mode;
    services.publish(state);
    mark_changed(state, 0x1B, services);
    services.pop();
}
/// Imported fixed-size settings records; original comparisons inspect every slot,
/// including unused entries, and preserve signed byte/word widening.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WeaponSettings {
    pub count: u32,
    pub slots: [(u16, i16); 10],
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BotSettings {
    pub count: u32,
    pub slots: [i32; 15],
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MapSettingsState {
    pub editor: EditorState,
    pub theme: u32,
    pub original_theme: u32,
    pub weapons: WeaponSettings,
    pub original_weapons: WeaponSettings,
    pub bots: BotSettings,
    pub original_bots: BotSettings,
    pub game_type: u32,
    pub selected_mode: i32,
}
pub fn map_settings_back(
    state: &mut MapSettingsState,
    rows_changed: bool,
    services: &mut dyn EditorServices,
) {
    if rows_changed
        || state.theme != state.original_theme
        || state.weapons != state.original_weapons
        || state.bots != state.original_bots
    {
        mark_changed(&mut state.editor, 0x1A, services);
    }
    if state.game_type == 0 {
        state.selected_mode = 0;
    } else if state.selected_mode == 0 {
        state.selected_mode = -1;
    }
    services.set_map_mode(state.selected_mode);
    services.pop();
    services.refresh();
}
fn read8(g: &dyn GuestUi, a: u32) -> u8 {
    (read(g, a & !3) >> ((a & 3) * 8)) as u8
}
fn read16(g: &dyn GuestUi, a: u32) -> u16 {
    u16::from_le_bytes([read8(g, a), read8(g, a + 1)])
}
fn write8(g: &mut dyn GuestUi, a: u32, v: u8) {
    let aligned = a & !3;
    let shift = (a & 3) * 8;
    let word = read(g, aligned);
    g.write32(aligned, (word & !(255 << shift)) | (u32::from(v) << shift));
}
fn settings(g: &mut dyn GuestUi) -> MapSettingsState {
    let bank = g.call(0x3365F0, &[0xC77668]).unwrap_or(0);
    let record = g.call(0x32A5B0, &[bank, 0xC77668]).unwrap_or(0);
    let root = read(g, 0x7844A8);
    MapSettingsState {
        editor: import(g),
        theme: read(g, root + 0x208),
        original_theme: read(g, 0x78402C),
        weapons: WeaponSettings {
            count: read(g, root + 0x11860),
            slots: core::array::from_fn(|i| {
                (
                    read16(g, root + 0x11864 + i as u32 * 8),
                    read16(g, root + 0x1186A + i as u32 * 8) as i16,
                )
            }),
        },
        original_weapons: WeaponSettings {
            count: u32::from(read8(g, record + 0x16)),
            slots: core::array::from_fn(|i| {
                (
                    read16(g, record + 0x18 + i as u32 * 4),
                    i16::from(read8(g, record + 0x1B + i as u32 * 4) as i8),
                )
            }),
        },
        bots: BotSettings {
            count: read(g, root + 0x11908),
            slots: core::array::from_fn(|i| read(g, root + 0x1190C + i as u32 * 4) as i32),
        },
        original_bots: BotSettings {
            count: u32::from(read8(g, record + 0x42)),
            slots: core::array::from_fn(|i| {
                i32::from(read16(g, record + 0x44 + i as u32 * 2) as i16)
            }),
        },
        game_type: read(g, root + 0x218),
        selected_mode: read(g, root + 0x220) as i32,
    }
}
/// The name page's return policy owns the string comparison and destination.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SaveNameState {
    pub name: Vec<u8>,
    pub existing: Vec<Vec<u8>>,
    pub matching: Option<usize>,
    pub stage: u32,
    pub cancelled: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveNameResult {
    Return,
    New,
    Overwrite(usize),
}
pub fn save_name_return(state: &mut SaveNameState, cancel: bool) -> SaveNameResult {
    state.cancelled = cancel;
    if cancel || state.name.is_empty() {
        state.stage = 1;
        return SaveNameResult::Return;
    }
    state.matching = state.existing.iter().position(|name| name == &state.name);
    if let Some(index) = state.matching {
        SaveNameResult::Overwrite(index)
    } else {
        state.stage = 7;
        SaveNameResult::New
    }
}
fn bytes(g: &dyn GuestUi, a: u32, limit: usize) -> Vec<u8> {
    (0..limit)
        .map(|i| read8(g, a + i as u32))
        .take_while(|b| *b != 0)
        .collect()
}
/// Owned initial text and route decision; page construction stays a neighbour.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveNameRequest {
    pub initial: Vec<u8>,
    pub frontend: bool,
    pub count: u32,
}
impl SaveNameRequest {
    pub fn at_capacity(&self) -> bool {
        self.count == 0x32
    }
}
fn open_save_name(g: &mut dyn GuestUi, page: PageRef) {
    let count = g.call(0x32A8B0, &[]).unwrap_or(0);
    if (SaveNameRequest {
        initial: Vec::new(),
        frontend: false,
        count,
    })
    .at_capacity()
    {
        let text = g.call(0x82950, &[0x20, 0]).unwrap_or(0);
        g.call(0x82560, &[page.0, u32::MAX, text, 0x32B2B0]);
        return;
    }
    write8(g, 0x7630D8, 1);
    let initial = if read(g, 0x79094C) == 0x65 && read8(g, 0x763E2C) != 0 {
        bytes(g, 0x763E2C, 31)
    } else {
        write8(g, 0x763E2C, 0);
        Vec::new()
    };
    let request = SaveNameRequest {
        initial,
        frontend: read(g, 0x765A80) & 1 != 0,
        count,
    };
    for (i, b) in request
        .initial
        .iter()
        .copied()
        .chain(core::iter::once(0))
        .enumerate()
    {
        write8(g, 0x765A88 + i as u32, b);
    }
    let size = g.call(0x32A8B0, &[]).unwrap_or(0) as u16;
    write8(g, 0x765AEC, size as u8);
    write8(g, 0x765AED, (size >> 8) as u8);
    write8(g, 0x765AEE, 0);
    write8(g, 0x765AEF, 0);
    if request.frontend {
        g.call(0x7D010, &[]);
        let ctx = read(g, 0x764A50);
        g.call(0x79B70, &[ctx, 0x52EA78]);
    } else {
        g.call(
            0x7D090,
            &[
                page.0,
                0x765A88,
                31,
                50.0f32.to_bits(),
                0,
                0,
                0x32AC60,
                0x32AC30,
            ],
        );
    }
}
fn name_return(g: &mut dyn GuestUi, page: PageRef, cancel: bool) {
    let list = read(g, 0x778D38);
    let count = read(g, list + 8) as i32;
    let mut state = SaveNameState {
        name: bytes(g, 0x765A88, 31),
        existing: if cancel {
            Vec::new()
        } else {
            (0..count.max(0) as u32)
                .map(|i| bytes(g, list + 0xC + i * 0x6C, 31))
                .collect()
        },
        ..Default::default()
    };
    match save_name_return(&mut state, cancel) {
        SaveNameResult::Return => {
            g.write32(0x765AF4, 1);
            write8(g, 0x7630D8, 0);
            let ctx = read(g, 0x764A50);
            g.call(0x79B70, &[ctx, 0x52EA98]);
        }
        SaveNameResult::New => {
            g.write32(0x7630D0, 0);
            g.write32(0x765AF4, 7);
            let ctx = read(g, 0x764A50);
            g.call(0x79B70, &[ctx, 0x52EA98]);
        }
        SaveNameResult::Overwrite(i) => {
            g.write32(0x764A54, i as u32);
            let text = g
                .call(0x82950, &[0x2A, list + 0xC + i as u32 * 0x6C])
                .unwrap_or(0);
            g.call(0x82650, &[page.0, u32::MAX, text, 0x32AAD0, 0x32A9D0]);
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageOperation {
    Load,
    Save,
    Other(u32),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageState {
    pub frontend: bool,
    pub operation: StorageOperation,
    pub completed: bool,
    pub primary_return: u32,
    pub fallback_return: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageReturn {
    Editor,
    MapMaker,
    Page(u32),
    Stay,
}
pub fn storage_return(s: &StorageState) -> StorageReturn {
    if !s.frontend {
        return StorageReturn::Editor;
    }
    match s.operation {
        StorageOperation::Load | StorageOperation::Save => {
            if s.completed && s.primary_return != 0 {
                StorageReturn::Page(s.primary_return)
            } else if !s.completed && s.fallback_return != 0 {
                StorageReturn::Page(s.fallback_return)
            } else {
                StorageReturn::MapMaker
            }
        }
        StorageOperation::Other(_) => StorageReturn::Stay,
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceChoice {
    Existing(u32),
    SampleMaps,
    Ignore,
}
impl DeviceChoice {
    pub fn from_id(id: i32) -> Self {
        if (0..11).contains(&id) {
            Self::Existing(id as u32)
        } else if id == 999 {
            Self::SampleMaps
        } else {
            Self::Ignore
        }
    }
}
pub trait DeviceServices {
    fn available(&mut self, index: u32) -> bool;
    fn select(&mut self, index: u32);
    fn samples(&mut self);
    fn denied(&mut self);
}
pub fn select_device(choice: DeviceChoice, services: &mut dyn DeviceServices) {
    match choice {
        DeviceChoice::Existing(index) => {
            if services.available(index) {
                services.select(index);
            } else {
                services.denied();
            }
        }
        DeviceChoice::SampleMaps => services.samples(),
        DeviceChoice::Ignore => {}
    }
}
/// T2319: the port has one local storage device. The chooser 0x32B6A0 lists the devices
/// 0..11 except 10 for which `listed` holds, so a chooser whose only row would be one device
/// is a storage prompt: it is skipped and that device selected directly. More than one device
/// keeps the choice.
pub fn sole_device(mut listed: impl FnMut(u32) -> bool) -> Option<u32> {
    let mut found = None;
    for index in (0..11).filter(|&index| index != 10) {
        if listed(index) {
            if found.is_some() {
                return None;
            }
            found = Some(index);
        }
    }
    found
}
/// The device the chooser would offer as its only row, if it would show no Sample Maps row
/// (0x32B6A0: load with 0x7657F8 set). The picker builder suppresses its rows while a device
/// change is pending, so leaving the picker open in that case creates a visible dead end.
/// A successful device-list query is the readiness check; select_device checks presence again.
fn sole_chooser_device(g: &mut dyn GuestUi) -> Option<u32> {
    if read(g, 0x7657A0) == 0 && read(g, 0x7657F8) != 0 {
        return None;
    }
    sole_device(|index| g.call(0x190AC0, &[index]).unwrap_or(0) != 0)
}
/// Open the device chooser 0x52EA68, or select the sole local device in its place.
fn open_device_chooser(g: &mut dyn GuestUi) {
    if let Some(index) = sole_chooser_device(g) {
        select_device(DeviceChoice::Existing(index), &mut GuestDevices(g));
        return;
    }
    let ctx = read(g, 0x764A50);
    g.call(0x79B70, &[ctx, 0x52EA68]);
}
/// Frontend storage opener 0x32AFE0(context, primary return, fallback return, samples):
/// load from the frontend (0x765A80 bit 0), bit 3 while a network session is joinable.
pub fn begin_frontend_storage(
    g: &mut dyn GuestUi,
    context: u32,
    primary: u32,
    fallback: u32,
    samples: u32,
) {
    g.write32(0x765A80, 1);
    if g.call(0x35E960, &[]).unwrap_or(0) != 0 && g.call(0x3533F0, &[]).unwrap_or(0) != 0 {
        g.write32(0x765A80, 1 | 8);
    }
    g.write32(0x764A50, context);
    g.write32(0x7657F8, samples);
    g.write32(0x765AFC, fallback);
    g.write32(0x7630CC, primary);
    clear_catalog(g);
    for address in [0x7630C8, 0x7657A0, 0x763E28, 0x7630D4] {
        g.write32(address, 0);
    }
    g.call(0x190A80, &[]);
    open_device_chooser(g);
}
struct GuestDevices<'a>(&'a mut dyn GuestUi);
impl DeviceServices for GuestDevices<'_> {
    fn available(&mut self, index: u32) -> bool {
        self.0.call(0x24E60, &[index]).unwrap_or(0) & 255 != 0
    }
    fn select(&mut self, index: u32) {
        self.0.write32(0x7630C8, 0);
        self.0.call(0x827D0, &[index]);
        self.0.write32(0x765B04, 4);
        self.0.write32(0x765AF4, 1);
        let ctx = read(self.0, 0x764A50);
        self.0.call(0x79B70, &[ctx, 0x52EA98]);
    }
    fn samples(&mut self) {
        self.0.call(0x827D0, &[u32::MAX]);
        self.0.write32(0x7630C8, 1);
        self.0.call(0x32A4A0, &[]);
        let ctx = read(self.0, 0x764A50);
        self.0.call(0x79B70, &[ctx, 0x52EA88]);
    }
    fn denied(&mut self) {
        if read(self.0, 0x7BA16C) != 0 {
            let bank = read(self.0, 0x7BA178);
            let sound = read(self.0, bank + 0xC);
            self.0.call(0x21780, &[sound]);
        }
    }
}
fn storage_state(g: &dyn GuestUi) -> StorageState {
    StorageState {
        frontend: read(g, 0x765A80) & 1 != 0,
        operation: match read(g, 0x7657A0) {
            0 => StorageOperation::Load,
            1 => StorageOperation::Save,
            v => StorageOperation::Other(v),
        },
        completed: read(g, 0x763E28) != 0,
        primary_return: read(g, 0x7630CC),
        fallback_return: read(g, 0x765AFC),
    }
}
/// Catalog zeroing is a memory-service boundary, not a guest-offset decision.
fn clear_catalog(g: &mut dyn GuestUi) {
    let base = read(g, 0x778D38);
    for i in 0..0x54C {
        g.write32(base + i * 4, 0);
    }
}
fn begin_storage(g: &mut dyn GuestUi, operation: StorageOperation) {
    let context = read(g, 0x7842A8);
    g.write32(0x764A50, context);
    clear_catalog(g);
    for address in [0x765A80, 0x7630C8, 0x7630D4] {
        g.write32(address, 0);
    }
    g.write32(0x7657F8, 1);
    g.write32(
        0x7657A0,
        if operation == StorageOperation::Save {
            1
        } else {
            0
        },
    );
    g.call(0x190A80, &[]);
    open_device_chooser(g);
}
fn finish_storage(g: &mut dyn GuestUi) {
    g.call(0x82EC0, &[]);
    clear_catalog(g);
    let state = storage_state(g);
    match storage_return(&state) {
        StorageReturn::Editor => {
            let mut state = import(g);
            state.view_flags &= !0x800;
            GuestEditor::new(g, PageRef(0)).publish(&state);
            g.call(0x311930, &[]);
        }
        StorageReturn::MapMaker => {
            let ctx = read(g, 0x764A50);
            g.call(0x79860, &[ctx, 0x523830, u32::MAX]);
        }
        StorageReturn::Page(d) => {
            let ctx = read(g, 0x764A50);
            g.call(0x79B70, &[ctx, d]);
        }
        StorageReturn::Stay => {}
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageBack {
    ConfirmSpecial,
    ConfirmNamed,
    Finish,
}
pub fn storage_back(
    state: &StorageState,
    special: bool,
    accepted_special: bool,
    named: bool,
) -> StorageBack {
    if state.frontend {
        StorageBack::Finish
    } else if special && !accepted_special {
        StorageBack::ConfirmSpecial
    } else if !special && named {
        StorageBack::ConfirmNamed
    } else {
        StorageBack::Finish
    }
}
fn back_storage(g: &mut dyn GuestUi) {
    let state = storage_state(g);
    let special = if state.frontend {
        false
    } else {
        g.call(0x35E960, &[]).unwrap_or(0) != 0
    };
    let accepted = read(g, 0x762CBC) != 0;
    // Repeat the second original query only when it is reached; it is a service,
    // so never cache across calls that could update mode.
    let second = if !state.frontend && !(special && !accepted) {
        g.call(0x35E960, &[]).unwrap_or(0) != 0
    } else {
        special
    };
    let named = if !state.frontend && !second {
        let map = read(g, 0x784014);
        let record = g.call(0x32A5B0, &[map, 0xC77668]).unwrap_or(0);
        read8(g, record + 8) != 0
    } else {
        false
    };
    let decision = if special && !accepted && !state.frontend {
        StorageBack::ConfirmSpecial
    } else {
        if !second && named && !state.frontend {
            StorageBack::ConfirmNamed
        } else {
            StorageBack::Finish
        }
    };
    match decision {
        StorageBack::ConfirmSpecial | StorageBack::ConfirmNamed => {
            let ctx = read(g, 0x764A50);
            g.call(
                0x79B70,
                &[
                    ctx,
                    if decision == StorageBack::ConfirmSpecial {
                        0x52EAB8
                    } else {
                        0x52EAA8
                    },
                ],
            );
        }
        StorageBack::Finish => {
            finish_storage(g);
            g.call(0x300580, &[]);
        }
    }
}
/// Map Maker overwrite-dialog request, imported from the shared dialog service.
#[derive(Clone, Debug, PartialEq)]
pub struct DialogPulse {
    pub timers: [f32; 13],
}
impl DialogPulse {
    pub fn reset() -> Self {
        Self { timers: [-0.2; 13] }
    }
}
fn dialog_record(g: &dyn GuestUi, page: PageRef) -> u32 {
    0x7B97A0 + read(g, page.0 + 0x38) * 0x224
}
fn is_mapmaker_no(g: &dyn GuestUi, page: PageRef) -> bool {
    read(g, dialog_record(g, page) + 0x218) == 0x32A9D0
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DialogReplyState {
    pub active: bool,
}
pub trait DialogReplyServices {
    fn close(&mut self);
    fn publish(&mut self, state: &DialogReplyState);
    fn continue_cancel(&mut self);
}
pub fn cancel_dialog(state: &mut DialogReplyState, services: &mut dyn DialogReplyServices) {
    services.close();
    state.active = false;
    services.publish(state);
    services.continue_cancel();
}
struct GuestDialog<'a> {
    g: &'a mut dyn GuestUi,
    page: PageRef,
}
impl DialogReplyServices for GuestDialog<'_> {
    fn close(&mut self) {
        self.g.call(0x74B10, &[self.page.0]);
    }
    fn publish(&mut self, state: &DialogReplyState) {
        let record = dialog_record(self.g, self.page);
        self.g.write32(record, u32::from(state.active));
    }
    fn continue_cancel(&mut self) {
        let record = dialog_record(self.g, self.page);
        let callback = read(self.g, record + 0x218);
        if callback != 0 && !dispatch(self.g, self.page, callback) {
            self.g.call(callback, &[self.page.0]);
        }
    }
}
fn dialog_no(g: &mut dyn GuestUi, page: PageRef) {
    let mut state = DialogReplyState {
        active: read(g, dialog_record(g, page)) != 0,
    };
    cancel_dialog(&mut state, &mut GuestDialog { g, page });
}
fn read(g: &dyn GuestUi, a: u32) -> u32 {
    g.read32(a).unwrap_or(0)
}
fn import(g: &dyn GuestUi) -> EditorState {
    let root = read(g, 0x7844A8);
    EditorState {
        flags: read(g, root + 0x238),
        view_flags: read(g, root + 0x25C),
        mode: read(g, root + 0x200),
        requested_mode: read(g, 0x52D148),
        dirty: read(g, 0x78407C) != 0,
    }
}
struct GuestEditor<'a> {
    g: &'a mut dyn GuestUi,
    page: PageRef,
    last: EditorState,
}
impl<'a> GuestEditor<'a> {
    fn new(g: &'a mut dyn GuestUi, page: PageRef) -> Self {
        let last = import(g);
        Self { g, page, last }
    }
}
impl EditorServices for GuestEditor<'_> {
    fn publish(&mut self, s: &EditorState) {
        let root = read(self.g, 0x7844A8);
        // Publish only owned changes. A neighbour may update other flags while
        // marking a change; the original re-reads the word before its OR/store.
        for (offset, before, after) in [
            (0x238, self.last.flags, s.flags),
            (0x25C, self.last.view_flags, s.view_flags),
        ] {
            let mask = before ^ after;
            if mask != 0 {
                let current = read(self.g, root + offset);
                self.g
                    .write32(root + offset, (current & !mask) | (after & mask));
            }
        }
        if self.last.mode != s.mode {
            self.g.write32(root + 0x200, s.mode);
        }
        self.last = s.clone();
    }
    fn open(&mut self, p: EditorPage) {
        let ctx = read(self.g, 0x7842A8);
        self.g.call(0x79B70, &[ctx, p.descriptor()]);
    }
    fn resume(&mut self) {
        resume(self.g, self.page);
    }
    fn history(&mut self, d: u32) {
        if self.g.native_map_history(d).is_some() {
            return;
        }
        let map = read(self.g, 0x784014);
        self.g.call(0x337760, &[map]);
        let map = read(self.g, 0x784014);
        self.g.call(0x337600, &[map, d]);
    }
    fn refresh(&mut self) {
        self.g.call(0x300580, &[]);
    }
    fn reset_view(&mut self) {
        self.g.call(0x308F40, &[]);
    }
    fn confirm(&mut self, text: u32, yes: u32, no: u32) {
        self.g.call(0x2F79E0, &[0x1575]);
        let text = self.g.call(0x3D7B0, &[text]).unwrap_or(0);
        self.g.call(0x7A080, &[self.page.0, text, yes, no]);
    }
    fn load(&mut self) {
        begin_storage(self.g, StorageOperation::Load);
    }
    fn native_load(&mut self) -> bool {
        self.g.native_map_load()
    }
    fn native_save(&mut self) -> bool {
        self.g.native_map_save()
    }
    fn save(&mut self) {
        let special = self.g.call(0x3C230, &[0x4766D8]).unwrap_or(0) != 0;
        if special {
            self.g.call(0x338020, &[]);
        } else {
            begin_storage(self.g, StorageOperation::Save);
        }
    }
    fn quit(&mut self) {
        self.g.call(0x312DF0, &[self.page.0]);
    }
    fn mark_changed(&mut self, k: u32) {
        self.g.call(0x3370A0, &[k]);
    }
    fn set_map_mode(&mut self, mode: i32) {
        let root = read(self.g, 0x7844A8);
        self.g.write32(root + 0x220, mode as u32);
    }
    fn pop(&mut self) {
        self.g.call(0x7A7A0, &[self.page.0]);
    }
}
/// Original editor close callback 0x3119B0; viewport/page services stay neighbours.
fn resume(g: &mut dyn GuestUi, page: PageRef) {
    if page.0 != 0 {
        let ctx = read(g, page.0 + 0x2C);
        g.call(0x79860, &[ctx, 0, u32::MAX]);
    }
    let mut state = import(g);
    state.view_flags &= !8;
    GuestEditor::new(g, page).publish(&state);
    for channel in [0, 1, 5, 8] {
        g.call(0x335840, &[channel]);
    }
    for channel in [2, 3] {
        g.call(0x3356F0, &[channel]);
    }
    if import(g).mode == 0 {
        g.call(0x3356F0, &[4]);
    }
    g.call(0x30CD40, &[0, u32::MAX]);
    g.call(0x73C80, &[]);
    for player in 0..4 {
        g.call(0x42830, &[player, 0x100]);
    }
    let mut state = import(g);
    state.flags |= 0x4000;
    GuestEditor::new(g, page).publish(&state);
}
pub const CANDIDATE_HANDLERS: &[u32] = &[
    0x312E50, 0x3119B0, 0x312170, 0x312120, 0x313310, 0x3128F0, 0x32AC30, 0x32AC60, 0x32AD60,
    0x7C4F0, 0x32B2B0, 0x32B380, 0x32AB40, 0x312B60, 0x32B9C0, 0x32A9F0, 0x32A9D0, 0x7CDE0,
    0x79790, 0x82460,
];
/// Tier 2 page-topology callbacks (editor 40-state and storage/text 66-state
/// circuits EQUAL on one binary), native by default; TSFP_UI_MAPMAKER_HANDLERS=0
/// restores the originals, 1 adds the cold candidates, a VA list bisects.
pub const ADMITTED_HANDLERS: &[u32] = &[
    0x312E50, 0x3119B0, 0x312170, 0x313310, 0x32B2B0, 0x32A9D0, 0x32A9F0, 0x32AB40, 0x32AC30,
    0x32AC60, 0x32B9C0, 0x79790, 0x7CDE0, 0x82460, 0x7C4F0,
];
/// Tier 0 candidates not reached natively by those traces.
pub const COLD_HANDLERS: &[u32] = &[0x312120, 0x3128F0, 0x32AD60, 0x32B380, 0x312B60];

/// Map Maker pages whose native command path owns cursor navigation while the
/// original viewport pointer context is active (see docs/t-ui-framework.md §4).
pub const NATIVE_CURSOR_PAGES: &[u32] = &[
    0x31_3010, 0x31_33E0, 0x31_3FE0, 0x31_2270, 0x31_5720, 0x31_90C0, 0x31_4420, 0x31_4650,
    0x31_FDC0, 0x32_23A0, 0x32_2BB0, 0x32_78D0, 0x32_8D20, 0x32_B6A0, 0x32_BEA0, 0x32_AEE0,
    0x32_9DE0, 0x32_8350, 0x32_7F90, 0x2B_F830, 0x2C_1830, 0x32_7A70, 0x7_A080,
];

/// Owner-route aliases whose Map Maker cursor role is INFERRED from the
/// 2026-10-19 live route; keep them separate from measured page builders.
pub const INFERRED_CURSOR_PAGE_ALIASES: &[u32] = &[0x2B_F770, 0x19_2110];

/// The measured descriptor-less Select Mode popup is a native page action
/// target. Map Maker's owner preset selects the Advanced row through this
/// command seam so the original row callback owns the mode transition.
pub const MODE_CHOICE_BUILDER: u32 = 0x7_A080;
pub const DEFAULT_MODE_CHOICE_ROW: u32 = 1;

/// The first Map Maker "Select Mode" prompt is a descriptor-less type-2 child
/// of the Map Maker picker. Its Beginner and Advanced rows both use the
/// measured `0x312B60` callback, so it needs the same pointer-context cursor
/// policy as the registered Map Maker pages.
const MODE_CHOICE_POPUP_PARENTS: &[u32] = &[
    0x2C_8940, 0x31_3010, 0x31_33E0, 0x31_2270, 0x31_5720, 0x31_4420, 0x32_B6A0, 0x32_BEA0,
    0x32_AEE0,
];

/// Whether `page` is the live Select Mode popup built by `0x312BC0`.
/// The exact duplicate callback pair and active-child relationship beneath a
/// Map Maker page identify it before the mode callback has necessarily set
/// the editor's mode field.
pub fn is_mode_choice_popup(g: &dyn GuestUi, page: PageRef) -> bool {
    use crate::menu_state::layout::{page as page_field, row};

    if g.read32(page.0) != Some(2)
        || crate::native_page::popup_over(g, MODE_CHOICE_POPUP_PARENTS) != Some(page)
    {
        return false;
    }

    let Some(mut node) = g.read32(page.0 + page_field::FIRST_NODE) else {
        return false;
    };
    let mut seen = Vec::new();
    let mut handlers = Vec::new();
    while node != 0 && seen.len() < 256 && !seen.contains(&node) {
        seen.push(node);
        let handler = g.read32(node + row::ON_ACTIVATE).unwrap_or(0);
        if handler != 0 {
            handlers.push(handler);
        }
        node = g.read32(node + row::NEXT).unwrap_or(0);
    }
    node == 0 && handlers == [0x31_2B60, 0x31_2B60]
}

pub fn enabled_for(handler: u32) -> bool {
    match std::env::var("TSFP_UI_MAPMAKER_HANDLERS") {
        Ok(v) => {
            v == "1"
                || v.split(',').any(|s| {
                    u32::from_str_radix(s.trim().strip_prefix("0x").unwrap_or(s.trim()), 16)
                        == Ok(handler)
                })
        }
        Err(_) => ADMITTED_HANDLERS.contains(&handler),
    }
}
pub fn dispatch(g: &mut dyn GuestUi, page: PageRef, handler: u32) -> bool {
    if !CANDIDATE_HANDLERS.contains(&handler) || !enabled_for(handler) {
        return false;
    }
    match handler {
        0x79790 => {
            let parent = PageRef(read(g, page.0 + 0x2C));
            if read(g, page.0 + 0x1D0) != 0x82460 || !is_mapmaker_no(g, parent) {
                return false;
            }
            g.call(0x78D80, &[page.0]);
            let pulse = DialogPulse::reset();
            let base = 0x7BA2DC + read(g, 0x7BA9E0) * 0x1E0;
            for (i, timer) in pulse.timers.iter().enumerate() {
                g.write32(base + i as u32 * 4, timer.to_bits());
            }
            let parent = PageRef(read(g, page.0 + 0x2C));
            if !dispatch(g, parent, 0x82460) {
                g.call(0x82460, &[parent.0]);
            }
        }
        0x82460 => {
            if !is_mapmaker_no(g, page) {
                return false;
            }
            dialog_no(g, page);
        }
        0x7CDE0 => {
            let source = read(g, page.0 + 0x224);
            if !matches!(source, 0x783CE0 | 0x783E00 | 0x765A88) {
                return false;
            }
            g.call(0x78D80, &[page.0]);
            let handler = read(g, page.0 + 0x1D0);
            if handler != 0 {
                let ctx = PageRef(read(g, page.0 + 0x2C));
                if !dispatch(g, ctx, handler) {
                    g.call(handler, &[ctx.0]);
                }
            }
        }
        0x32B9C0 => open_save_name(g, page),
        0x32A9F0 => {
            clear_catalog(g);
            g.write32(0x7630D4, 0);
            g.call(0x190A80, &[]);
            // A skipped chooser cannot be returned to: Back leaves storage as its Back 0x32B2B0 does.
            if sole_chooser_device(g).is_some() {
                finish_storage(g);
            } else {
                let ctx = read(g, 0x764A50);
                g.call(0x79B70, &[ctx, 0x52EA68]);
            }
        }
        0x32A9D0 => {
            g.write32(0x765AF4, 1);
            let ctx = read(g, 0x764A50);
            g.call(0x79B70, &[ctx, 0x52EA98]);
        }
        0x312B60 => {
            let has_row = read(g, page.0 + 0x198) != 0;
            let selected = read(g, page.0 + 0x18C);
            if has_row {
                let mut state = import(g);
                state.mode = selected;
                state.requested_mode = selected;
                state.view_flags &= !0x40000;
                state.flags &= !0x400;
                GuestEditor::new(g, page).publish(&state);
                g.write32(0x52D148, state.requested_mode);
                g.call(0x311930, &[]);
                g.call(0x300580, &[]);
            }
        }
        0x32B2B0 => finish_storage(g),
        0x32B380 => back_storage(g),
        0x32AB40 => {
            let choice = DeviceChoice::from_id(read(g, page.0 + 0x18C) as i32);
            select_device(choice, &mut GuestDevices(g));
        }
        0x313310 => {
            g.call(0x2FC0A0, &[]);
            let root = read(g, 0x7844A8);
            let value = read(g, root + 0x25C) & !0x100;
            g.write32(root + 0x25C, value);
            let changed = g.call(0x71880, &[page.0, 1]).unwrap_or(0) != 0;
            let mut state = settings(g);
            map_settings_back(&mut state, changed, &mut GuestEditor::new(g, page));
        }
        0x3128F0 => {
            g.call(0x2D2780, &[]);
            let mut state = import(g);
            state.flags |= 0x100;
            GuestEditor::new(g, page).publish(&state);
            g.call(0x335840, &[0]);
            let descriptor = match read(g, page.0 + 0x18C) {
                0 => Some(0x52375C),
                1 => Some(0x52371C),
                _ => None,
            };
            if let Some(d) = descriptor {
                let ctx = read(g, page.0 + 0x2C);
                g.call(0x79B70, &[ctx, d]);
            }
        }
        0x32AC30 => name_return(g, page, true),
        0x32AC60 | 0x32AD60 => name_return(g, page, false),
        0x3119B0 => resume(g, page),
        0x312E50 => {
            let row = read(g, page.0 + 0x18C);
            if let Some(action) = EditorAction::from_row(row) {
                let mut state = import(g);
                editor_action(&mut state, action, &mut GuestEditor::new(g, page));
            }
        }
        0x312170 => {
            let changed = g.call(0x71880, &[page.0, 1]).unwrap_or(0) != 0;
            let mut state = import(g);
            editor_settings_back(&mut state, changed, &mut GuestEditor::new(g, page));
        }
        0x312120 => {
            g.call(0x2FEBF0, &[1]);
            let mut state = import(g);
            accept_mode_change(&mut state, &mut GuestEditor::new(g, page));
        }
        _ => return false,
    }
    if std::env::var_os("TSFP_UI_NATIVE_LOG").is_some() {
        eprintln!("[ui-native] mapmaker handler=0x{handler:X}");
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn awards_chooser_is_admitted_for_native_mapmaker_cursor_input() {
        assert!(NATIVE_CURSOR_PAGES.contains(&0x32_2BB0));
        assert_eq!(
            NATIVE_CURSOR_PAGES
                .iter()
                .filter(|builder| **builder == 0x32_2BB0)
                .count(),
            1
        );
    }

    #[test]
    fn award_flags_page_is_admitted_for_native_mapmaker_cursor_input() {
        assert!(NATIVE_CURSOR_PAGES.contains(&0x31_FDC0));
        assert_eq!(
            NATIVE_CURSOR_PAGES
                .iter()
                .filter(|builder| **builder == 0x31_FDC0)
                .count(),
            1
        );
    }

    #[test]
    fn action_picker_page_is_admitted_for_native_mapmaker_cursor_input() {
        assert!(NATIVE_CURSOR_PAGES.contains(&crate::mapmaker_action_picker::BUILDER));
        assert_eq!(
            NATIVE_CURSOR_PAGES
                .iter()
                .filter(|builder| **builder == crate::mapmaker_action_picker::BUILDER)
                .count(),
            1
        );
    }

    #[test]
    fn assault_phase_start_point_page_is_admitted_for_native_cursor_input() {
        assert!(NATIVE_CURSOR_PAGES.contains(&0x31_90C0));
        assert_eq!(
            NATIVE_CURSOR_PAGES
                .iter()
                .filter(|builder| **builder == 0x31_90C0)
                .count(),
            1
        );
    }

    #[derive(Default)]
    struct HistoryGuest {
        native_result: Option<bool>,
        native_directions: Vec<u32>,
        calls: Vec<(u32, Vec<u32>)>,
    }
    impl GuestUi for HistoryGuest {
        fn read32(&self, _: u32) -> Option<u32> {
            Some(0)
        }
        fn write32(&mut self, _: u32, _: u32) -> bool {
            true
        }
        fn call(&mut self, address: u32, args: &[u32]) -> Option<u32> {
            self.calls.push((address, args.to_vec()));
            Some(0)
        }
        fn native_map_history(&mut self, direction: u32) -> Option<bool> {
            self.native_directions.push(direction);
            self.native_result
        }
    }

    #[test]
    fn native_history_claim_replaces_guest_undo_and_redo_calls() {
        let mut guest = HistoryGuest {
            native_result: Some(true),
            ..Default::default()
        };
        let mut editor = GuestEditor::new(&mut guest, PageRef(0));
        editor.history(1);
        editor.history(2);
        drop(editor);
        assert_eq!(guest.native_directions, [1, 2]);
        assert!(guest.calls.is_empty());
    }

    #[test]
    fn native_history_can_claim_an_empty_stack_without_legacy_fallback() {
        let mut guest = HistoryGuest {
            native_result: Some(false),
            ..Default::default()
        };
        let mut editor = GuestEditor::new(&mut guest, PageRef(0));
        editor.history(1);
        drop(editor);
        assert_eq!(guest.native_directions, [1]);
        assert!(guest.calls.is_empty());
    }

    #[test]
    fn unavailable_native_history_keeps_the_original_callback_path() {
        let mut guest = HistoryGuest::default();
        let mut editor = GuestEditor::new(&mut guest, PageRef(0));
        editor.history(1);
        drop(editor);
        assert_eq!(guest.native_directions, [1]);
        assert_eq!(guest.calls, [(0x337760, vec![0]), (0x337600, vec![0, 1])]);
    }

    #[derive(Default)]
    struct PickerGuest {
        memory: Vec<(u32, u32)>,
        calls: Vec<(u32, Vec<u32>)>,
    }
    impl GuestUi for PickerGuest {
        fn read32(&self, address: u32) -> Option<u32> {
            self.memory
                .iter()
                .rev()
                .find(|(at, _)| *at == address)
                .map(|(_, value)| *value)
        }
        fn write32(&mut self, address: u32, value: u32) -> bool {
            self.memory.push((address, value));
            true
        }
        fn call(&mut self, address: u32, args: &[u32]) -> Option<u32> {
            self.calls.push((address, args.to_vec()));
            match address {
                0x190AA0 => Some(1), // Pending-change case used to strand the visible picker.
                0x190AC0 | 0x24E60 if args.first() == Some(&9) => Some(1),
                _ => Some(0),
            }
        }
    }
    #[derive(Default)]
    struct Services(Vec<String>);
    impl EditorServices for Services {
        fn publish(&mut self, s: &EditorState) {
            self.0.push(format!("publish:{:x}:{}", s.flags, s.mode));
        }
        fn open(&mut self, p: EditorPage) {
            self.0.push(format!("open:{p:?}"));
        }
        fn resume(&mut self) {
            self.0.push("resume".into());
        }
        fn history(&mut self, d: u32) {
            self.0.push(format!("history:{d}"));
        }
        fn refresh(&mut self) {
            self.0.push("refresh".into());
        }
        fn reset_view(&mut self) {
            self.0.push("reset".into());
        }
        fn confirm(&mut self, t: u32, y: u32, n: u32) {
            self.0.push(format!("confirm:{t:x}:{y:x}:{n:x}"));
        }
        fn load(&mut self) {
            self.0.push("load".into());
        }
        fn save(&mut self) {
            self.0.push("save".into());
        }
        fn quit(&mut self) {
            self.0.push("quit".into());
        }
        fn mark_changed(&mut self, k: u32) {
            self.0.push(format!("changed:{k:x}"));
        }
        fn pop(&mut self) {
            self.0.push("pop".into());
        }
    }
    #[test]
    fn admission_partitions_candidates_into_traced_and_cold() {
        assert_eq!(ADMITTED_HANDLERS.len(), 15);
        for &va in CANDIDATE_HANDLERS {
            assert!(
                ADMITTED_HANDLERS.contains(&va) != COLD_HANDLERS.contains(&va),
                "{va:#x}"
            );
        }
        assert_eq!(
            ADMITTED_HANDLERS.len() + COLD_HANDLERS.len(),
            CANDIDATE_HANDLERS.len()
        );
    }
    #[test]
    fn map_settings_inspects_unused_slots_and_preserves_selected_mode() {
        let mut state = MapSettingsState::default();
        let mut e = Services::default();
        state.weapons.slots[9] = (4, -1);
        state.game_type = 1;
        map_settings_back(&mut state, false, &mut e);
        assert_eq!(state.editor.flags & 0x20, 0x20);
        assert_eq!(state.selected_mode, -1);
        assert_eq!(e.0, ["changed:1a", "publish:20:0", "pop", "refresh"]);
        e.0.clear();
        state.selected_mode = 5;
        state.game_type = 0;
        map_settings_back(&mut state, true, &mut e);
        assert_eq!(state.selected_mode, 0);
        assert_eq!(e.0, ["pop", "refresh"]);
    }
    #[test]
    fn settings_adapter_preserves_signed_packed_records() {
        use std::collections::HashMap;
        #[derive(Default)]
        struct Guest(HashMap<u32, u32>);
        impl GuestUi for Guest {
            fn read32(&self, a: u32) -> Option<u32> {
                Some(*self.0.get(&a).unwrap_or(&0))
            }
            fn write32(&mut self, a: u32, v: u32) -> bool {
                self.0.insert(a, v);
                true
            }
            fn call(&mut self, a: u32, args: &[u32]) -> Option<u32> {
                Some(match a {
                    0x3365F0 => 0x20000,
                    0x32A5B0 => {
                        assert_eq!(args, [0x20000, 0xC77668]);
                        0x30000
                    }
                    _ => 0,
                })
            }
        }
        let mut g = Guest::default();
        g.0.insert(0x7844A8, 0x10000);
        g.0.insert(0x10000 + 0x11860, 1);
        g.0.insert(0x10000 + 0x11864, 7);
        g.0.insert((0x10000 + 0x1186A) & !3, 0xFFFF0000);
        write8(&mut g, 0x30000 + 0x16, 1);
        write8(&mut g, 0x30000 + 0x18, 7);
        write8(&mut g, 0x30000 + 0x1B, 255);
        g.0.insert(0x10000 + 0x11908, 1);
        g.0.insert(0x10000 + 0x1190C, u32::MAX);
        write8(&mut g, 0x30000 + 0x42, 1);
        write8(&mut g, 0x30000 + 0x44, 255);
        write8(&mut g, 0x30000 + 0x45, 255);
        let state = settings(&mut g);
        assert_eq!(state.weapons, state.original_weapons);
        assert_eq!(state.bots, state.original_bots);
        assert_eq!(state.weapons.slots[0], (7, -1));
        assert_eq!(state.bots.slots[0], -1);
    }
    #[test]
    fn name_return_distinguishes_cancel_new_and_overwrite() {
        let mut state = SaveNameState {
            name: b"Map".to_vec(),
            existing: vec![b"map".to_vec(), b"Map".to_vec()],
            ..Default::default()
        };
        assert_eq!(
            save_name_return(&mut state, false),
            SaveNameResult::Overwrite(1)
        );
        state.name = b"new".to_vec();
        assert_eq!(save_name_return(&mut state, false), SaveNameResult::New);
        assert_eq!(state.stage, 7);
        assert_eq!(save_name_return(&mut state, true), SaveNameResult::Return);
        assert_eq!(state.stage, 1);
    }
    #[test]
    fn name_entry_request_preserves_exact_capacity_and_constructor_contract() {
        for (count, full) in [(49, false), (50, true), (51, false)] {
            assert_eq!(
                SaveNameRequest {
                    initial: Vec::new(),
                    frontend: false,
                    count
                }
                .at_capacity(),
                full
            );
        }
        use std::collections::HashMap;
        #[derive(Default)]
        struct Guest {
            memory: HashMap<u32, u32>,
            calls: Vec<(u32, Vec<u32>)>,
            count: u32,
        }
        impl GuestUi for Guest {
            fn read32(&self, a: u32) -> Option<u32> {
                Some(*self.memory.get(&a).unwrap_or(&0))
            }
            fn write32(&mut self, a: u32, v: u32) -> bool {
                self.memory.insert(a, v);
                true
            }
            fn call(&mut self, a: u32, v: &[u32]) -> Option<u32> {
                self.calls.push((a, v.to_vec()));
                Some(if a == 0x32A8B0 { self.count } else { 0 })
            }
        }
        let mut g = Guest::default();
        g.memory.insert(0x765AEC, 0xABCD1234);
        open_save_name(&mut g, PageRef(0x1000));
        assert_eq!(g.memory[&0x765AEC], 0);
        assert_eq!(read8(&g, 0x7630D8), 1);
        assert_eq!(
            g.calls,
            [
                (0x32A8B0, vec![]),
                (0x32A8B0, vec![]),
                (
                    0x7D090,
                    vec![
                        0x1000,
                        0x765A88,
                        31,
                        50.0f32.to_bits(),
                        0,
                        0,
                        0x32AC60,
                        0x32AC30
                    ]
                )
            ]
        );
        g.calls.clear();
        g.count = 50;
        open_save_name(&mut g, PageRef(0x1000));
        assert_eq!(
            g.calls,
            [
                (0x32A8B0, vec![]),
                (0x82950, vec![0x20, 0]),
                (0x82560, vec![0x1000, u32::MAX, 0, 0x32B2B0])
            ]
        );
        g.calls.clear();
        let list = 0x20000;
        g.memory.insert(0x778D38, list);
        g.memory.insert(list + 8, 1);
        for (i, b) in b"NativeMap\0".iter().enumerate() {
            write8(&mut g, 0x765A88 + i as u32, *b);
            write8(&mut g, list + 0xC + i as u32, *b);
        }
        name_return(&mut g, PageRef(0x1000), false);
        assert_eq!(
            g.calls,
            [
                (0x82950, vec![0x2A, list + 0xC]),
                (0x82650, vec![0x1000, u32::MAX, 0, 0x32AAD0, 0x32A9D0])
            ]
        );
    }
    #[test]
    fn native_device_selection_has_explicit_availability_boundary() {
        #[derive(Default)]
        struct Devices {
            calls: Vec<String>,
            ready: bool,
        }
        impl DeviceServices for Devices {
            fn available(&mut self, index: u32) -> bool {
                self.calls.push(format!("available:{index}"));
                self.ready
            }
            fn select(&mut self, index: u32) {
                self.calls.push(format!("select:{index}"));
            }
            fn samples(&mut self) {
                self.calls.push("samples".into());
            }
            fn denied(&mut self) {
                self.calls.push("denied".into());
            }
        }
        let mut d = Devices::default();
        select_device(DeviceChoice::from_id(9), &mut d);
        assert_eq!(d.calls, ["available:9", "denied"]);
        d.calls.clear();
        d.ready = true;
        select_device(DeviceChoice::from_id(9), &mut d);
        assert_eq!(d.calls, ["available:9", "select:9"]);
        d.calls.clear();
        select_device(DeviceChoice::from_id(999), &mut d);
        select_device(DeviceChoice::from_id(-1), &mut d);
        assert_eq!(d.calls, ["samples"]);
    }
    #[test]
    fn single_local_device_skips_the_chooser() {
        let present = |devices: &'static [u32]| move |index| devices.contains(&index);
        assert_eq!(sole_device(present(&[9])), Some(9));
        assert_eq!(sole_device(present(&[])), None);
        assert_eq!(sole_device(present(&[0, 9])), None);
        // Index 10 is never a chooser row.
        assert_eq!(sole_device(present(&[9, 10])), Some(9));
    }
    #[test]
    fn a_pending_device_change_does_not_leave_the_single_device_picker_open() {
        let mut guest = PickerGuest::default();
        guest.write32(0x7657A0, 1); // Save mode, no Sample Maps row.
        guest.write32(0x764A50, 0x4444);
        open_device_chooser(&mut guest);

        assert!(guest.calls.contains(&(0x190AC0, vec![9])));
        assert!(!guest.calls.iter().any(|(address, _)| *address == 0x190AA0));
        assert!(guest.calls.contains(&(0x827D0, vec![9])));
        assert!(guest.calls.contains(&(0x79B70, vec![0x4444, 0x52EA98])));
        assert!(!guest.calls.contains(&(0x79B70, vec![0x4444, 0x52EA68])));
    }
    #[test]
    fn storage_returns_and_warning_policy() {
        let mut state = StorageState {
            frontend: false,
            operation: StorageOperation::Save,
            completed: false,
            primary_return: 10,
            fallback_return: 20,
        };
        assert_eq!(storage_return(&state), StorageReturn::Editor);
        assert_eq!(
            storage_back(&state, true, false, false),
            StorageBack::ConfirmSpecial
        );
        assert_eq!(
            storage_back(&state, false, false, true),
            StorageBack::ConfirmNamed
        );
        state.frontend = true;
        assert_eq!(storage_return(&state), StorageReturn::Page(20));
        state.completed = true;
        assert_eq!(storage_return(&state), StorageReturn::Page(10));
        state.primary_return = 0;
        assert_eq!(storage_return(&state), StorageReturn::MapMaker);
    }
    #[test]
    fn guest_menu_load_and_save_use_native_callbacks_without_guest_storage() {
        use std::collections::HashMap;
        #[derive(Default)]
        struct Guest {
            memory: HashMap<u32, u32>,
            calls: Vec<(u32, Vec<u32>)>,
            native_load: bool,
            native_save: bool,
        }
        impl GuestUi for Guest {
            fn read32(&self, a: u32) -> Option<u32> {
                Some(*self.memory.get(&a).unwrap_or(&0))
            }
            fn write32(&mut self, a: u32, v: u32) -> bool {
                self.memory.insert(a, v);
                true
            }
            fn call(&mut self, a: u32, v: &[u32]) -> Option<u32> {
                self.calls.push((a, v.to_vec()));
                Some(0)
            }
            fn native_map_load(&mut self) -> bool {
                self.native_load
            }
            fn native_map_save(&mut self) -> bool {
                self.native_save
            }
        }
        let mut g = Guest::default();
        g.memory.insert(0x7844A8, 0x10000);
        g.memory.insert(0x778D38, 0x20000);
        g.memory.insert(0x7842A8, 0x30000);
        g.memory.insert(0x20000 + 0x152C, 99);
        let mut state = import(&g);
        editor_action(
            &mut state,
            EditorAction::Load,
            &mut GuestEditor::new(&mut g, PageRef(0x40000)),
        );
        assert_eq!(g.memory[&(0x10000 + 0x238)], 0x400);
        assert_eq!(g.memory[&(0x20000 + 0x152C)], 0);
        assert_eq!(
            g.calls,
            [(0x190A80, vec![]), (0x79B70, vec![0x30000, 0x52EA68])]
        );

        g.memory.insert(0x10000 + 0x238, 0);
        g.calls.clear();
        g.native_load = true;
        let mut state = import(&g);
        editor_action(
            &mut state,
            EditorAction::Load,
            &mut GuestEditor::new(&mut g, PageRef(0x40000)),
        );
        assert_eq!(state.flags, 0);
        assert_eq!(g.memory[&(0x10000 + 0x238)], 0);
        assert!(
            g.calls.is_empty(),
            "native Load must not enter guest storage"
        );

        g.calls.clear();
        g.native_save = true;
        let mut state = import(&g);
        editor_action(
            &mut state,
            EditorAction::Save,
            &mut GuestEditor::new(&mut g, PageRef(0x40000)),
        );
        assert_eq!(state.flags, 0);
        assert_eq!(g.memory[&(0x10000 + 0x238)], 0);
        assert!(
            g.calls.is_empty(),
            "native Save must not enter guest storage"
        );
    }
    #[test]
    fn dialog_cancel_releases_request_before_continuation() {
        #[derive(Default)]
        struct Effects(Vec<&'static str>);
        impl DialogReplyServices for Effects {
            fn close(&mut self) {
                self.0.push("close");
            }
            fn publish(&mut self, s: &DialogReplyState) {
                assert!(!s.active);
                self.0.push("publish");
            }
            fn continue_cancel(&mut self) {
                self.0.push("continue");
            }
        }
        let mut state = DialogReplyState { active: true };
        let mut e = Effects::default();
        cancel_dialog(&mut state, &mut e);
        assert_eq!(e.0, ["close", "publish", "continue"]);
        assert!(
            DialogPulse::reset()
                .timers
                .iter()
                .all(|timer| timer.to_bits() == 0xBE4CCCCD)
        );
    }
    #[test]
    fn publication_preserves_neighbour_flag_updates() {
        use std::collections::HashMap;
        struct Guest(HashMap<u32, u32>);
        impl GuestUi for Guest {
            fn read32(&self, a: u32) -> Option<u32> {
                Some(*self.0.get(&a).unwrap_or(&0))
            }
            fn write32(&mut self, a: u32, v: u32) -> bool {
                self.0.insert(a, v);
                true
            }
            fn call(&mut self, a: u32, _: &[u32]) -> Option<u32> {
                if a == 0x3370A0 {
                    self.0.insert(0x10000 + 0x238, 0x1000);
                    self.0.insert(0x10000 + 0x25C, 0x2000);
                }
                Some(0)
            }
        }
        let mut g = Guest(HashMap::from([(0x7844A8, 0x10000)]));
        let mut state = import(&g);
        editor_settings_back(
            &mut state,
            true,
            &mut GuestEditor::new(&mut g, PageRef(0x20000)),
        );
        assert_eq!(g.0[&(0x10000 + 0x238)], 0x1020);
        assert_eq!(g.0[&(0x10000 + 0x25C)], 0x2000);
    }
    #[test]
    fn menu_rows_and_order() {
        assert_eq!(EditorAction::from_row(12), None);
        for row in 0..12 {
            assert!(EditorAction::from_row(row).is_some());
        }
        let mut s = EditorState::default();
        let mut e = Services::default();
        editor_action(&mut s, EditorAction::Undo, &mut e);
        assert_eq!(e.0, ["history:1", "resume", "refresh"]);
        e.0.clear();
        editor_action(&mut s, EditorAction::Save, &mut e);
        assert_eq!(e.0, ["publish:400:0", "save"]);
        e.0.clear();
        editor_action(&mut s, EditorAction::Quit, &mut e);
        assert_eq!(e.0, ["quit"]);
        s.dirty = true;
        e.0.clear();
        editor_action(&mut s, EditorAction::Quit, &mut e);
        assert_eq!(e.0, ["confirm:afa:312df0:0"]);
    }
    #[test]
    fn mode_change_marks_once_and_preserves_warning() {
        let mut s = EditorState {
            mode: 0,
            requested_mode: 1,
            ..Default::default()
        };
        let mut e = Services::default();
        editor_settings_back(&mut s, true, &mut e);
        assert_eq!(s.mode, 1);
        assert_eq!(e.0, ["changed:1b", "publish:20:0", "publish:20:1", "pop"]);
        e.0.clear();
        s.requested_mode = 0;
        editor_settings_back(&mut s, false, &mut e);
        assert_eq!(s.mode, 1);
        assert_eq!(e.0, ["confirm:ae1:312120:361710"]);
        e.0.clear();
        accept_mode_change(&mut s, &mut e);
        assert_eq!(e.0, ["publish:20:0", "pop"]);
    }

    #[test]
    fn dynamic_awards_table_is_admitted_for_native_mapmaker_cursor_input() {
        assert!(NATIVE_CURSOR_PAGES.contains(&0x32_78D0));
        assert_eq!(
            NATIVE_CURSOR_PAGES
                .iter()
                .filter(|builder| **builder == 0x32_78D0)
                .count(),
            1
        );
    }
}
