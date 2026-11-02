use tsfp_ui::flag_leaves::{any_entry_flag_set, record_68_clear_bit1_set_state, slot_ready};
use tsfp_ui::gprs::Gprs;
use tsfp_ui::memory::GuestMemory;
use tsfp_ui::strmem::SparseMemory;

const ESP: u32 = 0x1000;
const REGS: Gprs = Gprs {
    eax: 7,
    ecx: 8,
    edx: 9,
};

#[test]
fn entry_scan_preserves_first_match_and_end_pointer() {
    let mut mem = SparseMemory::new();
    mem.write32(ESP + 4, 0x2000);
    mem.write32(0x2000 + 0x1c, 0x3000);
    mem.write32(0x2000 + 0x20, 3);
    mem.write8(0x3000 + 0x14 + 1, 2);
    assert_eq!(any_entry_flag_set(&mem, ESP, REGS).ecx, 0x3014);
    mem.write8(0x3000 + 0x14 + 1, 0);
    assert_eq!(any_entry_flag_set(&mem, ESP, REGS).ecx, 0x303c);
}

#[test]
fn slot_and_record_fields_preserve_wrapping_addresses() {
    let mut mem = SparseMemory::new();
    mem.write32(ESP + 4, 0x2000);
    mem.write32(ESP + 8, 2);
    mem.write32(0x2000 + 0x148, 1);
    assert_eq!(slot_ready(&mem, ESP, REGS).eax, 1);
    mem.write32(0x2000 + 0x150, 2);
    assert_eq!(slot_ready(&mem, ESP, REGS).eax, 0);
    mem.write32(ESP + 4, 3);
    let record = 0x778800 + 3 * 0x68;
    mem.write32(record, 0b111);
    assert_eq!(
        record_68_clear_bit1_set_state(&mut mem, ESP, REGS).eax,
        record
    );
    assert_eq!(mem.read32(record), 0b101);
    assert_eq!(mem.read32(record + 0x40), 2);
}
