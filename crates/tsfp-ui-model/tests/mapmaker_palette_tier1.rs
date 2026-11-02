//! T1690 Tier 1: readable palette capture and exit routing against lifted candidates.
//! All guest records and service responses are FABRICATED; this is not live UI or xemu evidence.

#[path = "support/pause_memory.rs"]
mod memory;

use memory::Mem;
use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::editor_next_reference::{f31a500, f31a710, f31af80};
use tsfp_ports::mem::GuestAccess;
use tsfp_ports::memory::GuestMemory;
use tsfp_ui_model::mapmaker_palette::{
    PaletteDescriptor, PaletteDocument, PaletteExit, PaletteNavigation, PaletteRecordId,
    PaletteSession, PaletteSync,
};

const STACK: u32 = 0x9000;
const WIDGET: u32 = 0x10000;
const PARENT: u32 = 0x11000;
const ACTIVE: u32 = 0x12000;
const EDITOR: u32 = 0x13000;
const SELECTION: u32 = 0x14000;
const APPEARANCE: u32 = 0x30000;

fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: STACK,
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
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}

struct DescriptorResolver {
    result: u32,
    calls: Vec<(u32, u32)>,
}

impl GuestCall for DescriptorResolver {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        assert_eq!(va, 0x321400, "capture calls only the descriptor resolver");
        self.calls.push((va, r.esp));
        r.eax = self.result;
    }
}

fn seeded_capture(n: u32) -> (Mem, PaletteDescriptor, PaletteDescriptor, u32, u32) {
    let mut mem = Mem::default();
    let mut seed = n.wrapping_add(1);
    let mut next = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        seed
    };
    mem.write32(WIDGET + 0x2c, PARENT);
    mem.write32(PARENT + 0x174, ACTIVE);
    let active = if n % 3 == 0 { 0x8000_0000 | n } else { next() };
    let comparison = if n % 3 == 0 { active } else { next() };
    mem.write32(ACTIVE, active);
    mem.write32(0x52e030, comparison);
    mem.write32(0x7844a8, EDITOR);
    mem.write32(EDITOR + 0x11858, SELECTION);
    mem.write16(
        SELECTION + 0xc,
        u16::from_le_bytes((n as i16).to_le_bytes()),
    );
    let record = EDITOR
        .wrapping_add(0xdd14)
        .wrapping_add((n as i16 as i32).wrapping_mul(0x8c) as u32);
    for word in 0..5 {
        mem.write32(APPEARANCE + word * 4, next());
        mem.write32(0x783a60 + word * 4, next());
    }
    let prior = PaletteDescriptor(std::array::from_fn(|_| next()));
    for word in 0..5 {
        mem.write32(0x783a60 + word * 4, prior.0[word as usize]);
    }
    let captured = PaletteDescriptor(std::array::from_fn(|word| {
        mem.read32(APPEARANCE + word as u32 * 4)
    }));
    (mem, prior, captured, record, active ^ comparison)
}

#[test]
fn capture_matches_lifted_31a710_across_2048_fabricated_states() {
    let mut cases = 0;
    for n in 0..2048u32 {
        let (mut mem, prior, captured, expected_record, _different) = seeded_capture(n);
        let active = mem.read32(ACTIVE);
        let comparison = mem.read32(0x52e030);
        let mut session = PaletteSession::default();
        session.replace_working_copy(prior);
        let record_id = PaletteRecordId {
            owner: 4,
            slot: n,
            generation: 2,
        };
        let mut guest = DescriptorResolver {
            result: APPEARANCE,
            calls: Vec::new(),
        };
        let mut r = regs();
        r.eax = WIDGET;
        f31a710(&mut mem, &mut guest, &mut r);
        assert_eq!(r.esp, STACK, "lifted capture restores its stack");

        let copied = session.capture_linked_appearance(active, comparison, record_id, captured);
        let lifted_copied = active != comparison;
        assert_eq!(copied, lifted_copied);
        assert_eq!(
            session.working(),
            PaletteDescriptor(std::array::from_fn(
                |word| mem.read32(0x783a60 + word as u32 * 4)
            ))
        );
        assert_eq!(
            guest.calls.len(),
            usize::from(lifted_copied),
            "only a changed active descriptor resolves the selected record"
        );
        if lifted_copied {
            assert_eq!(mem.read32(guest.calls[0].1), expected_record);
            assert_eq!(
                session.capture_source(),
                Some(
                    tsfp_ui_model::mapmaker_palette::PaletteCaptureSource::LinkedAppearance(
                        record_id
                    )
                )
            );
        } else {
            assert_eq!(session.working(), prior);
        }
        cases += 1;
    }
    assert_eq!(cases, 2048);
}

struct ExitCalls(Vec<u32>);

impl GuestCall for ExitCalls {
    fn call(&mut self, va: u32, _r: &mut ExtRegisters) {
        self.0.push(va);
    }
}

