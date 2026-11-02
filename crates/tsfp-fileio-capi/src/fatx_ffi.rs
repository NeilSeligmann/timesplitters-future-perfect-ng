//! C ABI of the Rust FATX volume (T2074). `src/input/mu_fatx.c` keeps its C implementation and routes the
//! public `fatx_*` functions here through the `fatx_backend` table when the host runs with `--rust-fileio`.
//! `CVol` and `CDirEntry` mirror `fatx_volume` and `fatx_directory_entry` in `src/input/mu_fatx.h`.
use std::ffi::{CStr, c_char, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use tsfp_engine_fileio::fatx_img::{self as f, Geo};

#[repr(C)]
pub struct CVol {
    image: *mut u8,
    size: usize,
    volume_id: u32,
    sectors_per_cluster: u32,
    root_cluster: u32,
    cluster_count: u32,
    fat_offset: usize,
    fat_size: usize,
    data_offset: usize,
    fat32: bool,
}

#[repr(C)]
pub struct CDirEntry {
    pub(crate) name: [u8; 43],
    pub(crate) directory: bool,
    pub(crate) attributes: u8,
    pub(crate) size: u32,
    pub(crate) allocation: u64,
    pub(crate) creation_time: u64,
    pub(crate) access_time: u64,
    pub(crate) write_time: u64,
}

fn geo(v: &CVol) -> Geo {
    Geo {
        size: v.size,
        volume_id: v.volume_id,
        root_cluster: v.root_cluster,
        cluster_count: v.cluster_count,
        fat_offset: v.fat_offset,
        fat_size: v.fat_size,
        data_offset: v.data_offset,
        fat32: v.fat32,
    }
}
fn put_geo(v: &mut CVol, image: *mut u8, g: &Geo) {
    *v = CVol {
        image,
        size: g.size,
        volume_id: g.volume_id,
        sectors_per_cluster: f::CLUSTER_SECTORS,
        root_cluster: g.root_cluster,
        cluster_count: g.cluster_count,
        fat_offset: g.fat_offset,
        fat_size: g.fat_size,
        data_offset: g.data_offset,
        fat32: g.fat32,
    };
}

/// Runs `body`; a panic (corrupt geometry reaching an out-of-range index) becomes `E_CORRUPT`.
fn guarded(body: impl FnOnce() -> u32) -> u32 {
    catch_unwind(AssertUnwindSafe(body)).unwrap_or(f::E_CORRUPT)
}

unsafe fn image<'a>(v: &CVol) -> &'a mut [u8] {
    unsafe { std::slice::from_raw_parts_mut(v.image, v.size) }
}
pub(crate) unsafe fn directory_entry_at(
    volume: *const CVol,
    path: &[u8],
    index: u32,
) -> Result<(CDirEntry, bool), u32> {
    if volume.is_null() {
        return Err(f::E_ARGUMENT);
    }
    let volume = unsafe { &*volume };
    if volume.image.is_null() || volume.size == 0 {
        return Err(f::E_ARGUMENT);
    }
    let bytes = unsafe { std::slice::from_raw_parts(volume.image, volume.size) };
    let (entry, end) = f::directory_entry_at(&geo(volume), bytes, path, index)?;
    if entry.name.len() > 42 {
        return Err(f::E_CORRUPT);
    }
    let mut out = CDirEntry {
        name: [0; 43],
        directory: entry.directory,
        attributes: entry.attributes,
        size: entry.size,
        allocation: entry.allocation,
        creation_time: entry.creation_time,
        access_time: entry.access_time,
        write_time: entry.write_time,
    };
    out.name[..entry.name.len()].copy_from_slice(&entry.name);
    Ok((out, end))
}
unsafe fn bytes<'a>(p: *const u8, n: u32) -> &'a [u8] {
    if n == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(p, n as usize) }
    }
}
unsafe fn cstr<'a>(p: *const c_char) -> &'a [u8] {
    unsafe { CStr::from_ptr(p) }.to_bytes()
}

