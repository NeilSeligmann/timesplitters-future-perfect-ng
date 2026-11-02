//! T2210: typed NT transfer adapter over existing storage and guest-memory APIs.
use std::sync::atomic::{AtomicU64, Ordering};
use tsfp_engine_fileio::{
    async_io::{Read, Volume},
    kernel_io::{
        self, Account, BackingWrite, Diagnostic, DirectoryEntry, File, IoHost, Metadata, Space,
    },
};
const ORDINALS: [u32; 10] = [219, 236, 218, 196, 200, 198, 211, 226, 210, 207];
static CALLS: [AtomicU64; 10] = [const { AtomicU64::new(0) }; 10];
fn count(ordinal: u32) {
    if let Some(index) = ORDINALS.iter().position(|n| *n == ordinal) {
        CALLS[index].fetch_add(1, Ordering::Relaxed);
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_kio_calls(ordinal: u32) -> u64 {
    ORDINALS
        .iter()
        .position(|n| *n == ordinal)
        .map_or(0, |index| CALLS[index].load(Ordering::Relaxed))
}
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Facts {
    offset: u64,
    options: u32,
    volume: u32,
    size: u64,
    cache_partition: u32,
    fatx: bool,
    directory: bool,
    host_dir: bool,
    disc_file: bool,
    disc_sector: u32,
}
#[repr(C)]
struct CDir {
    name: [u8; 256],
    directory: bool,
    size: u64,
    creation: u64,
    access: u64,
    write: u64,
    image_backed: bool,
    attributes: u32,
    allocation: u64,
}
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Host {
    metadata: extern "C" fn(u32, *mut Metadata) -> u32,
    dir: unsafe extern "C" fn(u32, bool, *const std::ffi::c_char, u32, *mut CDir, *mut u32) -> bool,
    set_value: extern "C" fn(u32, u32, u64, *mut u32) -> bool,
    disposition: extern "C" fn(u32, bool) -> u32,
    rename: extern "C" fn(u32, u32, u32) -> u32,

    is_file: extern "C" fn(u32) -> bool,
    space: extern "C" fn(u32, *mut u64, *mut u64, *mut u32, *mut u32) -> bool,
    refuse: extern "C" fn() -> bool,
    write_u32: extern "C" fn(u32, u32) -> bool,
    write_u8: extern "C" fn(u32, u8) -> bool,
    publish: extern "C" fn(u32, u32, u32, u32) -> bool,
    flush: extern "C" fn(u32) -> u32,
    file: extern "C" fn(u32, bool, *mut Facts) -> bool,
    event_status: extern "C" fn(u32) -> u32,
    read_u32: extern "C" fn(u32, *mut u32) -> bool,
    valid: extern "C" fn(u32, u32) -> bool,
    read: unsafe extern "C" fn(u32, *mut u8, u32) -> bool,
    write: unsafe extern "C" fn(u32, *const u8, u32) -> bool,
    iosb: extern "C" fn(u32, u32, u32, bool) -> bool,
    backing_read: unsafe extern "C" fn(u32, u64, *mut u8, u32, *mut u32, bool) -> bool,
    backing_write: unsafe extern "C" fn(u32, u64, *const u8, u32, *mut u32, *mut u32) -> bool,
    offset: extern "C" fn(u32, u64),
    clear: extern "C" fn(u32) -> u32,
    signal: extern "C" fn(u32),
    enabled: extern "C" fn() -> bool,
    eligible: extern "C" fn(u32, u32) -> bool,
    apc: extern "C" fn(u32, u32) -> bool,
    service: extern "C" fn(),
    submit: unsafe extern "C" fn(*mut super::async_ffi::CRead) -> bool,
    diagnostic: extern "C" fn(u32, u32, u32),
    account: extern "C" fn(u32, u32, u32, u64),
}
impl IoHost for Host {
    fn metadata(&self, attributes: u32) -> Result<Metadata, u32> {
        let mut result = Metadata::default();
        let status = (self.metadata)(attributes, &mut result);
        if status == 0 { Ok(result) } else { Err(status) }
    }
    fn directory_next(
        &mut self,
        file: u32,
        restart: bool,
        mask: Option<&[u8]>,
        max_name: u32,
    ) -> Result<DirectoryEntry, u32> {
        // The old ANSI buffer can contain NUL: the C string ends at its first NUL.
        let mask = mask.map(|mask| {
            std::ffi::CString::new(&mask[..mask.iter().position(|b| *b == 0).unwrap_or(mask.len())])
                .unwrap()
        });
        let mut entry = CDir {
            name: [0; 256],
            directory: false,
            size: 0,
            creation: 0,
            access: 0,
            write: 0,
            image_backed: false,
            attributes: 0,
            allocation: 0,
        };
        let mut status = 0;
        if !unsafe {
            (self.dir)(
                file,
                restart,
                mask.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
                max_name,
                &mut entry,
                &mut status,
            )
        } {
            return Err(status);
        }
        let length = entry.name.iter().position(|b| *b == 0).unwrap_or(256);
        Ok(DirectoryEntry {
            name: entry.name[..length].to_vec(),
            size: entry.size,
            creation: entry.creation,
            access: entry.access,
            write: entry.write,
            directory: entry.directory,
            image_backed: entry.image_backed,
            attributes: entry.attributes,
            allocation: entry.allocation,
        })
    }
    fn set_value(&mut self, file: u32, class: u32, value: u64) -> (bool, u32) {
        let mut status = kernel_io::UNSUCCESSFUL;
        let ok = (self.set_value)(file, class, value, &mut status);
        (ok, status)
    }
    fn disposition(&mut self, file: u32, delete: bool) -> u32 {
        (self.disposition)(file, delete)
    }
    fn rename(&mut self, file: u32, address: u32, length: u32) -> u32 {
        (self.rename)(file, address, length)
    }

    fn is_file_object(&self, handle: u32) -> bool {
        (self.is_file)(handle)
    }
    fn volume_space(&self, handle: u32) -> Option<Space> {
        let (mut total, mut available, mut sectors, mut sector_bytes) = (0, 0, 0, 0);
        (self.space)(
            handle,
            &mut total,
            &mut available,
            &mut sectors,
            &mut sector_bytes,
        )
        .then_some(Space {
            total,
            available,
            sectors,
            sector_bytes,
        })
    }
    fn refuse_fabrication(&self) -> bool {
        (self.refuse)()
    }
    fn write_u32(&mut self, address: u32, value: u32) -> bool {
        (self.write_u32)(address, value)
    }
    fn write_u8(&mut self, address: u32, value: u8) -> bool {
        (self.write_u8)(address, value)
    }
    fn publish(&mut self, ordinal: u32, address: u32, status: u32, information: u32) -> bool {
        (self.publish)(ordinal, address, status, information)
    }
    fn flush_fatx(&mut self, handle: u32) -> u32 {
        (self.flush)(handle)
    }

    fn file(&self, handle: u32, require_file_object: bool) -> Option<File> {
        let mut facts = Facts {
            offset: 0,
            options: 0,
            volume: 0,
            size: 0,
            cache_partition: 0,
            fatx: false,
            directory: false,
            host_dir: false,
            disc_file: false,
            disc_sector: 0,
        };
        (self.file)(handle, require_file_object, &mut facts).then_some(File {
            offset: facts.offset,
            options: facts.options,
            size: facts.size,
            cache_partition: facts.cache_partition,
            fatx: facts.fatx,
            directory: facts.directory,
            host_dir: facts.host_dir,
            disc_file: facts.disc_file,
            disc_sector: facts.disc_sector,
            volume: if facts.volume == 1 {
                Volume::Hdd
            } else {
                Volume::Disc
            },
        })
    }
    fn event_status(&self, handle: u32) -> u32 {
        (self.event_status)(handle)
    }
    fn read_u32(&self, address: u32) -> Option<u32> {
        let mut value = 0;
        (self.read_u32)(address, &mut value).then_some(value)
    }
    fn buffer_valid(&self, address: u32, length: u32) -> bool {
        (self.valid)(address, length)
    }
    fn guest_read(&self, address: u32, bytes: &mut [u8]) -> bool {
        unsafe { (self.read)(address, bytes.as_mut_ptr(), bytes.len() as u32) }
    }
    fn guest_write(&mut self, address: u32, bytes: &[u8]) -> bool {
        unsafe { (self.write)(address, bytes.as_ptr(), bytes.len() as u32) }
    }
    fn iosb(&mut self, address: u32, status: u32, information: u32, write: bool) -> bool {
        (self.iosb)(address, status, information, write)
    }
    fn backing_read(
        &mut self,
        file: u32,
        offset: u64,
        bytes: &mut [u8],
        stored: bool,
    ) -> Option<u32> {
        let mut got = 0;
        unsafe {
            (self.backing_read)(
                file,
                offset,
                bytes.as_mut_ptr(),
                bytes.len() as u32,
                &mut got,
                stored,
            )
        }
        .then_some(got)
    }
    fn backing_write(&mut self, file: u32, offset: u64, bytes: &[u8]) -> BackingWrite {
        let (mut got, mut status) = (0, kernel_io::UNSUCCESSFUL);
        let complete = unsafe {
            (self.backing_write)(
                file,
                offset,
                bytes.as_ptr(),
                bytes.len() as u32,
                &mut got,
                &mut status,
            )
        };
        BackingWrite {
            complete,
            status,
            transferred: got,
        }
    }
    fn set_offset(&mut self, file: u32, offset: u64) {
        (self.offset)(file, offset);
    }
    fn clear_event(&mut self, event: u32) -> u32 {
        (self.clear)(event)
    }
    fn signal_event(&mut self, event: u32) {
        (self.signal)(event);
    }
    fn async_enabled(&self) -> bool {
        (self.enabled)()
    }
    fn async_eligible(&self, options: u32, event: u32) -> bool {
        (self.eligible)(options, event)
    }
    fn apc_refused(&mut self, options: u32, apc: u32) -> bool {
        (self.apc)(options, apc)
    }
    fn service(&mut self) {
        (self.service)();
    }
    fn submit(&mut self, read: Read) -> Result<(), u32> {
        // Existing C async submission takes malloc ownership even when the C oracle is selected.
        let data = unsafe { libc::malloc(read.data.len().max(1)) }.cast::<u8>();
        if data.is_null() {
            return Err(kernel_io::NO_MEMORY);
        }
        unsafe { std::ptr::copy_nonoverlapping(read.data.as_ptr(), data, read.data.len()) };
        let mut request = super::async_ffi::CRead {
            file: read.file,
            event: read.event,
            iosb: read.iosb,
            buffer: read.buffer,
            data,
            length: read.data.len() as u32,
            requested: read.requested,
            volume: if read.volume == Volume::Hdd { 1 } else { 0 },
        };
        if unsafe { (self.submit)(&mut request) } {
            Ok(())
        } else {
            unsafe { libc::free(data.cast()) };
            Err(kernel_io::RESOURCES)
        }
    }
    fn diagnostic(&mut self, kind: Diagnostic, file: u32, value: u32) {
        (self.diagnostic)(kind as u32, file, value);
    }
    fn account(&mut self, kind: Account, bytes: u32, file: u32, offset: u64) {
        (self.account)(kind as u32, bytes, file, offset);
    }
}
unsafe extern "C" fn read(args: *const u32, stored: bool, host: *const Host) -> u32 {
    let args = if args.is_null() {
        None
    } else {
        Some(unsafe { *args.cast::<[u32; 8]>() })
    };
    count(219);
    kernel_io::read(&mut unsafe { *host }, args, stored)
}
unsafe extern "C" fn write(args: *const u32, host: *const Host) -> u32 {
    let args = if args.is_null() {
        None
    } else {
        Some(unsafe { *args.cast::<[u32; 8]>() })
    };
    count(236);
    kernel_io::write(&mut unsafe { *host }, args)
}
unsafe extern "C" fn extra(ordinal: u32, args: *const u32, stored: bool, host: *const Host) -> u32 {
    count(ordinal);
    let mut host = unsafe { *host };
    match ordinal {
        218 => kernel_io::volume(
            &mut host,
            if args.is_null() {
                None
            } else {
                Some(unsafe { *args.cast::<[u32; 5]>() })
            },
            stored,
        ),
        196 | 200 => kernel_io::control(
            &mut host,
            if args.is_null() {
                None
            } else {
                Some(unsafe { *args.cast::<[u32; 10]>() })
            },
            ordinal == 200,
        ),
        198 => kernel_io::flush(
            &mut host,
            if args.is_null() {
                None
            } else {
                Some(unsafe { *args.cast::<[u32; 2]>() })
            },
        ),
        211 => kernel_io::query_information(
            &mut host,
            if args.is_null() {
                None
            } else {
                Some(unsafe { *args.cast::<[u32; 5]>() })
            },
        ),
        226 => kernel_io::set_information(
            &mut host,
            if args.is_null() {
                None
            } else {
                Some(unsafe { *args.cast::<[u32; 5]>() })
            },
        ),
        210 => kernel_io::full_attributes(
            &mut host,
            if args.is_null() {
                None
            } else {
                Some(unsafe { *args.cast::<[u32; 2]>() })
            },
        ),
        207 => kernel_io::directory(
            &mut host,
            if args.is_null() {
                None
            } else {
                Some(unsafe { *args.cast::<[u32; 10]>() })
            },
        ),
        _ => kernel_io::NOT_IMPLEMENTED,
    }
}
#[repr(C)]
pub struct Backend {
    extra: unsafe extern "C" fn(u32, *const u32, bool, *const Host) -> u32,
    read: unsafe extern "C" fn(*const u32, bool, *const Host) -> u32,
    write: unsafe extern "C" fn(*const u32, *const Host) -> u32,
}
static BACKEND: Backend = Backend { extra, read, write };
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_io_backend() -> *const Backend {
    &BACKEND
}
