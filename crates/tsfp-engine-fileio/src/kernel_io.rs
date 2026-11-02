//! NT byte-transfer flow, retaining C's original preset (T2210).
//! Backing policy remains in the existing fileio contracts, not duplicated here.
use crate::async_io::{Read, Volume};
pub const INVALID_PARAMETER: u32 = 0xc000_000d;
pub const INVALID_HANDLE: u32 = 0xc000_0008;
pub const TYPE_MISMATCH: u32 = 0xc000_0024;
pub const UNSUCCESSFUL: u32 = 0xc000_0001;
pub const EOF: u32 = 0xc000_0011;
pub const NOT_IMPLEMENTED: u32 = 0xc000_0002;
pub const NO_MEMORY: u32 = 0xc000_0017;
pub const RESOURCES: u32 = 0xc000_009a;
#[derive(Clone, Copy, Debug)]
pub struct File {
    pub offset: u64,
    pub options: u32,
    pub size: u64,
    pub cache_partition: u32,
    pub fatx: bool,
    pub directory: bool,
    pub host_dir: bool,
    pub disc_file: bool,
    pub disc_sector: u32,
    pub volume: Volume,
}
#[derive(Clone, Copy, Debug)]
pub struct Transfer {
    pub file: u32,
    pub event: u32,
    pub apc: u32,
    pub iosb: u32,
    pub buffer: u32,
    pub length: u32,
    pub offset_pointer: u32,
}
impl From<[u32; 8]> for Transfer {
    fn from(a: [u32; 8]) -> Self {
        Self {
            file: a[0],
            event: a[1],
            apc: a[2],
            iosb: a[4],
            buffer: a[5],
            length: a[6],
            offset_pointer: a[7],
        }
    }
}
/// The C backing's boolean is distinct from its status, including failure with
/// partial progress. Keep both rather than infer success from a zero status.
#[derive(Clone, Copy, Debug)]
pub struct BackingWrite {
    pub complete: bool,
    pub status: u32,
    pub transferred: u32,
}
#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum Account {
    Read = 0,
    Write = 1,
    ApcIgnored = 2,
    WriteRefused = 3,
    FabricatedGeometry = 4,
    FabricatedIdentity = 5,
    VolumeRefused = 6,
    UnknownVolumeClass = 7,
    ControlAnswered = 8,
    ControlRefused = 9,
    Flush = 10,
    UnknownFileClass = 11,
    DirectoryEntry = 12,
    FabricatedTimestamp = 13,
    ReadPending = 14,
}
#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum Diagnostic {
    ApcRefused = 0,
    QueueRefused = 1,
    VolumeRefused = 2,
    VolumeFabricated = 3,
    ControlRefused = 4,
    FileClassRefused = 5,
    TimestampsFabricated = 6,
    DirectoryAsyncRefused = 7,
    ApcIgnored = 8,
    WriteEventIgnored = 9,
}
/// Existing fileio is the storage boundary, shared with the hybrid C oracle.
/// Event writes are intentionally absent from NtWriteFile's original preset.
#[derive(Clone, Copy, Debug)]
pub struct Space {
    pub total: u64,
    pub available: u64,
    pub sectors: u32,
    pub sector_bytes: u32,
}
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct Metadata {
    pub size: u64,
    pub creation: u64,
    pub access: u64,
    pub write: u64,
    pub directory: bool,
    pub host_dir: bool,
    pub times_known: bool,
}
#[derive(Clone, Debug)]
pub struct DirectoryEntry {
    pub name: Vec<u8>,
    pub size: u64,
    pub creation: u64,
    pub access: u64,
    pub write: u64,
    pub directory: bool,
    pub image_backed: bool,
    pub attributes: u32,
    pub allocation: u64,
}
pub trait IoHost {
    fn metadata(&self, attributes: u32) -> Result<Metadata, u32>;
    fn directory_next(
        &mut self,
        file: u32,
        restart: bool,
        mask: Option<&[u8]>,
        max_name: u32,
    ) -> Result<DirectoryEntry, u32>;
    fn set_value(&mut self, file: u32, class: u32, value: u64) -> (bool, u32);
    fn disposition(&mut self, file: u32, delete: bool) -> u32;
    fn rename(&mut self, file: u32, address: u32, length: u32) -> u32;

