// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned XInput device enumeration contract (T3879 IN-3).
//!
//! This models the measured EMPTY backend and the explicitly synthetic pad policy from
//! `src/input/xinput_devices.c`. Guest memory stays owned by the host: callers attach it
//! through [`GuestMemory`] instead of keeping a second Rust copy of guest tables.

pub const DEVICE_TYPES: [u32; 6] = [
    0x0046_C6E0,
    0x0046_C75C,
    0x0046_C7C0,
    0x0046_C894,
    0x0046_C8A0,
    0x0046_C8AC,
];
pub const GAMEPAD_TYPE: u32 = 0x0046_C75C;
pub const STATUS_COUNT: u32 = 0x0077_1454;
pub const STATUS_FLAG: u32 = 0x0077_1370;

const EXPECTED_DECLARATIONS: [u32; 8] = [
    0x0046_C6E0,
    8,
    0x0046_C8A0,
    4,
    0x0046_C894,
    4,
    GAMEPAD_TYPE,
    4,
];

/// Host-owned access to guest bytes. Implementations must reject unmapped or read-only writes.
pub trait GuestMemory {
    fn read(&self, address: u32, output: &mut [u8]) -> bool;
    fn write(&mut self, address: u32, bytes: &[u8]) -> bool;
}

/// Current explicit device policy, refreshed by the host before each HLE call.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PadPolicy {
    pub connected_ports: u32,
    pub synthetic_enabled: bool,
    pub multiport_enabled: bool,
    pub removed: bool,
}

