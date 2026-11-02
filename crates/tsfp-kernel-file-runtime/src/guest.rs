//! Native guest entrypoints for the file runtime. Host memory and object tables
//! remain explicit ABI services; all file and symbolic-link state is Rust-owned.
use std::ffi::{CString, c_char, c_void};
use std::sync::atomic::{AtomicBool, Ordering};
use tsfp_engine_fileio::guest_names::{self, Address, AttributeField, NameMemory, RenameHost};
use crate::guest_memory::{self, Memory, INVALID_PARAMETER, PATH_CAPACITY};
use crate::runtime::runtime;

const INSUFFICIENT_RESOURCES: u32 = 0xc000009a;
const INVALID_HANDLE: u32 = 0xc0000008;
const OBJECT_TYPE_MISMATCH: u32 = 0xc0000024;
const OBJECT_NAME_NOT_FOUND: u32 = 0xc0000034;
static INFERRED_NAMESPACE_ANNOUNCED: AtomicBool = AtomicBool::new(false);

pub(crate) fn reset_namespace_announcement() {
    INFERRED_NAMESPACE_ANNOUNCED.store(false, Ordering::Relaxed);
}

#[cfg(not(test))]
fn announce_inferred_namespace_policy() {
    if !INFERRED_NAMESPACE_ANNOUNCED.swap(true, Ordering::Relaxed) {
        let message = CString::new("kernel: INFERRED namespace policy: root 0xfffffffd + absolute drive names resolve through the title's existing \\??\\ links; other roots remain refused\n").expect("static diagnostic has no NUL");
        unsafe { kernel_hle_log_message(message.as_ptr()) };
    }
}

#[cfg(test)]
fn announce_inferred_namespace_policy() {
    INFERRED_NAMESPACE_ANNOUNCED.store(true, Ordering::Relaxed);
}

#[repr(C)]
#[derive(Default)]
struct ObjectEntry {
    handle: u32, kind: u32, owner_tag: u32, references: u32,
    in_use: bool, handle_open: bool,
    thread_body: u32, thread_body_references: u32,
    event_type: u32, event_initial_state: u32, event_signaled: bool,
    mutant_owner: u32, mutant_count: u32, file_body: u32, file_fs_name: u32,
    file_body_references: u32, file_dup_of: u32,
}

#[cfg(test)]
mod abi_tests {
    use super::{Attributes, ObjectEntry, argument_failure_message, contradictory_create_options,
        create_option_conflict_message, fabricated_open_message, open_refused_message,
        unmodelled_attributes_message};
    use std::mem::{offset_of, size_of};

    #[test]
    fn object_snapshot_and_file_attributes_match_c_layouts() {
        assert_eq!(size_of::<ObjectEntry>(), 64);
        assert_eq!(offset_of!(ObjectEntry, in_use), 16);
        assert_eq!(offset_of!(ObjectEntry, handle_open), 17);
        assert_eq!(offset_of!(ObjectEntry, thread_body), 20);
        assert_eq!(offset_of!(ObjectEntry, event_signaled), 36);
        assert_eq!(offset_of!(ObjectEntry, mutant_owner), 40);
        assert_eq!(offset_of!(ObjectEntry, file_body), 48);
        assert_eq!(offset_of!(ObjectEntry, file_dup_of), 60);
        assert_eq!(size_of::<Attributes>(), 40);
        assert_eq!(offset_of!(Attributes, backing), 0);
        assert_eq!(offset_of!(Attributes, is_directory), 4);
        assert_eq!(offset_of!(Attributes, times_known), 5);
        assert_eq!(offset_of!(Attributes, size), 8);
        assert_eq!(offset_of!(Attributes, last_write_time), 32);
    }

    #[test]
    fn contradictory_create_options_are_rejected() {
        assert!(contradictory_create_options(0x1 | 0x40));
        assert!(!contradictory_create_options(0x1));
        assert!(!contradictory_create_options(0x40));
    }

    #[test]
    fn create_option_conflict_diagnostic_matches_contract() {
        let message = create_option_conflict_message(b"\\Device\\Disk\\TDATA", 0x41);
        assert!(message.contains("FILE_DIRECTORY_FILE"));
        assert!(message.contains("those contradict"));
        assert!(message.contains("0x41"));
    }