    fn is_file_object(&self, handle: u32) -> bool;
    fn volume_space(&self, handle: u32) -> Option<Space>;
    fn refuse_fabrication(&self) -> bool;
    fn write_u32(&mut self, address: u32, value: u32) -> bool;
    fn write_u8(&mut self, address: u32, value: u8) -> bool;
    fn publish(&mut self, ordinal: u32, address: u32, status: u32, information: u32) -> bool;
    fn flush_fatx(&mut self, handle: u32) -> u32;

    fn file(&self, handle: u32, require_file_object: bool) -> Option<File>;
    fn event_status(&self, handle: u32) -> u32;
    fn read_u32(&self, address: u32) -> Option<u32>;
    fn buffer_valid(&self, address: u32, length: u32) -> bool;
    fn guest_read(&self, address: u32, bytes: &mut [u8]) -> bool;
    fn guest_write(&mut self, address: u32, bytes: &[u8]) -> bool;
    fn iosb(&mut self, address: u32, status: u32, information: u32, write: bool) -> bool;
    fn backing_read(
        &mut self,
        file: u32,
        offset: u64,
        bytes: &mut [u8],
        stored: bool,
    ) -> Option<u32>;
    fn backing_write(&mut self, file: u32, offset: u64, bytes: &[u8]) -> BackingWrite;
    fn set_offset(&mut self, file: u32, offset: u64);
    fn clear_event(&mut self, event: u32) -> u32;
    fn signal_event(&mut self, event: u32);
    fn async_enabled(&self) -> bool;
    fn async_eligible(&self, options: u32, event: u32) -> bool;
    fn apc_refused(&mut self, options: u32, apc: u32) -> bool;
    fn service(&mut self);
    fn submit(&mut self, read: Read) -> Result<(), u32>;
    /// Counter/receipt boundary; C counters stay shared until all NT adapters move.
    fn diagnostic(&mut self, kind: Diagnostic, file: u32, value: u32);
    fn account(&mut self, kind: Account, bytes: u32, file: u32, offset: u64);
}
fn offset(host: &impl IoHost, request: &Transfer, file: &File) -> Option<u64> {
    if request.offset_pointer == 0 {
        Some(file.offset)
    } else {
        Some(
            u64::from(host.read_u32(request.offset_pointer)?)
                | u64::from(host.read_u32(request.offset_pointer.wrapping_add(4))?) << 32,
        )
    }
}
fn fail(host: &mut impl IoHost, r: &Transfer, status: u32, information: u32, write: bool) -> u32 {
    host.iosb(r.iosb, status, information, write);
    status
}
pub fn read(host: &mut impl IoHost, args: Option<[u32; 8]>, stored: bool) -> u32 {
    let Some(args) = args else {
        return INVALID_PARAMETER;
    };
    let r = Transfer::from(args);
    let Some(file) = host.file(r.file, true) else {
        return INVALID_HANDLE;
    };
    if r.event != 0 {
        let status = host.event_status(r.event);
        if status != 0 {
            return status;
        }
    }
    if r.buffer == 0 {
        return fail(host, &r, INVALID_PARAMETER, 0, false);
    }
    let Some(offset) = offset(host, &r, &file) else {
        return fail(host, &r, INVALID_PARAMETER, 0, false);
    };
    if host.apc_refused(file.options, r.apc) {
        host.diagnostic(Diagnostic::ApcRefused, r.file, r.apc);
        return fail(host, &r, NOT_IMPLEMENTED, 0, false);
    }
    if r.apc != 0 {
        host.account(Account::ApcIgnored, 0, r.file, offset);
        host.diagnostic(Diagnostic::ApcIgnored, r.file, r.apc);
    }
    let queued =
        r.length != 0 && r.length <= 16 * 1024 * 1024 && host.async_eligible(file.options, r.event);
    if host.async_enabled() {
        host.service();
    }
    if queued && !stored {
        let mut data = Vec::new();
        if data.try_reserve_exact(r.length as usize).is_err() {
            return fail(host, &r, NO_MEMORY, 0, false);
        }
        data.resize(r.length as usize, 0);
        let Some(got) = host.backing_read(r.file, offset, &mut data, false) else {
            return fail(host, &r, UNSUCCESSFUL, 0, false);
        };
        if got == 0 {
            if r.event != 0 {
                let status = host.clear_event(r.event);
                if status != 0 {
                    return status;
                }
            }
            return EOF;
        }
        if !host.buffer_valid(r.buffer, got) {
            return fail(host, &r, INVALID_PARAMETER, 0, false);
        }
        data.truncate(got as usize);
        let read = Read {
            file: r.file,
            event: r.event,
            iosb: r.iosb,
            buffer: r.buffer,
            data,
            requested: r.length,
            volume: file.volume,
        };
        if let Err(status) = host.submit(read) {
            host.diagnostic(Diagnostic::QueueRefused, r.file, status);
            return fail(host, &r, status, 0, false);
        }
        if r.offset_pointer == 0 {
            host.set_offset(r.file, offset.wrapping_add(u64::from(got)));
        }
        host.account(Account::ReadPending, got, r.file, offset);
        return 0x103;
    }
    let mut staging = [0; 16384];
    let mut total = 0u32;
    while total < r.length {
        let want = (r.length - total).min(staging.len() as u32);
        let Some(got) = host.backing_read(
            r.file,
            offset.wrapping_add(u64::from(total)),
            &mut staging[..want as usize],
            stored,
        ) else {
            return fail(host, &r, UNSUCCESSFUL, total, false);
        };
        if got == 0 {
            break;
        }
        if !host.guest_write(r.buffer.wrapping_add(total), &staging[..got as usize]) {
            return fail(host, &r, INVALID_PARAMETER, total, false);
        }
        total += got;
        if got < want {
            break;
        }
    }
    if r.length != 0 && total == 0 {
        if r.event != 0 {
            let status = host.clear_event(r.event);
            if status != 0 {
                return status;
            }
        }
        return EOF; // The existing C preset leaves IOSB unchanged at EOF.
    }
    if r.offset_pointer == 0 {
        host.set_offset(r.file, offset.wrapping_add(u64::from(total)));
    }
    host.account(Account::Read, total, r.file, offset);
    if !host.iosb(r.iosb, 0, total, false) {
        return INVALID_PARAMETER;
    }
    if r.event != 0 {
        host.signal_event(r.event);
    }
    0
}
pub fn write(host: &mut impl IoHost, args: Option<[u32; 8]>) -> u32 {
    let Some(args) = args else {
        return INVALID_PARAMETER;
    };
    let r = Transfer::from(args);
    let Some(file) = host.file(r.file, false) else {
        return fail(host, &r, INVALID_HANDLE, 0, true);
    };
    if r.buffer == 0 && r.length > 0 {
        return fail(host, &r, INVALID_PARAMETER, 0, true);
    }
    let Some(offset) = offset(host, &r, &file) else {
        return fail(host, &r, INVALID_PARAMETER, 0, true);
    };
    if r.apc != 0 {
        host.account(Account::ApcIgnored, 0, r.file, offset);
        host.diagnostic(Diagnostic::ApcIgnored, r.file, r.apc);
    }
    if r.event != 0 {
        host.diagnostic(Diagnostic::WriteEventIgnored, r.file, r.event);
    }
    let mut staging = [0; 16384];
    let mut total = 0u32;
    let mut status = 0;
    // Consult the storage/access gate even for zero-length writes.
    loop {
        let want = (r.length - total).min(staging.len() as u32);
        if want > 0 && !host.guest_read(r.buffer.wrapping_add(total), &mut staging[..want as usize])
        {
            host.account(Account::WriteRefused, total, r.file, offset);
            return fail(host, &r, INVALID_PARAMETER, total, true);
        }
        let reply = host.backing_write(
            r.file,
            offset.wrapping_add(u64::from(total)),
            &staging[..want as usize],
        );
        total += reply.transferred;
        if !reply.complete {
            status = reply.status;
            break;
        }
        if total >= r.length {
            break;
        }
    }
    if status != 0 {
        host.account(Account::WriteRefused, total, r.file, offset);
        return fail(host, &r, status, total, true);
    }
    if r.offset_pointer == 0 {
        host.set_offset(r.file, offset.wrapping_add(u64::from(total)));
    }
    host.account(Account::Write, total, r.file, offset);
    if !host.iosb(r.iosb, 0, total, true) {
        return INVALID_PARAMETER;
    }
    0
}