unsafe extern "C" fn c_format(img: *mut u8, size: usize, volume_id: u32) -> u32 {
    if img.is_null() {
        return f::E_ARGUMENT;
    }
    guarded(|| {
        f::format(
            unsafe { std::slice::from_raw_parts_mut(img, size) },
            volume_id,
        )
    })
}
unsafe extern "C" fn c_open(v: *mut CVol, img: *mut u8, size: usize) -> u32 {
    if v.is_null() || img.is_null() {
        return f::E_ARGUMENT;
    }
    guarded(
        || match f::open(unsafe { std::slice::from_raw_parts(img, size) }) {
            Ok(g) => {
                put_geo(unsafe { &mut *v }, img, &g);
                f::OK
            }
            Err(e) => e,
        },
    )
}
unsafe extern "C" fn c_write_file(
    v: *mut CVol,
    name: *const c_char,
    data: *const u8,
    length: u32,
) -> u32 {
    if v.is_null() || (data.is_null() && length != 0) {
        return f::E_ARGUMENT;
    }
    guarded(|| {
        let v = unsafe { &*v };
        f::write_file(
            &geo(v),
            unsafe { image(v) },
            unsafe { cstr(name) },
            unsafe { bytes(data, length) },
        )
    })
}
unsafe extern "C" fn c_read_file(
    v: *const CVol,
    name: *const c_char,
    out: *mut u8,
    capacity: u32,
    length: *mut u32,
) -> u32 {
    if v.is_null() || length.is_null() || (out.is_null() && capacity != 0) {
        return f::E_ARGUMENT;
    }
    guarded(|| {
        let v = unsafe { &*v };
        let dst: &mut [u8] = if capacity == 0 {
            &mut []
        } else {
            unsafe { std::slice::from_raw_parts_mut(out, capacity as usize) }
        };
        f::read_file(
            &geo(v),
            unsafe { image(v) },
            unsafe { cstr(name) },
            dst,
            unsafe { &mut *length },
        )
    })
}
unsafe extern "C" fn c_remove_file(v: *mut CVol, name: *const c_char) -> u32 {
    if v.is_null() {
        return f::E_ARGUMENT;
    }
    guarded(|| {
        let v = unsafe { &*v };
        f::remove_file(&geo(v), unsafe { image(v) }, unsafe { cstr(name) })
    })
}
unsafe extern "C" fn c_file_count(v: *const CVol) -> u32 {
    catch_unwind(AssertUnwindSafe(|| {
        let v = unsafe { &*v };
        f::file_count(&geo(v), unsafe { image(v) })
    }))
    .unwrap_or(u32::MAX)
}
unsafe extern "C" fn c_free_clusters(v: *const CVol) -> u32 {
    catch_unwind(AssertUnwindSafe(|| {
        let v = unsafe { &*v };
        f::free_clusters(&geo(v), unsafe { image(v) })
    }))
    .unwrap_or(0)
}
unsafe extern "C" fn c_stat_path(
    v: *const CVol,
    path: *const c_char,
    directory: *mut bool,
    size: *mut u32,
) -> u32 {
    if directory.is_null() || size.is_null() {
        return f::E_ARGUMENT;
    }
    guarded(|| {
        let v = unsafe { &*v };
        match f::stat_path(&geo(v), unsafe { image(v) }, unsafe { cstr(path) }) {
            Ok((d, s)) => {
                unsafe {
                    *directory = d;
                    *size = s;
                }
                f::OK
            }
            Err(e) => e,
        }
    })
}
unsafe extern "C" fn c_mkdir_path(v: *mut CVol, path: *const c_char) -> u32 {
    guarded(|| {
        let v = unsafe { &*v };
        f::mkdir_path(&geo(v), unsafe { image(v) }, unsafe { cstr(path) })
    })
}
unsafe extern "C" fn c_write_path(
    v: *mut CVol,
    path: *const c_char,
    data: *const u8,
    size: u32,
) -> u32 {
    if data.is_null() && size != 0 {
        return f::E_ARGUMENT;
    }
    guarded(|| {
        let v = unsafe { &*v };
        f::write_path(
            &geo(v),
            unsafe { image(v) },
            unsafe { cstr(path) },
            unsafe { bytes(data, size) },
        )
    })
}
unsafe extern "C" fn c_read_range(
    v: *const CVol,
    path: *const c_char,
    offset: u64,
    out: *mut u8,
    count: u32,
    read: *mut u32,
) -> u32 {
    if read.is_null() || (count != 0 && out.is_null()) {
        return f::E_ARGUMENT;
    }
    unsafe { *read = 0 };
    guarded(|| {
        let v = unsafe { &*v };
        let dst: &mut [u8] = if count == 0 {
            &mut []
        } else {
            unsafe { std::slice::from_raw_parts_mut(out, count as usize) }
        };
        match f::read_range(
            &geo(v),
            unsafe { image(v) },
            unsafe { cstr(path) },
            offset,
            dst,
        ) {
            Ok(n) => {
                unsafe { *read = n };
                f::OK
            }
            Err(e) => e,
        }
    })
}
unsafe extern "C" fn c_resize_path(
    v: *mut CVol,
    path: *const c_char,
    size: u64,
    offset: u64,
    data: *const u8,
    count: u32,
) -> u32 {
    if v.is_null() || (count != 0 && data.is_null()) {
        return f::E_ARGUMENT;
    }
    guarded(|| {
        let v = unsafe { &*v };
        f::resize_path(
            &geo(v),
            unsafe { image(v) },
            unsafe { cstr(path) },
            size,
            offset,
            unsafe { bytes(data, count) },
        )
    })
}
unsafe extern "C" fn c_directory_entry_at(
    v: *const CVol,
    path: *const c_char,
    index: u32,
    out: *mut CDirEntry,
    end: *mut bool,
) -> u32 {
    if out.is_null() || end.is_null() {
        return f::E_ARGUMENT;
    }
    guarded(|| {
        let v = unsafe { &*v };
        let o = unsafe { &mut *out };
        *o = CDirEntry {
            name: [0; 43],
            directory: false,
            attributes: 0,
            size: 0,
            allocation: 0,
            creation_time: 0,
            access_time: 0,
            write_time: 0,
        };
        unsafe { *end = index as usize >= f::CLUSTER_BYTES / f::ENTRY_BYTES };
        match f::directory_entry_at(&geo(v), unsafe { image(v) }, unsafe { cstr(path) }, index) {
            Ok((e, last)) => {
                unsafe { *end = last };
                o.name[..e.name.len()].copy_from_slice(&e.name);
                (o.directory, o.attributes, o.size, o.allocation) =
                    (e.directory, e.attributes, e.size, e.allocation);
                (o.creation_time, o.access_time, o.write_time) =
                    (e.creation_time, e.access_time, e.write_time);
                f::OK
            }
            Err(e) => e,
        }
    })
}
unsafe extern "C" fn c_remove_path(v: *mut CVol, path: *const c_char) -> u32 {
    if v.is_null() || path.is_null() {
        return f::E_ARGUMENT;
    }
    guarded(|| {
        let v = unsafe { &*v };
        f::remove_path(&geo(v), unsafe { image(v) }, unsafe { cstr(path) })
    })
}
unsafe extern "C" fn c_rename_path(
    v: *mut CVol,
    from: *const c_char,
    to: *const c_char,
    replace: bool,
) -> u32 {
    if v.is_null() || from.is_null() || to.is_null() {
        return f::E_ARGUMENT;
    }
    guarded(|| {
        let v = unsafe { &*v };
        f::rename_path(
            &geo(v),
            unsafe { image(v) },
            unsafe { cstr(from) },
            unsafe { cstr(to) },
            replace,
        )
    })
}