    #[test]
    fn native_open_diagnostic_messages_match_contract() {
        assert!(argument_failure_message("NtOpenFile", Some(5)).contains("could not read argument 5"));
        assert!(argument_failure_message("NtOpenFile", None).contains("no argument frame"));
        assert!(open_refused_message(b"\\??\\W:", 0xc0000034, 0, 0, 0).contains("REFUSED"));
        assert!(fabricated_open_message(b"\\Device\\Disk\\missing", 7).contains("FABRICATED"));
        assert!(unmodelled_attributes_message("NtOpenFile", b"\\Device\\Disk\\file", 0x80)
            .contains("NtOpenFile(\"\\Device\\Disk\\file\") OBJECT_ATTRIBUTES.attributes is 0x80"));
    }
}

fn contradictory_create_options(options: u32) -> bool {
    options & 0x1 != 0 && options & 0x40 != 0
}

fn create_option_conflict_message(path: &[u8], options: u32) -> String {
    format!(
        "kernel: NtCreateFile(\"{}\") set both FILE_DIRECTORY_FILE and FILE_NON_DIRECTORY_FILE in options {options:#x} -- REFUSED, those contradict\n",
        String::from_utf8_lossy(path),
    )
}

fn create_refused_message(path: &[u8], disposition: u32, status: u32) -> String {
    format!("kernel: NtCreateFile(\"{}\", disposition {}) REFUSED with status {status:#010x} -- NOTHING WAS CREATED. Creation needs a WRITABLE volume mounted behind the name (see --hdd); the disc is read-only and a name with nothing behind it has nowhere to create\n",
        String::from_utf8_lossy(path), disposition)
}

fn argument_failure_message(ordinal: &str, index: Option<usize>) -> String {
    match index {
        Some(index) => format!("kernel: {ordinal} could not read argument {index} from the guest stack\n"),
        None => format!("kernel: {ordinal} called with no argument frame\n"),
    }
}

fn open_refused_message(path: &[u8], status: u32, desired: u32, share: u32, options: u32) -> String {
    format!("kernel: NtOpenFile(\"{}\") REFUSED with status {status:#010x} (access {desired:#x} share {share:#x} options {options:#x})\n",
        String::from_utf8_lossy(path))
}

fn fabricated_open_message(path: &[u8], handle: u32) -> String {
    format!("kernel: NtOpenFile(\"{}\") -> handle {handle:#x} for an EMPTY, FABRICATED file (no volume is mounted behind that name; everything the title reads through this handle is ours)\n",
        String::from_utf8_lossy(path))
}

fn unmodelled_attributes_message(ordinal: &str, path: &[u8], attributes: u32) -> String {
    format!("kernel: {ordinal}(\"{}\") OBJECT_ATTRIBUTES.attributes is {attributes:#x}, which this host does not model -- names are always matched case-insensitively and the other bits are ignored\n",
        String::from_utf8_lossy(path))
}

