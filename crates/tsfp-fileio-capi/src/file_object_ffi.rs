//! T2210 file-object ABI. Callback safety/lifetimes are enforced by the C boundary.
use std::sync::{Mutex, OnceLock};
use tsfp_engine_fileio::file_object::{Backing, BodyHost, File, FileObjects};
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Host {
    file: extern "C" fn(u32, *mut u32, *mut u32, *mut u32) -> bool,
    allocate: extern "C" fn() -> u32,
    write: unsafe extern "C" fn(u32, *const u8, u32) -> bool,
    free: extern "C" fn(u32) -> bool,
    bind: extern "C" fn(u32, u32, u32) -> bool,
    find: extern "C" fn(u32, *mut u32) -> bool,
    write_u32: extern "C" fn(u32, u32) -> bool,
    reset: extern "C" fn(),
}
// Registered once by the host when Rust kmem is authoritative. Each operation
// enters the existing kmem registry; this state owns only the FILE_OBJECT heap token.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeHeapOps {
    create: extern "C" fn(u32, u32, u32) -> u32,
    alloc: extern "C" fn(u32, u32) -> u32,
    valid: extern "C" fn(u32) -> bool,
    free: extern "C" fn(u32, u32) -> bool,
    destroy: extern "C" fn(u32) -> bool,
}

// Successful allocations are monotonic for the process, including across resets.
static NATIVE_HEAP: Mutex<(Option<NativeHeapOps>, u32, u64)> = Mutex::new((None, 0, 0));

/// Called at startup before FILE_OBJECT operations; the table is copied.
///
/// # Safety
/// `ops` must point to a valid, process-lifetime function table, or be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_file_object_set_native_heap(ops: *const NativeHeapOps) -> bool {
    let mut state = NATIVE_HEAP.lock().unwrap();
    // Startup registration only; do not switch owners with an outstanding heap.
    if state.1 != 0 {
        return false;
    }
    state.0 = unsafe { ops.as_ref() }.copied();
    true
}

/// Whether native FILE_OBJECT heap operations were registered at startup.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_file_object_native_heap_active() -> bool {
    NATIVE_HEAP.lock().unwrap().0.is_some()
}

/// Successful native FILE_OBJECT allocations, not registration or attempted calls.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_file_object_native_heap_allocations() -> u64 {
    NATIVE_HEAP.lock().unwrap().2
}

fn native_allocate() -> Option<u32> {
    let mut state = NATIVE_HEAP.lock().unwrap();
    let ops = state.0?;
    if state.1 == 0 || !(ops.valid)(state.1) {
        state.1 = (ops.create)(0, 0x1_0000, 0);
    }
    let allocation = if state.1 == 0 {
        0
    } else {
        (ops.alloc)(state.1, 0x30)
    };
    if allocation != 0 {
        state.2 = state.2.saturating_add(1);
    }
    Some(allocation)
}

fn native_free(body: u32) -> Option<bool> {
    let state = NATIVE_HEAP.lock().unwrap();
    let ops = state.0?;
    Some(state.1 != 0 && (ops.valid)(state.1) && (ops.free)(state.1, body))
}

fn native_reset() -> bool {
    let mut state = NATIVE_HEAP.lock().unwrap();
    let Some(ops) = state.0 else {
        return false;
    };
    if state.1 != 0 && (ops.valid)(state.1) {
        (ops.destroy)(state.1);
    }
    state.1 = 0;
    true
}