#[test]
fn exit_routing_matches_lifted_31a500_dispatch() {
    let cases = [
        (0x52e000, PaletteExit::Properties, vec![0x7a7a0]),
        (0x52e020, PaletteExit::AppearanceRow, vec![0x7a7a0]),
        (0x52e030, PaletteExit::PaletteGrid, vec![0x7a7a0]),
        (0x52e040, PaletteExit::AppearancePrompt, vec![0x7a7a0]),
        (0x52dff0, PaletteExit::ToolPalette, vec![0x31a4e0, 0x2fb090]),
        (0xdeadbeef, PaletteExit::Unknown(0xdeadbeef), vec![]),
    ];
    for (descriptor, page, expected_calls) in cases {
        let mut mem = Mem::default();
        let mut guest = ExitCalls(Vec::new());
        let mut r = regs();
        r.esi = WIDGET;
        mem.write32(WIDGET + 4, descriptor);
        f31a500(&mut mem, &mut guest, &mut r);
        assert_eq!(guest.0, expected_calls);
        assert_eq!(r.esp, STACK);
        let session = PaletteSession::default();
        assert_eq!(
            session.exit(page),
            match page {
                PaletteExit::Properties
                | PaletteExit::AppearanceRow
                | PaletteExit::PaletteGrid
                | PaletteExit::AppearancePrompt => PaletteNavigation::PopCurrentPage,
                PaletteExit::ToolPalette => PaletteNavigation::ResetToolThenPop,
                PaletteExit::Unknown(_) => PaletteNavigation::IgnoreUnknownPage,
            }
        );
    }
}

#[derive(Default)]
struct SelectedPalette {
    selected: Option<PaletteRecordId>,
    descriptor: Option<PaletteDescriptor>,
}

impl PaletteDocument for SelectedPalette {
    fn current_palette_record(&self) -> Option<PaletteRecordId> {
        self.selected
    }
    fn write_palette_descriptor(&mut self, id: PaletteRecordId, value: PaletteDescriptor) -> bool {
        if self.selected != Some(id) || self.descriptor.is_none() {
            return false;
        }
        self.descriptor = Some(value);
        true
    }
    fn initialize_properties_marker_if_zero(&mut self, _id: PaletteRecordId) -> bool {
        false
    }
}

struct PageGateGuest {
    calls: Vec<(u32, u32)>,
}

impl GuestCall for PageGateGuest {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        self.calls.push((va, r.esp));
        r.eax = if va == 0x321400 { APPEARANCE } else { 0 };
    }
}

#[test]
fn current_selection_sync_matches_lifted_31af80_before_input_dispatch() {
    let mut cases = 0;
    for n in 0..2048u32 {
        let mut mem = Mem::default();
        let page = 0x18000;
        let selection = SELECTION;
        let id = PaletteRecordId {
            owner: 5,
            slot: n,
            generation: 3,
        };
        let mut seed = n.wrapping_add(9);
        let mut next = || {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            seed
        };
        let descriptor = PaletteDescriptor(std::array::from_fn(|_| next()));
        let previous = PaletteDescriptor(std::array::from_fn(|_| next()));

        mem.write32(STACK + 4, page);
        mem.write32(page + 4, 0x52e000);
        mem.write32(page + 0x198, 0); // no row callback is selected
        mem.write32(0x7844a8, EDITOR);
        mem.write32(EDITOR + 0x11858, selection);
        mem.write16(
            selection + 0xc,
            u16::from_le_bytes((n as i16).to_le_bytes()),
        );
        mem.write32(EDITOR + 0x2f18, 0); // no object-node service
        mem.write32(EDITOR + 0x25c, 0);
        for word in 0..5 {
            mem.write32(0x783a60 + word * 4, previous.0[word as usize]);
            mem.write32(APPEARANCE + word * 4, descriptor.0[word as usize]);
        }

        let mut guest = PageGateGuest { calls: Vec::new() };
        let mut r = regs();
        f31af80(&mut mem, &mut guest, &mut r);
        assert_eq!(r.esp, STACK, "lifted page gate restores its stack");
        let lifted = PaletteDescriptor(std::array::from_fn(|word| {
            mem.read32(APPEARANCE + word as u32 * 4)
        }));

        let mut session = PaletteSession::default();
        session.replace_working_copy(previous);
        let mut document = SelectedPalette {
            selected: Some(id),
            descriptor: Some(descriptor),
        };
        assert_eq!(
            session.synchronize_before_input(&mut document),
            PaletteSync::Written(id)
        );
        assert_eq!(document.descriptor, Some(lifted));
        assert_eq!(
            lifted, previous,
            "the page gate writes current scratch, not the descriptor's old value"
        );
        assert!(guest.calls.iter().any(|(va, _)| *va == 0x321400));
        assert!(guest.calls.iter().any(|(va, _)| *va == 0x73de0));
        cases += 1;
    }
    assert_eq!(cases, 2048);
}