/// The table `src/input/mu_fatx.c` accepts (`fatx_backend`).
#[repr(C)]
pub struct FatxBackend {
    format: unsafe extern "C" fn(*mut u8, usize, u32) -> u32,
    open: unsafe extern "C" fn(*mut CVol, *mut u8, usize) -> u32,
    write_file: unsafe extern "C" fn(*mut CVol, *const c_char, *const u8, u32) -> u32,
    read_file: unsafe extern "C" fn(*const CVol, *const c_char, *mut u8, u32, *mut u32) -> u32,
    remove_file: unsafe extern "C" fn(*mut CVol, *const c_char) -> u32,
    file_count: unsafe extern "C" fn(*const CVol) -> u32,
    free_clusters: unsafe extern "C" fn(*const CVol) -> u32,
    stat_path: unsafe extern "C" fn(*const CVol, *const c_char, *mut bool, *mut u32) -> u32,
    mkdir_path: unsafe extern "C" fn(*mut CVol, *const c_char) -> u32,
    write_path: unsafe extern "C" fn(*mut CVol, *const c_char, *const u8, u32) -> u32,
    read_range:
        unsafe extern "C" fn(*const CVol, *const c_char, u64, *mut u8, u32, *mut u32) -> u32,
    resize_path: unsafe extern "C" fn(*mut CVol, *const c_char, u64, u64, *const u8, u32) -> u32,
    directory_entry_at:
        unsafe extern "C" fn(*const CVol, *const c_char, u32, *mut CDirEntry, *mut bool) -> u32,
    remove_path: unsafe extern "C" fn(*mut CVol, *const c_char) -> u32,
    rename_path: unsafe extern "C" fn(*mut CVol, *const c_char, *const c_char, bool) -> u32,
}
static BACKEND: FatxBackend = FatxBackend {
    format: c_format,
    open: c_open,
    write_file: c_write_file,
    read_file: c_read_file,
    remove_file: c_remove_file,
    file_count: c_file_count,
    free_clusters: c_free_clusters,
    stat_path: c_stat_path,
    mkdir_path: c_mkdir_path,
    write_path: c_write_path,
    read_range: c_read_range,
    resize_path: c_resize_path,
    directory_entry_at: c_directory_entry_at,
    remove_path: c_remove_path,
    rename_path: c_rename_path,
};

/// Pointer to the static table, for `fatx_set_backend`.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_fatx_backend() -> *const FatxBackend {
    &BACKEND
}

#[allow(dead_code)]
type Unused = *const c_void;