impl BodyHost for Host {
    fn file(&self, handle: u32) -> Option<File> {
        let (mut body, mut kind, mut sector) = (0, 0, 0);
        if !(self.file)(handle, &mut body, &mut kind, &mut sector) {
            return None;
        }
        Some(File {
            existing_body: body,
            backing: match kind {
                1 => Backing::Disc { sector },
                2 => Backing::HostDirectory,
                _ => Backing::Unsupported,
            },
        })
    }
    fn allocate(&mut self) -> u32 {
        native_allocate().unwrap_or_else(|| (self.allocate)())
    }
    fn write_body(&mut self, body: u32, bytes: &[u8; 0x30]) -> bool {
        unsafe { (self.write)(body, bytes.as_ptr(), 0x30) }
    }
    fn free(&mut self, body: u32) -> bool {
        native_free(body).unwrap_or_else(|| (self.free)(body))
    }
    fn bind(&mut self, handle: u32, body: u32, name: u32) -> bool {
        (self.bind)(handle, body, name)
    }
    fn find(&self, body: u32) -> Option<u32> {
        let mut name = 0;
        (self.find)(body, &mut name).then_some(name)
    }
    fn write_u32(&mut self, address: u32, value: u32) -> bool {
        (self.write_u32)(address, value)
    }
    fn reset_heap(&mut self) {
        if !native_reset() {
            (self.reset)();
        }
    }
}
fn state() -> &'static FileObjects {
    static STATE: OnceLock<FileObjects> = OnceLock::new();
    STATE.get_or_init(FileObjects::default)
}
unsafe extern "C" fn fabricate(handle: u32, host: *const Host) -> u32 {
    state().fabricate(&mut unsafe { *host }, handle)
}
unsafe extern "C" fn release(body: u32, host: *const Host) {
    state().release(&mut unsafe { *host }, body);
}
unsafe extern "C" fn query(args: *const u32, host: *const Host) -> u32 {
    let args = if args.is_null() {
        None
    } else {
        Some(unsafe { *(args.cast::<[u32; 5]>()) })
    };
    state().query(&mut unsafe { *host }, args)
}
unsafe extern "C" fn reset(host: *const Host) {
    state().reset(&mut unsafe { *host });
}
extern "C" fn counter(index: u32) -> u32 {
    state().counters().get(index as usize).copied().unwrap_or(0)
}
#[repr(C)]
pub struct Backend {
    fabricate: unsafe extern "C" fn(u32, *const Host) -> u32,
    release: unsafe extern "C" fn(u32, *const Host),
    query: unsafe extern "C" fn(*const u32, *const Host) -> u32,
    reset: unsafe extern "C" fn(*const Host),
    counter: extern "C" fn(u32) -> u32,
}
static BACKEND: Backend = Backend {
    fabricate,
    release,
    query,
    reset,
    counter,
};
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_file_object_backend() -> *const Backend {
    &BACKEND
}

#[cfg(test)]
mod native_heap_tests {
    use super::*;
    use tsfp_kmem_capi::{
        MODE_RUST, tsfp_kmem_audio_heap_alloc, tsfp_kmem_audio_heap_block_size,
        tsfp_kmem_audio_heap_create, tsfp_kmem_audio_heap_destroy, tsfp_kmem_audio_heap_valid,
        tsfp_kmem_heap_free, tsfp_kmem_init,
    };

    extern "C" fn file(_: u32, body: *mut u32, kind: *mut u32, sector: *mut u32) -> bool {
        unsafe {
            body.write(0);
            kind.write(1);
            sector.write(7);
        }
        true
    }
    extern "C" fn failed_create(_: u32, _: u32, _: u32) -> u32 {
        0
    }
    extern "C" fn failed_alloc(_: u32, _: u32) -> u32 {
        0
    }
    extern "C" fn fallback_allocate() -> u32 {
        7
    }
    unsafe extern "C" fn write(_: u32, _: *const u8, length: u32) -> bool {
        length == 0x30
    }
    extern "C" fn fallback_free(body: u32) -> bool {
        body == 7
    }
    extern "C" fn bind(_: u32, _: u32, _: u32) -> bool {
        true
    }
    extern "C" fn find(_: u32, _: *mut u32) -> bool {
        false
    }
    extern "C" fn write_u32(_: u32, _: u32) -> bool {
        true
    }
    extern "C" fn fallback_reset() {}