fn write_u64(host: &mut impl IoHost, address: u32, value: u64) -> bool {
    host.write_u32(address, value as u32)
        && host.write_u32(address.wrapping_add(4), (value >> 32) as u32)
}
fn reply(host: &mut impl IoHost, ordinal: u32, iosb: u32, status: u32, bytes: u32) -> u32 {
    host.publish(ordinal, iosb, status, bytes);
    status
}
/// Class 1 identity and class 3 capacity. Fabrications retain the original label/policy.
pub fn volume(host: &mut impl IoHost, args: Option<[u32; 5]>, stored: bool) -> u32 {
    const ORD: u32 = 218;
    let Some([file, iosb, buffer, length, class]) = args else {
        return INVALID_PARAMETER;
    };
    if !host.is_file_object(file) {
        return reply(host, ORD, iosb, INVALID_HANDLE, 0);
    }
    if stored && class != 3 {
        return reply(host, ORD, iosb, crate::info::STATUS_INVALID_INFO_CLASS, 0);
    }
    if class != 1 && class != 3 {
        host.account(Account::UnknownVolumeClass, 0, file, 0);
        host.diagnostic(Diagnostic::VolumeRefused, file, class);
        return reply(host, ORD, iosb, crate::info::STATUS_INVALID_INFO_CLASS, 0);
    }
    if length < 24 {
        return reply(host, ORD, iosb, crate::info::STATUS_INFO_LENGTH_MISMATCH, 0);
    }
    if buffer == 0 {
        return reply(host, ORD, iosb, INVALID_PARAMETER, 0);
    }
    if class == 1 {
        if host.refuse_fabrication() {
            host.account(Account::VolumeRefused, 0, file, 0);
            host.diagnostic(Diagnostic::VolumeRefused, file, class);
            return reply(host, ORD, iosb, UNSUCCESSFUL, 0);
        }
        host.account(Account::FabricatedIdentity, 0, file, 0);
        if !write_u64(host, buffer, 0)
            || !host.write_u32(buffer.wrapping_add(8), 0x7473_6670)
            || !host.write_u32(buffer.wrapping_add(12), 0)
            || !host.write_u8(buffer.wrapping_add(16), 0)
        {
            return reply(host, ORD, iosb, INVALID_PARAMETER, 0);
        }
        if !host.publish(ORD, iosb, 0, 17) {
            return INVALID_PARAMETER;
        }
        host.diagnostic(Diagnostic::VolumeFabricated, file, class);
        return 0;
    }
    let space = match host.volume_space(file) {
        Some(space) => space,
        None if stored || host.refuse_fabrication() => {
            host.account(Account::VolumeRefused, 0, file, 0);
            return reply(host, ORD, iosb, UNSUCCESSFUL, 0);
        }
        None => {
            host.account(Account::FabricatedGeometry, 0, file, 0);
            host.diagnostic(Diagnostic::VolumeFabricated, file, class);
            Space {
                total: 0x40000,
                available: 0x20000,
                sectors: 32,
                sector_bytes: 512,
            }
        }
    };
    if !write_u64(host, buffer, space.total)
        || !write_u64(host, buffer.wrapping_add(8), space.available)
        || !host.write_u32(buffer.wrapping_add(16), space.sectors)
        || !host.write_u32(buffer.wrapping_add(20), space.sector_bytes)
    {
        return reply(host, ORD, iosb, INVALID_PARAMETER, 0);
    }
    if !host.publish(ORD, iosb, 0, 24) {
        return INVALID_PARAMETER;
    }
    0
}
/// Raw cache-partition geometry/partition information and the measured dismount.
pub fn control(host: &mut impl IoHost, args: Option<[u32; 10]>, fs: bool) -> u32 {
    let ordinal = if fs { 200 } else { 196 };
    let Some(args) = args else {
        return INVALID_PARAMETER;
    };
    let (file, iosb, code, buffer, length) = (args[0], args[4], args[5], args[8], args[9]);
    let Some(facts) = host.file(file, false) else {
        return reply(host, ordinal, iosb, INVALID_HANDLE, 0);
    };
    if facts.cache_partition == 0
        || (fs && code != 0x90020)
        || (!fs && code != 0x70000 && code != 0x74004)
    {
        host.account(Account::ControlRefused, 0, file, 0);
        host.diagnostic(Diagnostic::ControlRefused, file, code);
        return reply(host, ordinal, iosb, 0xc000_0010, 0);
    }
    let bytes = if fs {
        0
    } else if code == 0x70000 {
        24
    } else {
        32
    };
    if !fs && (buffer == 0 || length < bytes) {
        return reply(host, ordinal, iosb, 0xc000_0023, 0);
    }
    let ok = if fs {
        true
    } else if code == 0x70000 {
        write_u64(host, buffer, facts.size / (255 * 63 * 512))
            && host.write_u32(buffer.wrapping_add(8), 12)
            && host.write_u32(buffer.wrapping_add(12), 255)
            && host.write_u32(buffer.wrapping_add(16), 63)
            && host.write_u32(buffer.wrapping_add(20), 512)
    } else {
        write_u64(host, buffer, 0)
            && write_u64(host, buffer.wrapping_add(8), facts.size)
            && host.write_u32(buffer.wrapping_add(16), 0)
            && host.write_u32(buffer.wrapping_add(20), facts.cache_partition)
            && host.write_u32(buffer.wrapping_add(24), 0)
            && host.write_u32(buffer.wrapping_add(28), 0)
    };
    if !ok {
        return reply(host, ordinal, iosb, INVALID_PARAMETER, 0);
    }
    host.account(Account::ControlAnswered, 0, file, 0);
    reply(host, ordinal, iosb, 0, bytes)
}
pub fn flush(host: &mut impl IoHost, args: Option<[u32; 2]>) -> u32 {
    let Some([file, iosb]) = args else {
        return INVALID_PARAMETER;
    };
    let Some(facts) = host.file(file, false) else {
        return reply(host, 198, iosb, INVALID_HANDLE, 0);
    };
    let status = if facts.fatx { host.flush_fatx(file) } else { 0 };
    if !host.publish(198, iosb, status, 0) {
        return INVALID_PARAMETER;
    }
    if !facts.fatx {
        host.account(Account::Flush, 0, file, 0);
    }
    status
}

