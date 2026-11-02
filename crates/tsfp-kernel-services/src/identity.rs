// SPDX-License-Identifier: GPL-3.0-or-later
use crate::GuestMemory;

/// FABRICATED retail claim matching the C host, never hardware evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleIdentity {
    pub flags: u32,
    pub gpu_revision: u8,
    pub mcp_revision: u8,
    pub version: [u16; 4],
}
impl Default for ConsoleIdentity {
    fn default() -> Self {
        Self {
            flags: 0,
            gpu_revision: 0xa2,
            mcp_revision: 0xd1,
            version: [1, 0, 5849, 1],
        }
    }
}
impl ConsoleIdentity {
    pub fn publish(&self, memory: &mut dyn GuestMemory, hardware: u32, version: u32) -> bool {
        let mut hw = [0; 8];
        hw[..4].copy_from_slice(&self.flags.to_le_bytes());
        hw[4] = self.gpu_revision;
        hw[5] = self.mcp_revision;
        let mut krnl = [0; 8];
        for (part, value) in krnl.chunks_exact_mut(2).zip(self.version) {
            part.copy_from_slice(&value.to_le_bytes());
        }
        memory.write(hardware, &hw) && memory.write(version, &krnl)
    }
}

/// Actual backing device bytes. No fabricated fallback or pathname-derived identity.
///
/// `publish` places two eight-byte guest STRING descriptors at `model` and `serial`.
/// Their payloads occupy a 64-byte region at `storage`: up to 40 model bytes,
/// then up to 20 serial bytes. The rest of those fields is space padded.
/// The caller keeps that guest storage alive while the descriptors are used.
///
/// ```
/// use tsfp_kernel_services::identity::DiskIdentity;
/// let disk = DiskIdentity {
///     model: b"Drive".to_vec(),
///     serial: b"123".to_vec(),
///     device: (0, 0),
/// };
/// assert_eq!(disk.model.len(), 5);
/// assert_eq!(disk.serial.len(), 3);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskIdentity {
    pub model: Vec<u8>,
    pub serial: Vec<u8>,
    pub device: (u32, u32),
}
fn range(address: u32, length: usize) -> bool {
    address != 0 && u64::from(address) + length as u64 <= 0x1_0000_0000
}
fn overlap(a: u32, na: usize, b: u32, nb: usize) -> bool {
    u64::from(a) < u64::from(b) + nb as u64 && u64::from(b) < u64::from(a) + na as u64
}
fn descriptor(length: usize, maximum: u16, address: u32) -> [u8; 8] {
    let mut out = [0; 8];
    out[..2].copy_from_slice(&(length as u16).to_le_bytes());
    out[2..4].copy_from_slice(&maximum.to_le_bytes());
    out[4..].copy_from_slice(&address.to_le_bytes());
    out
}
impl DiskIdentity {
    pub fn publish(
        &self,
        memory: &mut dyn GuestMemory,
        model: u32,
        serial: u32,
        storage: u32,
    ) -> bool {
        if self.model.is_empty()
            || self.model.len() > 40
            || self.serial.is_empty()
            || self.serial.len() > 20
            || !range(model, 8)
            || !range(serial, 8)
            || !range(storage, 64)
            || overlap(model, 8, serial, 8)
            || overlap(model, 8, storage, 64)
            || overlap(serial, 8, storage, 64)
        {
            return false;
        }
        let mut payload = [0; 64];
        payload[..60].fill(b' ');
        payload[..self.model.len()].copy_from_slice(&self.model);
        payload[40..40 + self.serial.len()].copy_from_slice(&self.serial);
        memory.write(storage, &payload)
            && memory.write(model, &descriptor(self.model.len(), 40, storage))
            && memory.write(serial, &descriptor(self.serial.len(), 20, storage + 40))
    }
    pub fn read(memory: &dyn GuestMemory, model: u32, serial: u32) -> Option<Self> {
        fn string(memory: &dyn GuestMemory, address: u32, cap: usize) -> Option<Vec<u8>> {
            let mut desc = [0; 8];
            if !memory.read(address, &mut desc) {
                return None;
            }
            let length = u16::from_le_bytes(desc[..2].try_into().unwrap()) as usize;
            let maximum = u16::from_le_bytes(desc[2..4].try_into().unwrap()) as usize;
            let address = u32::from_le_bytes(desc[4..].try_into().unwrap());
            if length == 0 || length > cap || maximum < length || !range(address, length) {
                return None;
            }
            let mut out = vec![0; length];
            memory.read(address, &mut out).then_some(out)
        }
        Some(Self {
            model: string(memory, model, 40)?,
            serial: string(memory, serial, 20)?,
            device: (0, 0),
        })
    }
}