impl PadPolicy {
    fn synthetic_mask(self) -> Result<Option<u32>, DeviceError> {
        if !self.synthetic_enabled {
            return if self.connected_ports == 0 {
                Ok(None)
            } else {
                Err(DeviceError::SyntheticPadRequired)
            };
        }
        if self.multiport_enabled {
            return Ok(Some(self.connected_ports));
        }
        if self.removed && self.connected_ports == 0 {
            return Ok(Some(0));
        }
        if self.connected_ports == 1 {
            return Ok(Some(1));
        }
        Err(DeviceError::PortZeroSyntheticRequired)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceError {
    SyntheticPadRequired,
    PortZeroSyntheticRequired,
    RepeatedInitialization,
    WrongDeclarationCount,
    DeclarationMemoryUnavailable,
    DeclarationShapeNotMeasured,
    UnknownDeviceType,
    TableMemoryUnavailable,
    DeviceTableNotCold,
    InvalidMuMask,
    StatusMemoryUnavailable,
    DeclarationsAliasStatus,
    OutputMemoryUnavailable,
    OutputAliasesDeviceTable,
    OutputsOverlap,
    NotInitialized,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Initialization {
    pub became_ready: bool,
    pub synthetic_insertions: u32,
    pub diagnostics: Vec<&'static str>,
}

/// State that does not belong in guest memory. The adapter owns policy and one-time reports.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeviceHle {
    ready: bool,
    mu_presence: u32,
    announced: bool,
    pad_inserted_announced: bool,
}

impl DeviceHle {
    pub fn reset(&mut self) {
        let mu_presence = self.mu_presence;
        *self = Self {
            mu_presence,
            ..Self::default()
        };
    }

    pub fn set_mu_mask<M: GuestMemory>(
        &mut self,
        memory: &mut M,
        mask: u32,
    ) -> Result<(), DeviceError> {
        if mask & !0x000F_000F != 0 {
            return Err(DeviceError::InvalidMuMask);
        }
        let changed = self.mu_presence ^ mask;
        if self.ready && changed != 0 {
            let mut state = read_words(memory, DEVICE_TYPES[0])?;
            state[0] = mask;
            state[1] |= changed;
            write_words(memory, DEVICE_TYPES[0], state)?;
        }
        self.mu_presence = mask;
        Ok(())
    }

    pub fn is_ready(&self) -> bool {
        self.ready
    }

    /// Initialize only the measured cold four-declaration sequence. Every input is checked before writes begin.
    pub fn initialize<M: GuestMemory>(
        &mut self,
        memory: &mut M,
        count: u32,
        declarations: u32,
        policy: PadPolicy,
    ) -> Result<Initialization, DeviceError> {
        let synthetic_mask = policy.synthetic_mask()?;
        if self.ready {
            return Err(DeviceError::RepeatedInitialization);
        }
        if count != 4 {
            return Err(DeviceError::WrongDeclarationCount);
        }
        let mut declaration_bytes = [0u8; 32];
        if !memory.read(declarations, &mut declaration_bytes) {
            return Err(DeviceError::DeclarationMemoryUnavailable);
        }
        for (index, expected) in EXPECTED_DECLARATIONS.iter().enumerate() {
            let start = index * 4;
            if u32::from_le_bytes(declaration_bytes[start..start + 4].try_into().unwrap())
                != *expected
            {
                return Err(DeviceError::DeclarationShapeNotMeasured);
            }
        }

        for device_type in DEVICE_TYPES {
            let state = read_words(memory, device_type)?;
            if state != [0, 0, 0] {
                return Err(DeviceError::DeviceTableNotCold);
            }
        }
        let mut count_bytes = [0u8; 4];
        let mut flag_byte = [0u8; 1];
        if !memory.read(STATUS_COUNT, &mut count_bytes) || !memory.read(STATUS_FLAG, &mut flag_byte)
        {
            return Err(DeviceError::StatusMemoryUnavailable);
        }
        if overlaps(declarations, 32, STATUS_COUNT, 4) || overlaps(declarations, 32, STATUS_FLAG, 1)
        {
            return Err(DeviceError::DeclarationsAliasStatus);
        }

        if !memory.write(STATUS_COUNT, &0u32.to_le_bytes()) || !memory.write(STATUS_FLAG, &[0]) {
            return Err(DeviceError::StatusMemoryUnavailable);
        }
        self.ready = true;
        let mut result = Initialization {
            became_ready: true,
            ..Initialization::default()
        };
        if let Some(mask) = synthetic_mask {
            write_words(memory, GAMEPAD_TYPE, [mask, mask, 0])?;
            result.synthetic_insertions = mask;
            if !policy.multiport_enabled && !self.pad_inserted_announced {
                result.diagnostics.push(
                    "FABRICATED: synthetic gamepad reported inserted on port 0 exactly once; no USB or host input",
                );
                self.pad_inserted_announced = true;
            }
        }
        write_words(
            memory,
            DEVICE_TYPES[0],
            [self.mu_presence, self.mu_presence, 0],
        )?;
        if !self.announced {
            result.diagnostics.push(if policy.multiport_enabled {
                "explicit four-port adapter initialized; guest presence model, no IRQ equivalence"
            } else if synthetic_mask.is_some() {
                "explicit synthetic pad on port 0; other ports empty; no host input, OHCI, IRQ or heap objects"
            } else {
                "explicit EMPTY gamepad backend; MU presence is host policy; no host input, OHCI, IRQ or heap objects"
            });
            self.announced = true;
        }
        Ok(result)
    }

    pub fn get<M: GuestMemory>(
        &self,
        memory: &mut M,
        device_type: u32,
        policy: PadPolicy,
    ) -> Result<u32, DeviceError> {
        self.require_ready(policy)?;
        let mut state = read_words(memory, checked_type(device_type)?)?;
        state[1] = 0;
        state[2] = state[0];
        write_words(memory, device_type, state)?;
        Ok(state[0])
    }

    pub fn peek<M: GuestMemory>(
        &self,
        memory: &mut M,
        device_type: u32,
        previous: u32,
        reconnected: u32,
        policy: PadPolicy,
    ) -> Result<u32, DeviceError> {
        self.require_ready(policy)?;
        let state = read_words(memory, checked_type(device_type)?)?;
        let previous_at = output_at(memory, previous, true)?;
        let reconnected_at = output_at(memory, reconnected, true)?;
        check_output_pair(previous, previous_at, reconnected, reconnected_at)?;
        let reconnect = state[1] & state[2] & state[0];
        if previous_at {
            write_word(memory, previous, state[2])?;
        }
        if reconnected_at {
            write_word(memory, reconnected, reconnect)?;
        }
        Ok(state[0])
    }

    pub fn changes<M: GuestMemory>(
        &self,
        memory: &mut M,
        device_type: u32,
        insertions: u32,
        removals: u32,
        policy: PadPolicy,
    ) -> Result<bool, DeviceError> {
        self.require_ready(policy)?;
        let device_type = checked_type(device_type)?;
        let mut state = read_words(memory, device_type)?;
        let insertions_at = output_at(memory, insertions, false)?;
        let removals_at = output_at(memory, removals, false)?;
        check_output_pair(insertions, insertions_at, removals, removals_at)?;
        let (added, removed) = if state[1] == 0 {
            (0, 0)
        } else {
            let reconnect = state[1] & state[2] & state[0];
            let added = (!state[2] & state[0]) | reconnect;
            let removed = (!state[0] & state[2]) | reconnect;
            state[1] = 0;
            state[2] = state[0];
            (added, removed)
        };
        write_word(memory, insertions, added)?;
        write_word(memory, removals, removed)?;
        write_words(memory, device_type, state)?;
        Ok(added | removed != 0)
    }

    pub fn enumeration_status<M: GuestMemory>(
        &self,
        memory: &M,
        policy: PadPolicy,
    ) -> Result<bool, DeviceError> {
        self.require_ready(policy)?;
        let mut count_bytes = [0u8; 4];
        let mut flag = [0u8; 1];
        if !memory.read(STATUS_COUNT, &mut count_bytes) || !memory.read(STATUS_FLAG, &mut flag) {
            return Err(DeviceError::StatusMemoryUnavailable);
        }
        Ok(u32::from_le_bytes(count_bytes) != 0 || flag[0] != 0)
    }

    fn require_ready(&self, policy: PadPolicy) -> Result<(), DeviceError> {
        policy.synthetic_mask()?;
        if self.ready {
            Ok(())
        } else {
            Err(DeviceError::NotInitialized)
        }
    }
}

fn checked_type(device_type: u32) -> Result<u32, DeviceError> {
    DEVICE_TYPES
        .contains(&device_type)
        .then_some(device_type)
        .ok_or(DeviceError::UnknownDeviceType)
}

fn read_words<M: GuestMemory>(memory: &M, address: u32) -> Result<[u32; 3], DeviceError> {
    let mut bytes = [0u8; 12];
    if !memory.read(address, &mut bytes) {
        return Err(DeviceError::TableMemoryUnavailable);
    }
    Ok(std::array::from_fn(|index| {
        let start = index * 4;
        u32::from_le_bytes(bytes[start..start + 4].try_into().unwrap())
    }))
}

fn write_words<M: GuestMemory>(
    memory: &mut M,
    address: u32,
    words: [u32; 3],
) -> Result<(), DeviceError> {
    let mut bytes = [0u8; 12];
    for (index, word) in words.into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&word.to_le_bytes());
    }
    if memory.write(address, &bytes) {
        Ok(())
    } else {
        Err(DeviceError::TableMemoryUnavailable)
    }
}

