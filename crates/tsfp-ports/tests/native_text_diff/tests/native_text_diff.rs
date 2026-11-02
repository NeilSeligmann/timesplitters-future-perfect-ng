// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 differential for the Rust-owned bank lookup against lifted `0x3D7B0`.
//! Inputs are fabricated banks; no original strings or binary data are included.

use std::collections::BTreeMap;

use tsfp_format_bs::Bank;
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, Xmm},
    c2r::data_text::game_text_string_get_by_id_from_bank,
    memory::GuestMemory,
};
use tsfp_ui::native_text::{LabelId, STORY_FIRST_ID, TextBanks, TextLookup};

const COMMON_BASE: u32 = 0x0100_0000;
const STORY_BASE: u32 = 0x0102_0000;
const MISSING_BASE: u32 = 0x0104_0000;
const EMPTY_TEXT: u32 = 0x0047_66d8;
const MISSING_TEXT_GLOBAL: u32 = 0x004c_0a44;
const COMMON_BANK_GLOBAL: u32 = 0x006b_7b10;
const STORY_BANK_GLOBAL: u32 = 0x006b_7b08;

#[derive(Default)]
struct Memory(BTreeMap<u32, u8>);

impl Memory {
    fn write_bytes(&mut self, address: u32, bytes: &[u8]) {
        for (offset, byte) in bytes.iter().copied().enumerate() {
            self.write8(address.wrapping_add(offset as u32), byte);
        }
    }

    fn c_string(&self, address: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut at = address;
        loop {
            let byte = self.read8(at);
            if byte == 0 {
                return bytes;
            }
            bytes.push(byte);
            at = at.wrapping_add(1);
        }
    }
}

impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        self.0.get(&address).copied().unwrap_or(0)
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
}

fn synthetic_bank(tag: u8, count: usize) -> (Vec<u8>, Bank) {
    let table_bytes = count * 4;
    let mut raw = vec![0; table_bytes];
    for index in 0..count {
        let offset = raw.len() as u32;
        raw[index * 4..index * 4 + 4].copy_from_slice(&offset.to_le_bytes());
        raw.extend([tag, b'0' + ((index / 1000) % 10) as u8]);
        raw.extend(format!("{index:04}").bytes());
        raw.push(0x80 + (index % 0x7f) as u8);
        raw.push(0);
    }
    let parsed =
        Bank::parse(&raw, count).expect("synthetic bank has valid offsets and terminators");
    (raw, parsed)
}

fn install_bank(memory: &mut Memory, base: u32, raw: &[u8], count: usize) {
    let table_bytes = count * 4;
    memory.write_bytes(base + table_bytes as u32, &raw[table_bytes..]);
    for index in 0..count {
        let file_offset = u32::from_le_bytes(
            raw[index * 4..index * 4 + 4]
                .try_into()
                .expect("four-byte offset"),
        );
        memory.write32(base + (index * 4) as u32, base + file_offset);
    }
}

fn registers(memory: &mut Memory, id: u32) -> ExtRegisters {
    let esp = 0x0008_0000;
    memory.write32(esp + 4, id);
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp,
        ebx: 0,
        ebp: 0,
        esi: 0,
        edi: 0,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0x7f,
        },
        xmm: [Xmm { lanes: [0; 4] }; 5],
        xmm_hi: [Xmm { lanes: [0; 4] }; 3],
    }
}

#[test]
fn owned_lookup_matches_lifted_original_across_both_banks() {
    const COMMON_COUNT: usize = STORY_FIRST_ID as usize;
    const STORY_COUNT: usize = 4149;

    let (common_raw, common) = synthetic_bank(b'C', COMMON_COUNT);
    let (story_raw, story) = synthetic_bank(b'S', STORY_COUNT);
    let model = TextBanks::new(
        Some(common),
        Some(story),
        b"synthetic missing string".to_vec(),
    );
    let mut memory = Memory::default();
    install_bank(&mut memory, COMMON_BASE, &common_raw, COMMON_COUNT);
    install_bank(&mut memory, STORY_BASE, &story_raw, STORY_COUNT);
    memory.write32(COMMON_BANK_GLOBAL, COMMON_BASE);
    memory.write32(STORY_BANK_GLOBAL, STORY_BASE);
    memory.write32(MISSING_TEXT_GLOBAL, MISSING_BASE);
    memory.write_bytes(MISSING_BASE, b"synthetic missing string\0");

    let mut ids = Vec::with_capacity(COMMON_COUNT + STORY_COUNT);
    ids.push(0);
    ids.extend(1..STORY_FIRST_ID);
    ids.extend(STORY_FIRST_ID..STORY_FIRST_ID + STORY_COUNT as u32);

    for id in ids {
        let expected = model
            .lookup(LabelId::new(id))
            .expect("generated ID is in range");
        let mut regs = registers(&mut memory, id);
        game_text_string_get_by_id_from_bank(&mut memory, &mut regs);
        let actual = memory.c_string(regs.eax);
        assert_eq!(actual, expected.bytes(), "label ID {id:#x}");
        if id == 0 {
            assert_eq!(regs.eax, EMPTY_TEXT);
            assert_eq!(expected, TextLookup::Empty);
        }
    }
}

#[test]
fn owned_lookup_matches_lifted_missing_bank_precedence() {
    let (common_raw, common) = synthetic_bank(b'C', STORY_FIRST_ID as usize);
    let mut memory = Memory::default();
    install_bank(
        &mut memory,
        COMMON_BASE,
        &common_raw,
        STORY_FIRST_ID as usize,
    );
    memory.write32(COMMON_BANK_GLOBAL, COMMON_BASE);
    memory.write32(STORY_BANK_GLOBAL, 0);
    memory.write32(MISSING_TEXT_GLOBAL, MISSING_BASE);
    memory.write_bytes(MISSING_BASE, b"missing\0");

    let no_story = TextBanks::new(Some(common.clone()), None, b"missing".to_vec());
    for id in [0, 1, STORY_FIRST_ID] {
        let expected = no_story.lookup(LabelId::new(id)).unwrap();
        let mut regs = registers(&mut memory, id);
        game_text_string_get_by_id_from_bank(&mut memory, &mut regs);
        assert_eq!(regs.eax, MISSING_BASE, "story bank gates id {id:#x}");
        assert_eq!(expected, TextLookup::Missing(b"missing"));
        assert_eq!(memory.c_string(regs.eax), expected.bytes());
    }

    let (_story_raw, story) = synthetic_bank(b'S', 1);
    let no_common = TextBanks::new(None, Some(story), b"missing".to_vec());
    memory.write32(STORY_BANK_GLOBAL, STORY_BASE);
    memory.write32(COMMON_BANK_GLOBAL, 0);
    for (id, expected_model) in [(0, TextLookup::Empty), (1, TextLookup::Missing(b"missing"))] {
        let expected = no_common.lookup(LabelId::new(id)).unwrap();
        assert_eq!(expected, expected_model);
        let mut regs = registers(&mut memory, id);
        game_text_string_get_by_id_from_bank(&mut memory, &mut regs);
        if id == 0 {
            assert_eq!(regs.eax, EMPTY_TEXT);
        } else {
            assert_eq!(regs.eax, MISSING_BASE);
        }
        assert_eq!(memory.c_string(regs.eax), expected.bytes());
    }
}
