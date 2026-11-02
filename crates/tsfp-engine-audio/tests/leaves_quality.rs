use std::collections::BTreeMap;
use tsfp_engine_audio::{gprs::Gprs, leaves, memory::GuestMemory};

#[derive(Default)]
struct Words(BTreeMap<u32, u8>);

impl GuestMemory for Words {
    fn read8(&self, address: u32) -> u8 {
        *self.0.get(&address).unwrap_or(&0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
}

#[test]
fn mask_bit_and_pair_equality_grids() {
    let mut memory = Words::default();
    let entry = Gprs {
        eax: 17,
        ecx: 19,
        edx: 23,
    };
    for mask in [0, 1, 0x8000_0000, 0xa5a5_5a5a, u32::MAX] {
        memory.write32(0x004c_b54c, mask);
        for bit in 0..64 {
            let out = leaves::global_mask_bit_test(&memory, bit, entry);
            assert_eq!(
                out,
                Gprs {
                    eax: ((mask >> (bit & 31)) & 1),
                    ecx: bit,
                    edx: 23
                }
            );
        }
    }
    for left in [0, 1, u32::MAX, 0x8000_0000] {
        for right in [0, 1, u32::MAX, 0x8000_0000] {
            memory.write32(0x1000, left);
            memory.write32(0x1004, right);
            let out = leaves::pair_cursor_equal(&memory, 0x1000, entry);
            assert_eq!(
                out,
                Gprs {
                    eax: (left == right) as u32,
                    ecx: 0x1000,
                    edx: 23
                }
            );
        }
    }
}

#[test]
fn fallback_only_for_empty_special_entry() {
    let mut memory = Words::default();
    let entry = Gprs {
        edx: 23,
        ..Gprs::default()
    };
    for index in [0, 1, 0x23, 0x24, 0x25] {
        for word in [0, 0x1234_5678] {
            let address = 0x0053_a3ccu32.wrapping_add(index * 48);
            memory.write32(address, word);
            for flag in [0, 1, u32::MAX] {
                let expected = if index == 0x24 && word == 0 {
                    if flag == 0 { 0x0053_9b08 } else { 0x0053_a2c8 }
                } else {
                    word
                };
                assert_eq!(
                    leaves::table_special_fallback(&memory, index, flag, entry),
                    Gprs {
                        eax: expected,
                        ecx: index,
                        edx: 23
                    }
                );
            }
        }
    }
}

#[test]
fn first_matching_key_clears_only_selected_slot() {
    let entry = Gprs {
        eax: 19,
        ecx: 7,
        edx: 23,
    };
    for first_table in [0, 1, 2] {
        for slot in [0, 1, 19, 39] {
            let mut memory = Words::default();
            for i in 0..40 {
                memory.write32(0x0058_35f0 + i * 4, 8);
                memory.write32(0x0058_18e8 + i * 4, 8);
                memory.write32(0x0056_5a00 + i * 4, 99);
                memory.write32(0x0056_5aa0 + i * 4, 99);
            }
            if first_table < 2 {
                let key_base = if first_table == 0 {
                    0x0058_35f0
                } else {
                    0x0058_18e8
                };
                memory.write32(key_base + slot * 4, 7);
            }
            let out = leaves::clear_matching_key(&mut memory, entry);
            assert_eq!(
                out,
                Gprs {
                    eax: if first_table == 2 { 40 } else { slot },
                    ecx: 7,
                    edx: 0
                }
            );
            for i in 0..40 {
                assert_eq!(
                    memory.read32(0x0056_5a00 + i * 4),
                    if first_table == 0 && i == slot { 0 } else { 99 }
                );
                assert_eq!(
                    memory.read32(0x0056_5aa0 + i * 4),
                    if first_table == 1 && i == slot { 0 } else { 99 }
                );
            }
        }
    }
}