pub fn query_information(host: &mut impl IoHost, args: Option<[u32; 5]>) -> u32 {
    let Some([handle, iosb, buffer, length, class]) = args else {
        return INVALID_PARAMETER;
    };
    let Some(file) = host.file(handle, false) else {
        return reply(host, 211, iosb, INVALID_HANDLE, 0);
    };
    if buffer == 0 {
        return reply(host, 211, iosb, INVALID_PARAMETER, 0);
    }
    let facts = crate::info::FileFacts {
        offset: file.offset,
        size: file.size,
        is_directory: file.directory,
        host_dir: file.host_dir,
        disc_file: file.disc_file,
        disc_sector: file.disc_sector,
    };
    let result = crate::info::query(class, length, &facts);
    if result.unknown_class {
        host.account(Account::UnknownFileClass, 0, handle, 0);
        host.diagnostic(Diagnostic::FileClassRefused, handle, class);
    }
    if result.status != 0 {
        return reply(host, 211, iosb, result.status, 0);
    }
    // C original writes 64-bit scalar replies as two dwords; preserve partial publication.
    let ok = if class == crate::info::CLASS_POSITION {
        write_u64(host, buffer, file.offset)
    } else if class == crate::info::CLASS_INTERNAL {
        write_u64(host, buffer, u64::from(file.disc_sector))
    } else {
        host.guest_write(buffer, &result.payload)
    };
    if !ok {
        return reply(host, 211, iosb, INVALID_PARAMETER, 0);
    }
    if result.fabricated_timestamps {
        host.account(Account::FabricatedTimestamp, 0, handle, 0);
        host.diagnostic(Diagnostic::TimestampsFabricated, handle, class);
    }
    reply(host, 211, iosb, 0, result.information)
}
pub fn set_information(host: &mut impl IoHost, args: Option<[u32; 5]>) -> u32 {
    use crate::info::{self, SetPlan};
    let Some([file, iosb, buffer, length, class]) = args else {
        return INVALID_PARAMETER;
    };
    let Some(facts) = host.file(file, false) else {
        return reply(host, 226, iosb, INVALID_HANDLE, 0);
    };
    if class == 10 {
        let status = host.rename(file, buffer, length);
        return reply(host, 226, iosb, status, 0);
    }
    if class == 13 {
        let mut flag = [0];
        let status = if length >= 1 && buffer != 0 && host.guest_read(buffer, &mut flag) {
            host.disposition(file, flag[0] != 0)
        } else {
            info::STATUS_INFO_LENGTH_MISMATCH
        };
        return reply(host, 226, iosb, status, 0);
    }
    let value = if length >= 8
        && buffer != 0
        && !(class == info::CLASS_ALLOCATION && buffer > u32::MAX - 7)
    {
        host.read_u32(buffer)
            .and_then(|low| {
                host.read_u32(buffer.wrapping_add(4))
                    .map(|high| (low, high))
            })
            .map(|(low, high)| u64::from(low) | u64::from(high) << 32)
    } else {
        None
    };
    let plan = info::set_plan(class, length, value);
    let value = match plan {
        SetPlan::Seek(v) | SetPlan::Allocation(v) | SetPlan::EndOfFile(v) => v,
        SetPlan::Basic => {
            // T3786: nothing is stored on a writable volume; the disc is read-only (STATUS_MEDIA_WRITE_PROTECTED).
            let status = if facts.disc_file { 0xC000_00A2 } else { 0 };
            return reply(host, 226, iosb, status, 0);
        }
        SetPlan::Refuse {
            status,
            unknown_class,
        } => {
            if unknown_class {
                host.account(Account::UnknownFileClass, 0, file, 0);
                host.diagnostic(Diagnostic::FileClassRefused, file, class);
            }
            return reply(host, 226, iosb, status, 0);
        }
    };
    let (ok, status) = host.set_value(file, class, value);
    if class == info::CLASS_POSITION {
        return reply(
            host,
            226,
            iosb,
            if ok { 0 } else { INVALID_HANDLE },
            if ok { 8 } else { 0 },
        );
    }
    if !ok {
        host.account(Account::WriteRefused, 0, file, 0);
        // Allocation refusal deliberately leaves IOSB untouched (historical xemu reference).
        if class == info::CLASS_ALLOCATION {
            return status;
        }
        return reply(host, 226, iosb, status, 0);
    }
    reply(
        host,
        226,
        iosb,
        if class == info::CLASS_ALLOCATION {
            status
        } else {
            0
        },
        if class == info::CLASS_ALLOCATION {
            0
        } else {
            8
        },
    )
}
pub fn full_attributes(host: &mut impl IoHost, args: Option<[u32; 2]>) -> u32 {
    let Some([attributes, buffer]) = args else {
        return INVALID_PARAMETER;
    };
    let found = match host.metadata(attributes) {
        Ok(found) => found,
        Err(status) => return status,
    };
    let mut result = [0u8; 56];
    let allocation = if found.host_dir {
        found.size
    } else {
        found.size.wrapping_add(2047) / 2048 * 2048
    };
    for (index, value) in [
        found.creation,
        found.access,
        found.write,
        found.write,
        allocation,
        found.size,
    ]
    .into_iter()
    .enumerate()
    {
        result[index * 8..index * 8 + 8].copy_from_slice(&value.to_le_bytes());
    }
    result[48..52].copy_from_slice(&(if found.directory { 0x10u32 } else { 0x20 }).to_le_bytes());
    if !host.guest_write(buffer, &result) {
        return INVALID_PARAMETER;
    }
    if !found.times_known {
        host.account(Account::FabricatedTimestamp, 0, 0, 0);
        host.diagnostic(Diagnostic::TimestampsFabricated, 0, 210);
    }
    0
}
pub fn directory(host: &mut impl IoHost, args: Option<[u32; 10]>) -> u32 {
    const ORD: u32 = 207;
    let Some(args) = args else {
        return INVALID_PARAMETER;
    };
    let (file, iosb, buffer, length, class) = (args[0], args[4], args[5], args[6], args[7]);
    if host.file(file, false).is_none() {
        return reply(host, ORD, iosb, INVALID_HANDLE, 0);
    }
    if args[1] != 0 || args[2] != 0 || args[3] != 0 {
        host.diagnostic(Diagnostic::DirectoryAsyncRefused, file, args[1]);
        return reply(host, ORD, iosb, NOT_IMPLEMENTED, 0);
    }
    if class != 1 {
        host.account(Account::UnknownFileClass, 0, file, 0);
        host.diagnostic(Diagnostic::FileClassRefused, file, class);
        return reply(host, ORD, iosb, crate::info::STATUS_INVALID_INFO_CLASS, 0);
    }
    if buffer == 0 || length < 64 {
        return reply(host, ORD, iosb, crate::info::STATUS_INFO_LENGTH_MISMATCH, 0);
    }
    let mut mask = Vec::new();
    if args[8] != 0 {
        let pointer = args[8];
        let mut size = [0; 2];
        if !host.guest_read(pointer, &mut size) {
            return reply(host, ORD, iosb, INVALID_PARAMETER, 0);
        }
        let Some(source) = host.read_u32(pointer.wrapping_add(4)) else {
            return reply(host, ORD, iosb, INVALID_PARAMETER, 0);
        };
        let size = u16::from_le_bytes(size) as usize;
        if size > crate::dirpick::NAME_MAX || (size > 0 && source == 0) {
            return reply(host, ORD, iosb, INVALID_PARAMETER, 0);
        }
        mask.resize(size, 0);
        for (index, byte) in mask.iter_mut().enumerate() {
            if !host.guest_read(
                source.wrapping_add(index as u32),
                std::slice::from_mut(byte),
            ) {
                return reply(host, ORD, iosb, INVALID_PARAMETER, 0);
            }
        }
    }
    let entry = match host.directory_next(
        file,
        args[9] & 0xff != 0,
        (args[8] != 0).then_some(mask.as_slice()),
        length - 64,
    ) {
        Ok(entry) => entry,
        Err(status) => return reply(host, ORD, iosb, status, 0),
    };
    let allocation = if entry.image_backed {
        entry.allocation
    } else {
        entry.size.wrapping_add(4095) / 4096 * 4096
    };
    let attributes = if entry.image_backed {
        entry.attributes
    } else if entry.directory {
        0x10
    } else {
        0x20
    };
    let mut ok = host.write_u32(buffer, 0) && host.write_u32(buffer.wrapping_add(4), 0);
    for (index, value) in [
        entry.creation,
        entry.access,
        entry.write,
        entry.write,
        entry.size,
        allocation,
    ]
    .into_iter()
    .enumerate()
    {
        ok = ok && write_u64(host, buffer.wrapping_add(8 + index as u32 * 8), value);
    }
    ok = ok
        && host.write_u32(buffer.wrapping_add(56), attributes)
        && host.write_u32(buffer.wrapping_add(60), entry.name.len() as u32)
        && host.guest_write(buffer.wrapping_add(64), &entry.name);
    if !ok {
        return reply(host, ORD, iosb, INVALID_PARAMETER, 0);
    }
    host.publish(ORD, iosb, 0, 64 + entry.name.len() as u32);
    host.account(Account::DirectoryEntry, 0, file, 0);
    0
}
