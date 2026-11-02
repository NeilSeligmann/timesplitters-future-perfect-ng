//! Liveness counters of the Rust file I/O backends (T2074): one atomic per backend entry point, printed by the host at exit
//! (`rust-fileio: liveness ...`) so a run proves the Rust code, not the C fallback, served the route.
use std::sync::atomic::{AtomicU64, Ordering};

pub const HOSTDIR_WALK: usize = 0;
pub const HOSTDIR_NAME: usize = 1;
pub const HANDLE_CLAIM: usize = 2;
pub const HANDLE_FIND: usize = 3;
pub const DIR_PICK: usize = 4;
pub const INFO_QUERY: usize = 5;
pub const INFO_SET: usize = 6;
pub const PAYLOAD: usize = 7;
pub const DISPOSITION: usize = 8;
pub const COMMIT: usize = 9;
pub const NAMESPACE: usize = 10;
pub const VIEWS: usize = 11;
pub const GLUE: usize = 12;
pub const DESC: usize = 13;
pub const HOST_OPEN: usize = 14;
pub const HOST_CLOSE: usize = 15;
pub const CACHE_READ: usize = 16;
pub const HOST_ALLOCATE: usize = 17;
pub const HOST_METADATA: usize = 18;
pub const HOST_METADATA_AT: usize = 19;
pub const HOST_SPACE: usize = 20;
pub const DIR_OPEN: usize = 21;
pub const DIR_NEXT: usize = 22;
pub const DIR_CLOSE: usize = 23;
pub const GUEST_NAME: usize = 24;
pub const GUEST_RENAME: usize = 25;
pub const HOST_DUP: usize = 26;
pub const HOST_MKDIR: usize = 27;
pub const HOST_UNLINK: usize = 28;
pub const HOST_RENAME: usize = 29;
pub const ACQUIRE: usize = 30;
pub const RESOLVE: usize = 31;
pub const CREATE: usize = 32;
pub const RENAME_COMMIT: usize = 33;
pub const VOLUME_LIFECYCLE: usize = 34;
pub const CACHE_LIFECYCLE: usize = 35;
pub const ACTIVITY: usize = 36;
pub const OBSERVER: usize = 37;
pub const OPENABLE: usize = 38;
pub const COUNT: usize = 39;
pub const NAMES: [&str; COUNT] = [
    "hostdir_walk",
    "hostdir_name",
    "handle_claim",
    "handle_find",
    "dir_pick",
    "info_query",
    "info_set",
    "payload",
    "disposition",
    "commit",
    "namespace",
    "views",
    "glue",
    "desc",
    "host_open",
    "host_close",
    "cache_read",
    "host_allocate",
    "host_metadata",
    "host_metadata_at",
    "host_space",
    "dir_open",
    "dir_next",
    "dir_close",
    "guest_name",
    "guest_rename",
    "host_dup",
    "host_mkdir",
    "host_unlink",
    "host_rename",
    "acquire",
    "resolve",
    "create",
    "rename_commit",
    "volume_lifecycle",
    "cache_lifecycle",
    "activity",
    "observer",
    "openable",
];

static COUNTERS: [AtomicU64; COUNT] = [const { AtomicU64::new(0) }; COUNT];

pub fn bump(index: usize) {
    COUNTERS[index].fetch_add(1, Ordering::Relaxed);
}

/// Count of calls the Rust backend `index` served so far (0 for an unknown index).
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_counter(index: u32) -> u64 {
    COUNTERS
        .get(index as usize)
        .map_or(0, |c| c.load(Ordering::Relaxed))
}

/// NUL-terminated name of counter `index`, NULL past the end.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_counter_name(index: u32) -> *const std::ffi::c_char {
    const TABLE: [&std::ffi::CStr; COUNT] = [
        c"hostdir_walk",
        c"hostdir_name",
        c"handle_claim",
        c"handle_find",
        c"dir_pick",
        c"info_query",
        c"info_set",
        c"payload",
        c"disposition",
        c"commit",
        c"namespace",
        c"views",
        c"glue",
        c"desc",
        c"host_open",
        c"host_close",
        c"cache_read",
        c"host_allocate",
        c"host_metadata",
        c"host_metadata_at",
        c"host_space",
        c"dir_open",
        c"dir_next",
        c"dir_close",
        c"guest_name",
        c"guest_rename",
        c"host_dup",
        c"host_mkdir",
        c"host_unlink",
        c"host_rename",
        c"acquire",
        c"resolve",
        c"create",
        c"rename_commit",
        c"volume_lifecycle",
        c"cache_lifecycle",
        c"activity",
        c"observer",
        c"openable",
    ];
    TABLE
        .get(index as usize)
        .map_or(std::ptr::null(), |s| s.as_ptr())
}