fn write_word<M: GuestMemory>(memory: &mut M, address: u32, word: u32) -> Result<(), DeviceError> {
    if memory.write(address, &word.to_le_bytes()) {
        Ok(())
    } else {
        Err(DeviceError::OutputMemoryUnavailable)
    }
}

fn output_at<M: GuestMemory>(
    memory: &M,
    address: u32,
    optional: bool,
) -> Result<bool, DeviceError> {
    if address == 0 && optional {
        return Ok(false);
    }
    let mut bytes = [0u8; 4];
    if !memory.read(address, &mut bytes) {
        return Err(DeviceError::OutputMemoryUnavailable);
    }
    if DEVICE_TYPES
        .iter()
        .any(|device_type| overlaps(address, 4, *device_type, 12))
    {
        return Err(DeviceError::OutputAliasesDeviceTable);
    }
    Ok(true)
}

fn check_output_pair(
    first_address: u32,
    first_present: bool,
    second_address: u32,
    second_present: bool,
) -> Result<(), DeviceError> {
    if first_present && second_present && overlaps(first_address, 4, second_address, 4) {
        Err(DeviceError::OutputsOverlap)
    } else {
        Ok(())
    }
}

fn overlaps(a: u32, size_a: u32, b: u32, size_b: u32) -> bool {
    u64::from(a) < u64::from(b) + u64::from(size_b)
        && u64::from(b) < u64::from(a) + u64::from(size_a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Memory(BTreeMap<u32, u8>);

    impl Memory {
        fn map(&mut self, address: u32, bytes: &[u8]) {
            for (offset, byte) in bytes.iter().copied().enumerate() {
                self.0.insert(address + offset as u32, byte);
            }
        }

        fn word(&self, address: u32) -> u32 {
            let bytes: [u8; 4] = std::array::from_fn(|i| self.0[&(address + i as u32)]);
            u32::from_le_bytes(bytes)
        }
    }

    impl GuestMemory for Memory {
        fn read(&self, address: u32, output: &mut [u8]) -> bool {
            for (i, byte) in output.iter_mut().enumerate() {
                let Some(value) = self.0.get(&(address + i as u32)) else {
                    return false;
                };
                *byte = *value;
            }
            true
        }

        fn write(&mut self, address: u32, bytes: &[u8]) -> bool {
            if !(0..bytes.len()).all(|offset| self.0.contains_key(&(address + offset as u32))) {
                return false;
            }
            for (offset, byte) in bytes.iter().copied().enumerate() {
                self.0.insert(address + offset as u32, byte);
            }
            true
        }
    }

    fn cold_memory() -> Memory {
        let mut memory = Memory::default();
        for device_type in DEVICE_TYPES {
            memory.map(device_type, &[0; 12]);
        }
        let mut declarations = [0u8; 32];
        for (index, word) in EXPECTED_DECLARATIONS.iter().copied().enumerate() {
            declarations[index * 4..index * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        memory.map(0x1000, &declarations);
        memory.map(STATUS_COUNT, &19u32.to_le_bytes());
        memory.map(STATUS_FLAG, &[1]);
        memory
    }

    #[test]
    fn cold_empty_init_normalizes_status_and_preserves_explicit_mu_policy() {
        let mut memory = cold_memory();
        let mut hle = DeviceHle::default();
        hle.set_mu_mask(&mut memory, 5).unwrap();
        let report = hle
            .initialize(&mut memory, 4, 0x1000, PadPolicy::default())
            .unwrap();
        assert!(report.became_ready);
        assert_eq!(report.synthetic_insertions, 0);
        assert_eq!(memory.word(STATUS_COUNT), 0);
        assert_eq!(memory.word(DEVICE_TYPES[0]), 5);
        assert_eq!(memory.word(DEVICE_TYPES[0] + 4), 5);
        assert_eq!(memory.word(GAMEPAD_TYPE), 0);
        assert!(hle.is_ready());
    }

    #[test]
    fn synthetic_port_zero_is_announced_once_and_initializes_one_change() {
        let mut memory = cold_memory();
        let mut hle = DeviceHle::default();
        let policy = PadPolicy {
            connected_ports: 1,
            synthetic_enabled: true,
            ..PadPolicy::default()
        };
        let first = hle.initialize(&mut memory, 4, 0x1000, policy).unwrap();
        assert_eq!(first.synthetic_insertions, 1);
        assert_eq!(
            [
                memory.word(GAMEPAD_TYPE),
                memory.word(GAMEPAD_TYPE + 4),
                memory.word(GAMEPAD_TYPE + 8)
            ],
            [1, 1, 0]
        );
        assert!(
            first
                .diagnostics
                .iter()
                .any(|line| line.contains("FABRICATED"))
        );
        hle.reset();
        let mut memory = cold_memory();
        let second = hle.initialize(&mut memory, 4, 0x1000, policy).unwrap();
        assert_eq!(
            second
                .diagnostics
                .iter()
                .filter(|line| line.contains("FABRICATED"))
                .count(),
            1
        );
    }

    #[test]
    fn get_peek_and_changes_preserve_the_measured_mask_rules() {
        let mut memory = cold_memory();
        let mut hle = DeviceHle::default();
        hle.initialize(&mut memory, 4, 0x1000, PadPolicy::default())
            .unwrap();
        memory.map(0x2000, &0u32.to_le_bytes());
        memory.map(0x2010, &0u32.to_le_bytes());
        memory.map(0x2020, &0u32.to_le_bytes());
        memory.map(GAMEPAD_TYPE, &[3, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0]);
        assert_eq!(
            hle.peek(
                &mut memory,
                GAMEPAD_TYPE,
                0x2000,
                0x2010,
                PadPolicy::default()
            ),
            Ok(3)
        );
        assert_eq!(memory.word(0x2000), 1);
        assert_eq!(memory.word(0x2010), 1);
        assert!(
            hle.changes(
                &mut memory,
                GAMEPAD_TYPE,
                0x2000,
                0x2020,
                PadPolicy::default()
            )
            .unwrap()
        );
        assert_eq!(memory.word(0x2000), 3);
        assert_eq!(memory.word(0x2020), 1);
        assert_eq!(
            [
                memory.word(GAMEPAD_TYPE),
                memory.word(GAMEPAD_TYPE + 4),
                memory.word(GAMEPAD_TYPE + 8)
            ],
            [3, 0, 3]
        );
    }

    #[test]
    fn mu_mask_changes_accumulate_until_guest_consumption() {
        let mut memory = cold_memory();
        let mut hle = DeviceHle::default();
        hle.set_mu_mask(&mut memory, 5).unwrap();
        hle.initialize(&mut memory, 4, 0x1000, PadPolicy::default())
            .unwrap();
        assert_eq!(
            hle.get(&mut memory, DEVICE_TYPES[0], PadPolicy::default()),
            Ok(5)
        );
        memory.map(0x2000, &0u32.to_le_bytes());
        memory.map(0x2010, &0u32.to_le_bytes());

        hle.set_mu_mask(&mut memory, 0).unwrap();
        assert_eq!(
            [
                memory.word(DEVICE_TYPES[0]),
                memory.word(DEVICE_TYPES[0] + 4),
                memory.word(DEVICE_TYPES[0] + 8)
            ],
            [0, 5, 5]
        );
        assert_eq!(
            hle.changes(
                &mut memory,
                DEVICE_TYPES[0],
                0x2000,
                0x2010,
                PadPolicy::default()
            ),
            Ok(true)
        );
        assert_eq!(memory.word(0x2000), 0);
        assert_eq!(memory.word(0x2010), 5);
        assert_eq!(memory.word(DEVICE_TYPES[0] + 4), 0);
    }

    #[test]
    fn mu_mask_rejects_unmeasured_bits_without_changing_host_policy() {
        let mut memory = cold_memory();
        let mut hle = DeviceHle::default();
        assert_eq!(
            hle.set_mu_mask(&mut memory, 0x10),
            Err(DeviceError::InvalidMuMask)
        );
        hle.initialize(&mut memory, 4, 0x1000, PadPolicy::default())
            .unwrap();
        assert_eq!(memory.word(DEVICE_TYPES[0]), 0);
    }

    #[test]
    fn init_refuses_unmeasured_declarations_without_mutating_status() {
        let mut memory = cold_memory();
        memory.map(0x1000, &[0; 32]);
        let mut hle = DeviceHle::default();
        assert_eq!(
            hle.initialize(&mut memory, 4, 0x1000, PadPolicy::default()),
            Err(DeviceError::DeclarationShapeNotMeasured)
        );
        assert_eq!(memory.word(STATUS_COUNT), 19);
        assert!(!hle.is_ready());
    }
}