#[cfg(not(test))]
fn log_guest_diagnostic(text: String) {
    let message = CString::new(text).expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

#[cfg(test)]
fn log_guest_diagnostic(_text: String) {}

fn log_unmodelled_attributes(ordinal: &str, path: &[u8], address: u32) {
    let Some([_, _, attributes]) = guest_memory::attributes(&mut HostMemory, Address(address)) else { return };
    if attributes != 0x40 {
        log_guest_diagnostic(unmodelled_attributes_message(ordinal, path, attributes));
    }
}

fn trailing_volume_prefix_message(prefix: &[u8]) -> String {
    let trimmed = prefix.strip_suffix(b"\\").or_else(|| prefix.strip_suffix(b"/"))
        .unwrap_or(prefix);
    format!("kernel: cannot mount \"{}\": a volume prefix must NOT end in a separator, or paths under it will not resolve. Pass \"{}\" instead\n",
        String::from_utf8_lossy(prefix), String::from_utf8_lossy(trimmed))
}

fn invalid_host_mount_message(prefix: &[u8], path: &[u8]) -> String {
    format!("kernel: cannot mount \"{}\" as writable storage on \"{}\": unavailable (it must already exist and be a DIRECTORY -- nothing is created here, because a typo would otherwise produce an empty hard disk somewhere you did not mean)\n",
        String::from_utf8_lossy(path), String::from_utf8_lossy(prefix))
}

fn created_directory_message(path: &[u8]) -> String {
    format!("kernel: CREATED directory \"{}\" -- a real directory on a real filesystem, not a fabrication\n",
        String::from_utf8_lossy(path))
}

fn created_file_message(path: &[u8], writable: bool) -> String {
    let disposition = if writable {
        "opened FOR WRITING: NtWriteFile through this handle puts real bytes in it"
    } else {
        "opened READ-ONLY because the create did not ask for write access"
    };
    format!("kernel: created file \"{}\", {disposition}\n", String::from_utf8_lossy(path))
}

fn directory_read_refused_message(path: &[u8]) -> String {
    format!("kernel: read on \"{}\", which is a DIRECTORY -- REFUSED. A directory has no byte stream to read\n",
        String::from_utf8_lossy(path))
}

fn write_refused_message(path: &[u8], backing: u32, writable: bool, directory: bool) -> String {
    let reason = if directory {
        "which is a DIRECTORY -- REFUSED. A directory has no byte stream to write into"
    } else if backing == 1 {
        "REFUSED -- its backing is the user's READ-ONLY disc image; nothing was written"
    } else if backing == 0 {
        "REFUSED -- its backing is a FABRICATED empty file with no host object behind it"
    } else if !writable {
        "REFUSED -- that handle was opened WITHOUT write access, so its host descriptor is read-only"
    } else {
        "REFUSED -- the backing cannot be written"
    };
    format!("kernel: NtWriteFile on \"{}\" {reason}\n", String::from_utf8_lossy(path))
}

fn overwrite_disposition_refused_message(path: &[u8]) -> String {
    format!("kernel: NtCreateFile(\"{}\") would OVERWRITE an existing name, but this backing cannot be truncated\n",
        String::from_utf8_lossy(path))
}

fn truncated_to_zero_message(path: &[u8], disposition: u32) -> String {
    format!("kernel: NtCreateFile(\"{}\") -> an existing file TRUNCATED TO ZERO for disposition {}\n",
        String::from_utf8_lossy(path), disposition)
}

fn device_mount_message(prefix: &[u8], path: &[u8], name: &[u8], capacity: u64, cache: Option<u32>) -> String {
    if let Some(number) = cache {
        format!("kernel: CACHE PARTITION {} \"{}\" mounted, FABRICATED: raw view \"{}\" is backed by \"{}\" ({} bytes); directory view is prepared after format\n",
            number, String::from_utf8_lossy(prefix), String::from_utf8_lossy(name), String::from_utf8_lossy(path), capacity)
    } else {
        format!("kernel: VIRTUAL DEVICE \"{}\" mounted, FABRICATED: regular file \"{}\" in \"{}\" with {} bytes of capacity; unwritten bytes read as zeros\n",
            String::from_utf8_lossy(prefix), String::from_utf8_lossy(name), String::from_utf8_lossy(path), capacity)
    }
}

fn device_open_message(path: &[u8]) -> String {
    format!("kernel: \"{}\" -> VIRTUAL raw device, FABRICATED: host file contents are exposed as a bounded device\n",
        String::from_utf8_lossy(path))
}

fn device_zero_fill_message(path: &[u8], offset: u64, count: usize) -> String {
    format!("kernel: read of virtual device \"{}\" at offset {}: {} byte(s) were never written and are FABRICATED zeros\n",
        String::from_utf8_lossy(path), offset, count)
}

fn device_write_range_message(path: &[u8], offset: u64, length: usize, capacity: u64) -> String {
    format!("kernel: write to virtual device \"{}\" at offset {} length {} runs past its {}-byte capacity -- REFUSED\n",
        String::from_utf8_lossy(path), offset, length, capacity)
}

fn device_fixed_length_message(path: &[u8]) -> String {
    format!("kernel: resize of virtual device \"{}\" REFUSED: a raw device has a fixed length\n",
        String::from_utf8_lossy(path))
}

fn device_mount_nonregular_message(prefix: &[u8], path: &[u8], name: &[u8]) -> String {
    format!("kernel: cannot mount \"{}\" as a virtual device: \"{}\" in \"{}\" exists and is not a REGULAR file -- REFUSED\n",
        String::from_utf8_lossy(prefix), String::from_utf8_lossy(name), String::from_utf8_lossy(path))
}

fn directory_query_not_implemented_message(path: &[u8]) -> String {
    format!("kernel: directory query on \"{}\" is NOT IMPLEMENTED for this backing\n", String::from_utf8_lossy(path))
}

fn cache_format_message(name: &[u8], formatted: bool, content: &[u8], created: bool) -> String {
    if formatted {
        format!("kernel: cache image \"{}\" is FORMATTED as FATX; directory view \"{}\" {}\n",
            String::from_utf8_lossy(name), String::from_utf8_lossy(content),
            if created { "CREATED just now" } else { "it already existed" })
    } else {
        format!("kernel: cache image \"{}\" is NOT FORMATTED as FATX\n", String::from_utf8_lossy(name))
    }
}

fn cache_directory_refused_message(content: &[u8]) -> String {
    format!("kernel: cache directory view \"{}\" is not a plain directory -- REFUSED\n",
        String::from_utf8_lossy(content))
}

#[cfg(not(test))]
pub(crate) fn log_mount_refused(prefix: &[u8], path: &[u8], trailing_prefix: bool) {
    let text = if trailing_prefix { trailing_volume_prefix_message(prefix) }
        else { invalid_host_mount_message(prefix, path) };
    let message = CString::new(text).expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

#[cfg(not(test))]
pub(crate) fn log_created_directory(path: &[u8]) {
    let message = CString::new(created_directory_message(path))
        .expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

#[cfg(not(test))]
pub(crate) fn log_created_file(path: &[u8], writable: bool) {
    let message = CString::new(created_file_message(path, writable))
        .expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

#[cfg(not(test))]
pub(crate) fn log_directory_read_refused(path: &[u8]) {
    let message = CString::new(directory_read_refused_message(path))
        .expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

#[cfg(not(test))]
pub(crate) fn log_write_refused(path: &[u8], backing: u32, writable: bool, directory: bool) {
    let message = CString::new(write_refused_message(path, backing, writable, directory))
        .expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

#[cfg(not(test))]
pub(crate) fn log_overwrite_disposition_refused(path: &[u8]) {
    let message = CString::new(overwrite_disposition_refused_message(path))
        .expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

#[cfg(not(test))]
pub(crate) fn log_truncated_to_zero(path: &[u8], disposition: u32) {
    let message = CString::new(truncated_to_zero_message(path, disposition))
        .expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

#[cfg(not(test))]
pub(crate) fn log_device_mount(prefix: &[u8], path: &[u8], name: &[u8], capacity: u64, cache: Option<u32>) {
    let message = CString::new(device_mount_message(prefix, path, name, capacity, cache))
        .expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

#[cfg(not(test))]
pub(crate) fn log_device_open(path: &[u8]) {
    let message = CString::new(device_open_message(path)).expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

#[cfg(not(test))]
pub(crate) fn log_device_zero_fill(path: &[u8], offset: u64, count: usize) {
    let message = CString::new(device_zero_fill_message(path, offset, count))
        .expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

#[cfg(not(test))]
pub(crate) fn log_device_write_past_capacity(path: &[u8], offset: u64, length: usize, capacity: u64) {
    let message = CString::new(device_write_range_message(path, offset, length, capacity))
        .expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

#[cfg(not(test))]
pub(crate) fn log_device_fixed_length(path: &[u8]) {
    let message = CString::new(device_fixed_length_message(path)).expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

#[cfg(not(test))]
pub(crate) fn log_device_mount_nonregular(prefix: &[u8], path: &[u8], name: &[u8]) {
    let message = CString::new(device_mount_nonregular_message(prefix, path, name))
        .expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

#[cfg(not(test))]
pub(crate) fn log_delete_refused(path: &[u8], reason: &str) {
    let text = format!("kernel: NtDeleteFile {:?} REFUSED: {}\n", String::from_utf8_lossy(path), reason);
    let message = CString::new(text).expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

#[cfg(not(test))]
pub(crate) fn log_delete_succeeded(path: &[u8]) {
    let text = format!("kernel: NtDeleteFile {:?} DELETED\n", String::from_utf8_lossy(path));
    let message = CString::new(text).expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

#[cfg(not(test))]
pub(crate) fn log_directory_query_not_implemented(path: &[u8]) {
    let message = CString::new(directory_query_not_implemented_message(path))
        .expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

#[cfg(not(test))]
pub(crate) fn log_cache_not_formatted(name: &[u8]) {
    let message = CString::new(cache_format_message(name, false, b"", false)).expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

#[cfg(not(test))]
pub(crate) fn log_cache_formatted(name: &[u8], content: &[u8], created: bool) {
    let message = CString::new(cache_format_message(name, true, content, created)).expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

#[cfg(not(test))]
pub(crate) fn log_cache_directory_refused(content: &[u8]) {
    let message = CString::new(cache_directory_refused_message(content)).expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

#[cfg(test)]
pub(crate) fn log_mount_refused(_prefix: &[u8], _path: &[u8], _trailing_prefix: bool) {}

#[cfg(test)]
pub(crate) fn log_created_directory(_path: &[u8]) {}

#[cfg(test)]
pub(crate) fn log_created_file(_path: &[u8], _writable: bool) {}

#[cfg(test)]
pub(crate) fn log_directory_read_refused(_path: &[u8]) {}

#[cfg(test)]
pub(crate) fn log_write_refused(_path: &[u8], _backing: u32, _writable: bool, _directory: bool) {}

#[cfg(test)]
pub(crate) fn log_overwrite_disposition_refused(_path: &[u8]) {}

#[cfg(test)]
pub(crate) fn log_truncated_to_zero(_path: &[u8], _disposition: u32) {}

#[cfg(test)]
pub(crate) fn log_device_mount(_prefix: &[u8], _path: &[u8], _name: &[u8], _capacity: u64, _cache: Option<u32>) {}

#[cfg(test)]
pub(crate) fn log_device_open(_path: &[u8]) {}

#[cfg(test)]
pub(crate) fn log_device_zero_fill(_path: &[u8], _offset: u64, _count: usize) {}

#[cfg(test)]
pub(crate) fn log_device_write_past_capacity(_path: &[u8], _offset: u64, _length: usize, _capacity: u64) {}

#[cfg(test)]
pub(crate) fn log_device_fixed_length(_path: &[u8]) {}

#[cfg(test)]
pub(crate) fn log_device_mount_nonregular(_prefix: &[u8], _path: &[u8], _name: &[u8]) {}

#[cfg(test)]
pub(crate) fn log_delete_refused(_path: &[u8], _reason: &str) {}

#[cfg(test)]
pub(crate) fn log_delete_succeeded(_path: &[u8]) {}

#[cfg(test)]
pub(crate) fn log_directory_query_not_implemented(_path: &[u8]) {}

#[cfg(test)]
pub(crate) fn log_cache_not_formatted(_name: &[u8]) {}

#[cfg(test)]
pub(crate) fn log_cache_formatted(_name: &[u8], _content: &[u8], _created: bool) {}

#[cfg(test)]
pub(crate) fn log_cache_directory_refused(_content: &[u8]) {}

#[cfg(not(test))]
pub(crate) fn log_create_refused(path: &[u8], disposition: u32, status: u32) {
    let message = CString::new(create_refused_message(path, disposition, status))
        .expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

#[cfg(test)]
pub(crate) fn log_create_refused(_path: &[u8], _disposition: u32, _status: u32) {}

fn log_create_option_conflict(path: &[u8], options: u32) {
    let message = CString::new(create_option_conflict_message(path, options))
        .expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

fn path_escape_message(path: &[u8]) -> String {
    format!("kernel: \"{}\" contains a \".\" or \"..\" component -- REFUSED. A guest path may not leave the directory backing the volume\n",
        String::from_utf8_lossy(path))
}

#[cfg(not(test))]
pub(crate) fn log_path_escape(path: &[u8]) {
    let message = CString::new(path_escape_message(path)).expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

#[cfg(test)]
pub(crate) fn log_path_escape(_path: &[u8]) {}

fn host_symlink_message(path: &[u8]) -> String {
    format!("kernel: \"{}\" names a host SYMBOLIC LINK -- REFUSED, because following it could leave the backing directory\n",
        String::from_utf8_lossy(path))
}

#[cfg(not(test))]
pub(crate) fn log_host_symlink(path: &[u8]) {
    let message = CString::new(host_symlink_message(path)).expect("formatted diagnostic has no NUL");
    unsafe { kernel_hle_log_message(message.as_ptr()) };
}

#[cfg(test)]
pub(crate) fn log_host_symlink(_path: &[u8]) {}

unsafe extern "C" {
    fn kernel_hle_log_message(message: *const c_char);
}

type Handler = unsafe extern "C" fn(*mut c_void) -> u32;
unsafe extern "C" {
    fn kernel_guest_read_u8(address: u32, out: *mut u8) -> bool;
    fn kernel_guest_read_u32(address: u32, out: *mut u32) -> bool;
    fn kernel_guest_write_u8(address: u32, value: u8) -> bool;
    fn kernel_guest_write_u32(address: u32, value: u32) -> bool;
    fn kernel_frame_arg(frame: *const c_void, index: u32, out: *mut u32) -> bool;
    fn kernel_object_create(kind: u32, owner: u32) -> u32;
    fn kernel_object_release(handle: u32) -> bool;
    fn kernel_object_get_copy(handle: u32, out: *mut ObjectEntry) -> bool;
    fn kernel_hle_register(ordinal: u32, handler: Handler) -> bool;
}

pub(crate) fn object_kind(handle: u32) -> Option<u32> {
    let mut entry = ObjectEntry::default();
    unsafe { kernel_object_get_copy(handle, &mut entry) }.then_some(entry.kind)
}
pub(crate) fn object_live(handle: u32) -> bool { object_kind(handle).is_some() }

struct HostMemory;
impl NameMemory for HostMemory {
    fn byte(&mut self, a: Address) -> Option<u8> {
        let mut value = 0;
        unsafe { kernel_guest_read_u8(a.0, &mut value) }.then_some(value)
    }
    fn word(&mut self, a: Address) -> Option<u32> {
        let mut value = 0;
        unsafe { kernel_guest_read_u32(a.0, &mut value) }.then_some(value)
    }
}
impl Memory for HostMemory {
    fn put_byte(&mut self, a: Address, value: u8) -> bool {
        unsafe { kernel_guest_write_u8(a.0, value) }
    }
    fn put_word(&mut self, a: Address, value: u32) -> bool {
        unsafe { kernel_guest_write_u32(a.0, value) }
    }
}

unsafe fn arguments<const N: usize>(context: *mut c_void) -> Option<[u32; N]> {
    if context.is_null() { return None; }
    let mut values = [0; N];
    for (i, out) in values.iter_mut().enumerate() {
        if !unsafe { kernel_frame_arg(context, i as u32, out) } { return None; }
    }
    Some(values)
}

unsafe fn arguments_with_failure<const N: usize>(context: *mut c_void) -> Result<[u32; N], Option<usize>> {
    if context.is_null() { return Err(None); }
    let mut values = [0; N];
    for (i, out) in values.iter_mut().enumerate() {
        if !unsafe { kernel_frame_arg(context, i as u32, out) } { return Err(Some(i)); }
    }
    Ok(values)
}

fn object_path(address: u32) -> Result<Vec<u8>, u32> {
    if address == 0 { return Err(INVALID_PARAMETER); }
    let [root, name, attributes] = guest_memory::attributes(&mut HostMemory, Address(address))
        .ok_or(INVALID_PARAMETER)?;
    let name = guest_memory::name(&mut HostMemory, Address(name))?;
    if attributes != 0x40 {
        runtime().lock().unwrap().counters.unmodelled_attributes += 1;
    }
    let result = guest_memory::rooted(root, &name, PATH_CAPACITY);
    if result.is_err() {
        runtime().lock().unwrap().counters.relative_refused += 1;
    } else if root == 0xFFFF_FFFD {
        announce_inferred_namespace_policy();
    }
    result
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_read_object_attributes(
    address: u32, root: *mut u32, name: *mut u32, attributes: *mut u32,
) -> bool {
    let outputs = [root, name, attributes];
    let index = |field| match field {
        AttributeField::RootDirectory => 0,
        AttributeField::ObjectName => 1,
        AttributeField::Attributes => 2,
    };
    guest_names::read_attributes(&mut HostMemory, Address(address),
        |field| !outputs[index(field)].is_null(),
        |field, value| unsafe { outputs[index(field)].write(value) })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_prepare_root_path(root: u32, path: *mut c_char, capacity: usize) -> bool {
    if root == 0 { return true; }
    if path.is_null() || capacity == 0 || capacity > isize::MAX as usize { return false; }
    let bytes = unsafe { std::slice::from_raw_parts_mut(path.cast::<u8>(), capacity) };
    let Some(end) = bytes.iter().position(|&b| b == 0) else { return false };
    let Ok(expanded) = guest_memory::rooted(root, &bytes[..end], capacity) else { return false };
    bytes[..expanded.len()].copy_from_slice(&expanded);
    bytes[expanded.len()] = 0;
    if root == 0xFFFF_FFFD { announce_inferred_namespace_policy(); }
    true
}

fn publish_status(address: u32, status: u32, information: u32) -> u32 {
    if guest_memory::io_status(&mut HostMemory, Address(address), status, information) {
        status
    } else { INVALID_PARAMETER }
}

unsafe fn open(context: *mut c_void, create: bool, stored_only: bool) -> u32 {
    let (handle_out, desired, attributes, io_status, share, disposition, options, ordinal) = if create {
        let a = match unsafe { arguments_with_failure::<9>(context) } {
            Ok(a) => a,
            Err(index) => { log_guest_diagnostic(argument_failure_message("NtCreateFile", index)); return INVALID_PARAMETER; }
        };
        (a[0], a[1], a[2], a[3], a[6], a[7], a[8], 190)
    } else {
        let a = match unsafe { arguments_with_failure::<6>(context) } {
            Ok(a) => a,
            Err(index) => { log_guest_diagnostic(argument_failure_message("NtOpenFile", index)); return INVALID_PARAMETER; }
        };
        (a[0], a[1], a[2], a[3], a[4], 1, a[5], 202)
    };
    if handle_out == 0 || attributes == 0 {
        return publish_status(io_status, INVALID_PARAMETER, 0);
    }
    if create && contradictory_create_options(options) {
        if let Ok(path) = object_path(attributes) {
            log_create_option_conflict(&path, options);
        }
        return publish_status(io_status, INVALID_PARAMETER, 0);
    }
    let path = match object_path(attributes) {
        Ok(path) => path,
        Err(status) => return publish_status(io_status, status, 0),
    };
    log_unmodelled_attributes(if create { "NtCreateFile" } else { "NtOpenFile" }, &path, attributes);
    // Read the path before publishing: callers may alias handle_out with their
    // input ANSI_STRING pointer. Allocate through the shared object service.
    crate::ffi::kernel_file_sweep_closed();
    let mut allocated = 0;
    let (attempt, result, fabricated) = {
        let mut rt = runtime().lock().unwrap();
        let fabricated_before = rt.counters.fabricated;
        let attempt = rt.record_attempt(&path, desired, share, options);
        let result = rt.open_allocate(&path, desired, share, options, disposition, stored_only, || {
            allocated = unsafe { kernel_object_create(2, ordinal) };
            allocated
        });
        (attempt, result, rt.counters.fabricated != fabricated_before)
    };
    let (handle, information) = match result {
        Ok(opened) => opened,
        Err(status) => {
            if !stored_only { log_guest_diagnostic(open_refused_message(&path, status, desired, share, options)); }
            if allocated != 0 { unsafe { kernel_object_release(allocated) }; }
            return publish_status(io_status, status, if status == 0xc0000035 { 4 } else { 0 });
        }
    };
    if !HostMemory.put_word(Address(handle_out), handle) {
        return publish_status(io_status, INVALID_PARAMETER, 0);
    }
    let status = publish_status(io_status, 0, information);
    if status == 0 {
        runtime().lock().unwrap().mark_opened(attempt);
        crate::ffi::observe(1, &path, 0);
        if fabricated { log_guest_diagnostic(fabricated_open_message(&path, handle)); }
    }
    status
}

unsafe extern "C" fn nt_open_file(context: *mut c_void) -> u32 { unsafe { open(context, false, false) } }
unsafe extern "C" fn nt_create_file(context: *mut c_void) -> u32 { unsafe { open(context, true, false) } }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_open_stored(context: *mut c_void) -> u32 {
    unsafe { open(context, false, true) }
}

#[repr(C)]
#[derive(Default)]
pub struct Attributes {
    pub backing: u32, pub is_directory: bool, pub times_known: bool,
    pub size: u64, pub creation_time: u64, pub last_access_time: u64, pub last_write_time: u64,
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_query_attributes(address: u32, out: *mut Attributes) -> u32 {
    if out.is_null() { return INVALID_PARAMETER; }
    unsafe { out.write(Attributes::default()) };
    let path = match object_path(address) { Ok(p) => p, Err(s) => return s };
    let mut rt = runtime().lock().unwrap();
    rt.counters.attribute_queries += 1;
    match rt.query(&path) {
        Ok(meta) => {
            unsafe { out.write(Attributes {
                backing: meta.backing, is_directory: meta.directory, times_known: meta.times_known,
                size: meta.size, creation_time: meta.creation,
                last_access_time: meta.access, last_write_time: meta.write,
            }) };
            0
        }
        Err(s) => s,
    }
}

impl RenameHost for HostMemory {
    fn relative_refused(&mut self, _root: u32, _name: &[u8]) {}
    fn rename(&mut self, handle: u32, replace: bool, name: &[u8]) -> u32 {
        let Ok(name) = std::ffi::CString::new(name) else { return INVALID_PARAMETER };
        unsafe { crate::ffi::kernel_file_rename(handle, replace, name.as_ptr()) }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_rename_from_guest(handle: u32, address: u32, length: u32) -> u32 {
    guest_names::rename_from_guest(&mut HostMemory, handle, Address(address), length)
}

unsafe extern "C" fn nt_delete_file(context: *mut c_void) -> u32 {
    let Some([attributes]) = (unsafe { arguments(context) }) else { return INVALID_PARAMETER };
    crate::ffi::kernel_file_sweep_closed();
    match object_path(attributes) {
        Ok(path) => runtime().lock().unwrap().delete(&path),
        Err(status) => status,
    }
}
unsafe extern "C" fn io_create_link(context: *mut c_void) -> u32 {
    let Some([name, target]) = (unsafe { arguments(context) }) else { return INVALID_PARAMETER };
    let (Ok(name), Ok(target)) = (guest_memory::name(&mut HostMemory, Address(name)),
        guest_memory::name(&mut HostMemory, Address(target))) else { return INVALID_PARAMETER };
    runtime().lock().unwrap().add_link(&name, &target, false).err().unwrap_or(0)
}
unsafe extern "C" fn io_delete_link(context: *mut c_void) -> u32 {
    let Some([name]) = (unsafe { arguments(context) }) else { return INVALID_PARAMETER };
    let Ok(name) = guest_memory::name(&mut HostMemory, Address(name)) else { return INVALID_PARAMETER };
    if runtime().lock().unwrap().remove_link(&name) { 0 } else { OBJECT_NAME_NOT_FOUND }
}
unsafe extern "C" fn nt_open_link(context: *mut c_void) -> u32 {
    let [out, attributes] = match unsafe { arguments_with_failure::<2>(context) } {
        Ok(a) => a,
        Err(index) => { log_guest_diagnostic(argument_failure_message("NtOpenSymbolicLinkObject", index)); return INVALID_PARAMETER; }
    };
    if out == 0 { return INVALID_PARAMETER; }
    let path = match object_path(attributes) { Ok(p) => p, Err(s) => return s };
    log_unmodelled_attributes("NtOpenSymbolicLinkObject", &path, attributes);
    let mut rt = runtime().lock().unwrap();
    let Some(target) = rt.link_target(&path) else {
        drop(rt);
        log_guest_diagnostic(format!("kernel: NtOpenSymbolicLinkObject(\"{}\") -- no such link was created by the title, reporting STATUS_OBJECT_NAME_NOT_FOUND\n", String::from_utf8_lossy(&path)));
        return OBJECT_NAME_NOT_FOUND;
    };
    rt.symlink_handles.retain(|(handle, _)| object_live(*handle));
    if rt.symlink_handles.len() >= 16 { return INSUFFICIENT_RESOURCES; }
    let handle = unsafe { kernel_object_create(6, 203) };
    if handle == 0 { return INSUFFICIENT_RESOURCES; }
    rt.symlink_handles.push((handle, target));
    drop(rt);
    if HostMemory.put_word(Address(out), handle) { 0 } else {
        log_guest_diagnostic(format!("kernel: NtOpenSymbolicLinkObject(\"{}\") could not write the handle to {out:#x}\n", String::from_utf8_lossy(&path)));
        INVALID_PARAMETER
    }
}
unsafe extern "C" fn nt_query_link(context: *mut c_void) -> u32 {
    let Some([handle, string, returned]) = (unsafe { arguments(context) }) else { return INVALID_PARAMETER };
    match object_kind(handle) {
        None => return INVALID_HANDLE,
        Some(6) => {}
        _ => return OBJECT_TYPE_MISMATCH,
    }
    let target = runtime().lock().unwrap().symlink_handles.iter()
        .find(|(h, _)| *h == handle).map(|(_, target)| target.clone());
    let Some(target) = target else { return 0xc0000001 };
    guest_memory::write_link(&mut HostMemory, Address(string), Address(returned), &target)
}

#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_register() -> u32 {
    let bindings: [(u32, Handler); 7] = [
        (202, nt_open_file), (190, nt_create_file), (195, nt_delete_file),
        (67, io_create_link), (69, io_delete_link), (203, nt_open_link), (215, nt_query_link),
    ];
    bindings.into_iter().filter(|&(ordinal, handler)| unsafe { kernel_hle_register(ordinal, handler) }).count() as u32
}
