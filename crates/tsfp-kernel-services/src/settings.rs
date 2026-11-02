// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{GuestMemory, INVALID_PARAMETER, SUCCESS, crypto};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UnknownPolicy {
    #[default]
    FabricatedZeros,
    Fail,
}
#[derive(Clone, Debug)]
struct Setting {
    bytes: Vec<u8>,
    source: bool,
    status: u32,
}
#[derive(Debug, Default)]
pub struct Settings {
    values: BTreeMap<u32, Setting>,
    pub policy: UnknownPolicy,
    pub queried: Vec<u32>,
    /// C increments again for an unrecorded index after the 64-slot log fills.
    pub queried_total: u32,
    pub fabricated: u32,
    pub refused: u32,
    pub served: u32,
    keys: Option<([u8; 16], [u8; 16])>,
}
impl Settings {
    pub fn set(&mut self, index: u32, bytes: &[u8]) -> bool {
        if bytes.len() > 256 || (!self.values.contains_key(&index) && self.values.len() == 32) {
            return false;
        }
        if index == 0xffff {
            self.keys = None;
        }
        self.values.insert(
            index,
            Setting {
                bytes: bytes.to_vec(),
                source: false,
                status: SUCCESS,
            },
        );
        true
    }
    pub fn count(&self) -> usize {
        self.values.len()
    }
    pub fn eeprom_available(&self) -> bool {
        self.values.get(&0xffff).is_some_and(|s| s.source)
    }
    pub fn key(&self, ordinal: u32) -> Option<&[u8; 16]> {
        let (eeprom, hd) = self.keys.as_ref()?;
        match ordinal {
            321 => Some(eeprom),
            323 => Some(hd),
            _ => None,
        }
    }
    pub fn set_eeprom(&mut self, bytes: &[u8; 256], key: Option<&[u8; 16]>) -> bool {
        let keys = match key {
            None => None,
            Some(key) => {
                let digest = crypto::hmac(key, &bytes[..20], &[], true);
                // Decryption uses the exact RC4 model through a private bounded buffer.
                let mut memory = EepromMemory {
                    bytes: vec![0; 320],
                };
                memory.bytes[1..21].copy_from_slice(&digest);
                memory.bytes[280..308].copy_from_slice(&bytes[20..48]);
                if crypto::rc4_key(&mut memory, 22, 20, 1) != SUCCESS
                    || crypto::rc4_crypt(&mut memory, 22, 28, 280) != SUCCESS
                {
                    return false;
                }
                let plain = &memory.bytes[280..308];
                let check = crypto::hmac(key, plain, &[], true);
                if check
                    .iter()
                    .zip(bytes)
                    .fold(0u8, |diff, (a, b)| diff | (a ^ b))
                    != 0
                {
                    return false;
                }
                Some((*key, plain[8..24].try_into().unwrap()))
            }
        };
        let needed = [0xffff, 0x101]
            .iter()
            .filter(|i| !self.values.contains_key(i))
            .count();
        if self.values.len() + needed > 32 {
            return false;
        }
        let mut sum = 0u32;
        let mut carries = 0u32;
        for word in bytes[0x30..0x60].chunks_exact(4) {
            let (next, overflow) =
                sum.overflowing_add(u32::from_le_bytes(word.try_into().unwrap()));
            sum = next;
            carries += u32::from(overflow);
        }
        let (next, overflow) = sum.overflowing_add(carries);
        sum = next.wrapping_add(u32::from(overflow));
        self.values.insert(
            0xffff,
            Setting {
                bytes: bytes.to_vec(),
                source: true,
                status: SUCCESS,
            },
        );
        self.values.insert(
            0x101,
            Setting {
                bytes: bytes[64..70].to_vec(),
                source: true,
                status: if sum == u32::MAX { SUCCESS } else { 0xc000009c },
            },
        );
        self.keys = keys;
        true
    }
    fn record(&mut self, index: u32) {
        if !self.queried.contains(&index) {
            if self.queried.len() < 64 {
                self.queried.push(index);
            }
            self.queried_total = self.queried_total.wrapping_add(1);
        }
    }
    /// Returns status and whether this call fabricated an answer.
    pub fn query(
        &mut self,
        args: &[u32],
        stored_only: bool,
        memory: &mut dyn GuestMemory,
    ) -> (u32, bool) {
        let [index, type_out, value, capacity, length_out] = args[..5].try_into().unwrap();
        if self.values.get(&index).is_some_and(|s| s.source) {
            self.record(index);
            let stored = &self.values[&index];
            if stored.status != SUCCESS {
                return (stored.status, false);
            }
            let length = stored.bytes.len() as u32;
            let whole = index == 0xffff;
            if whole && length_out != 0 && !memory.put_word(length_out, 256) {
                return (INVALID_PARAMETER, false);
            }
            if capacity < length {
                return (0xc0000023, false);
            }
            if !memory.put_word(type_out, 3) {
                return (INVALID_PARAMETER, false);
            }
            if !whole && length_out != 0 && !memory.put_word(length_out, 6) {
                return (INVALID_PARAMETER, false);
            }
            if !whole
                && (u64::from(value) + u64::from(capacity) > u64::from(u32::MAX)
                    || !write_value(memory, value, None, capacity))
            {
                return (INVALID_PARAMETER, false);
            }
            if !write_value(memory, value, Some(&stored.bytes), length) {
                return (INVALID_PARAMETER, false);
            }
            self.served = self.served.wrapping_add(1);
            return (SUCCESS, false);
        }
        if value == 0 || capacity == 0 || capacity > 256 {
            return (INVALID_PARAMETER, false);
        }
        self.record(index);
        let fabricated;
        let (produced, source) = if let Some(stored) = self.values.get(&index) {
            if stored.bytes.len() > capacity as usize {
                return (0xc0000023, false);
            }
            self.served = self.served.wrapping_add(1);
            fabricated = false;
            (stored.bytes.len() as u32, Some(stored.bytes.as_slice()))
        } else if stored_only || self.policy == UnknownPolicy::Fail {
            self.refused = self.refused.wrapping_add(1);
            return (0xc0000034, false);
        } else {
            self.fabricated = self.fabricated.wrapping_add(1);
            fabricated = true;
            (capacity, None)
        };
        if !write_value(memory, value, source, produced)
            || (type_out != 0 && !memory.put_word(type_out, if produced == 4 { 4 } else { 3 }))
            || (length_out != 0 && !memory.put_word(length_out, produced))
        {
            return (INVALID_PARAMETER, fabricated);
        }
        (SUCCESS, fabricated)
    }
}
fn write_value(
    memory: &mut dyn GuestMemory,
    destination: u32,
    source: Option<&[u8]>,
    length: u32,
) -> bool {
    for i in 0..length {
        if !memory.write(
            destination.wrapping_add(i),
            &[source.map_or(0, |s| s[i as usize])],
        ) {
            return false;
        }
    }
    true
}
struct EepromMemory {
    bytes: Vec<u8>,
}
impl GuestMemory for EepromMemory {
    fn readable(&self, address: u32, length: usize) -> bool {
        address != 0
            && (address as usize)
                .checked_add(length)
                .is_some_and(|e| e <= self.bytes.len())
    }
    fn read(&self, address: u32, output: &mut [u8]) -> bool {
        if !self.readable(address, output.len()) {
            return false;
        }
        output.copy_from_slice(&self.bytes[address as usize..address as usize + output.len()]);
        true
    }
    fn write(&mut self, address: u32, input: &[u8]) -> bool {
        if !self.readable(address, input.len()) {
            return false;
        }
        self.bytes[address as usize..address as usize + input.len()].copy_from_slice(input);
        true
    }
}