    #[test]
    fn file_objects_share_kmem_heap_and_reset_its_lifecycle() {
        assert_eq!(tsfp_kmem_init(MODE_RUST, None, None), MODE_RUST);
        let ops = NativeHeapOps {
            create: tsfp_kmem_audio_heap_create,
            alloc: tsfp_kmem_audio_heap_alloc,
            valid: tsfp_kmem_audio_heap_valid,
            free: tsfp_kmem_heap_free,
            destroy: tsfp_kmem_audio_heap_destroy,
        };
        let failed_ops = NativeHeapOps {
            create: failed_create,
            ..ops
        };
        assert!(unsafe { tsfp_rfio_file_object_set_native_heap(&ops) });
        let allocations_before = tsfp_rfio_file_object_native_heap_allocations();
        let a = native_allocate().unwrap();
        let b = native_allocate().unwrap();
        assert_ne!(a, 0);
        assert_ne!(a, b);
        assert_eq!(
            tsfp_rfio_file_object_native_heap_allocations(),
            allocations_before + 2
        );
        let heap = NATIVE_HEAP.lock().unwrap().1;
        assert!(!unsafe { tsfp_rfio_file_object_set_native_heap(&failed_ops) });
        {
            let state = NATIVE_HEAP.lock().unwrap();
            assert_eq!(state.1, heap);
            assert_eq!(state.0.unwrap().create as usize, ops.create as usize);
        }
        assert!(tsfp_kmem_audio_heap_valid(heap));
        let mut size = 0;
        assert!(unsafe { tsfp_kmem_audio_heap_block_size(heap, a, &mut size) });
        assert_eq!(size, 0x30);
        assert_eq!(native_free(a), Some(true));
        assert_eq!(native_free(a), Some(false));
        assert!(!unsafe { tsfp_kmem_audio_heap_block_size(heap, a, &mut size) });
        assert!(native_reset());
        assert!(!tsfp_kmem_audio_heap_valid(heap));
        assert_eq!(native_free(b), Some(false));
        let stale_body = native_allocate().unwrap();
        let stale_heap = NATIVE_HEAP.lock().unwrap().1;
        assert_ne!(stale_body, 0);
        unsafe {
            tsfp_kmem_capi::tsfp_kmem_op(
                tsfp_kmem_capi::ops::OP_RESET,
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                0,
            );
        }
        assert!(!tsfp_kmem_audio_heap_valid(stale_heap));
        let replacement_body = native_allocate().unwrap();
        assert_ne!(replacement_body, 0);
        assert!(native_reset());
        let mut host = Host {
            file,
            allocate: fallback_allocate,
            write,
            free: fallback_free,
            bind,
            find,
            write_u32,
            reset: fallback_reset,
        };
        let objects = FileObjects::default();
        let body = objects.fabricate(&mut host, 1);
        assert_ne!(body, 0);
        let heap = NATIVE_HEAP.lock().unwrap().1;
        assert!(unsafe { tsfp_kmem_audio_heap_block_size(heap, body, &mut size) });
        objects.release(&mut host, body);
        assert!(!unsafe { tsfp_kmem_audio_heap_block_size(heap, body, &mut size) });
        objects.reset(&mut host);
        assert!(!tsfp_kmem_audio_heap_valid(heap));
        // A failed native create stays failed rather than falling back to C.
        assert!(unsafe { tsfp_rfio_file_object_set_native_heap(&failed_ops) });
        let successful_before_failure = tsfp_rfio_file_object_native_heap_allocations();
        assert_eq!(native_allocate(), Some(0));
        assert_eq!(
            tsfp_rfio_file_object_native_heap_allocations(),
            successful_before_failure
        );
        assert_eq!(native_free(body), Some(false));
        assert!(native_reset());
        let failed_alloc_ops = NativeHeapOps {
            alloc: failed_alloc,
            ..ops
        };
        assert!(unsafe { tsfp_rfio_file_object_set_native_heap(&failed_alloc_ops) });
        assert_eq!(native_allocate(), Some(0));
        assert_eq!(
            tsfp_rfio_file_object_native_heap_allocations(),
            successful_before_failure
        );
        assert!(native_reset());
        assert!(unsafe { tsfp_rfio_file_object_set_native_heap(std::ptr::null()) });
        assert_eq!(native_allocate(), None);
        assert_eq!(host.allocate(), 7);
        assert!(host.free(7));
        host.reset_heap();
    }
}
