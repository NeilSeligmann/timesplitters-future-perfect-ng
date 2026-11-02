//! NtQueryInformationFile / NtSetInformationFile decisions (T2074 slice 2 of S1): which information classes exist, what each
//! answers for an open file, and what a set call asks the backing to do.
//!
//! The C layer (`src/xbox/kernel_io.c`) keeps the guest memory reads and writes, the IO_STATUS block and the backing mutations
//! (`kernel_file_set_end_of_file`, `kernel_file_set_allocation`, `kernel_file_open_set_offset`); this module owns the decision
//! table, a statement-for-statement port of those handlers. Parity first: a class the C handler refuses (basic 4, standard 5,
//! rename 10 and the rest) stays refused here (T3786 accepts basic 4, see `SetPlan::Basic`), enabling one is a guest-visible change that needs its own decision (`nt.rs`
//! models basic and standard, FABRICATED, and is not wired).
//! Evidence: FABRICATED spec tests plus the existing kernel_io suites run with the Rust table installed.

pub const STATUS_SUCCESS: u32 = 0;
pub const STATUS_INFO_LENGTH_MISMATCH: u32 = 0xC000_0004;
pub const STATUS_INVALID_INFO_CLASS: u32 = 0xC000_0003;

pub const CLASS_BASIC: u32 = 0x04;
pub const CLASS_INTERNAL: u32 = 0x06;
pub const CLASS_POSITION: u32 = 0x0E;
pub const CLASS_ALLOCATION: u32 = 0x13;
pub const CLASS_END_OF_FILE: u32 = 0x14;
pub const CLASS_NETWORK_OPEN: u32 = 0x22;

pub const POSITION_BYTES: u32 = 8;
pub const INTERNAL_BYTES: u32 = 8;
pub const NETWORK_OPEN_BYTES: usize = 0x38;
pub const SET_BYTES: u32 = 8;
/// FILE_BASIC_INFORMATION: four LARGE_INTEGER times and the ULONG attributes (40 with tail padding).
pub const BASIC_BYTES: u32 = 36;
pub const SECTOR_BYTES: u64 = 2048;

const ATTRIBUTE_DIRECTORY: u32 = 0x10;
const ATTRIBUTE_ARCHIVE: u32 = 0x20;

/// What the open file knows about itself.
#[derive(Clone, Copy, Debug, Default)]
pub struct FileFacts {
    pub offset: u64,
    pub size: u64,
    pub is_directory: bool,
    /// The backing is a host-directory volume (allocation is the unrounded size).
    pub host_dir: bool,
    /// The backing is the disc image and not a raw device (class 6 answers the first sector).
    pub disc_file: bool,
    pub disc_sector: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Reply {
    pub status: u32,
    /// IO_STATUS.Information.
    pub information: u32,
    /// Bytes to store at the caller's buffer (empty for a refusal).
    pub payload: Vec<u8>,
    /// The four time stamps were fabricated as zero (counted by the caller).
    pub fabricated_timestamps: bool,
    /// The class is not derived (counted and logged by the caller).
    pub unknown_class: bool,
}

impl Reply {
    fn refuse(status: u32, unknown_class: bool) -> Self {
        Self {
            status,
            information: 0,
            payload: Vec::new(),
            fabricated_timestamps: false,
            unknown_class,
        }
    }
    fn ok(payload: Vec<u8>, fabricated_timestamps: bool) -> Self {
        Self {
            status: STATUS_SUCCESS,
            information: payload.len() as u32,
            payload,
            fabricated_timestamps,
            unknown_class: false,
        }
    }
}

fn round_to_sector(size: u64) -> u64 {
    size.div_ceil(SECTOR_BYTES) * SECTOR_BYTES
}

/// NtQueryInformationFile on an open file whose output buffer is `length` bytes.
pub fn query(class: u32, length: u32, file: &FileFacts) -> Reply {
    match class {
        CLASS_POSITION => {
            if length < POSITION_BYTES {
                return Reply::refuse(STATUS_INFO_LENGTH_MISMATCH, false);
            }
            Reply::ok(file.offset.to_le_bytes().to_vec(), false)
        }
        CLASS_NETWORK_OPEN => {
            if (length as usize) < NETWORK_OPEN_BYTES {
                return Reply::refuse(STATUS_INFO_LENGTH_MISMATCH, false);
            }
            let allocation = if file.host_dir {
                file.size
            } else {
                round_to_sector(file.size)
            };
            let attributes = if file.is_directory {
                ATTRIBUTE_DIRECTORY
            } else {
                ATTRIBUTE_ARCHIVE
            };
            let mut bytes = vec![0u8; NETWORK_OPEN_BYTES];
            // four time stamps at 0x00..0x20 stay zero (fabricated, announced by the caller)
            bytes[0x20..0x28].copy_from_slice(&allocation.to_le_bytes());
            bytes[0x28..0x30].copy_from_slice(&file.size.to_le_bytes());
            bytes[0x30..0x34].copy_from_slice(&attributes.to_le_bytes());
            Reply::ok(bytes, true)
        }
        CLASS_INTERNAL if file.disc_file => {
            if length < INTERNAL_BYTES {
                return Reply::refuse(STATUS_INFO_LENGTH_MISMATCH, false);
            }
            Reply::ok(u64::from(file.disc_sector).to_le_bytes().to_vec(), false)
        }
        _ => Reply::refuse(STATUS_INVALID_INFO_CLASS, true),
    }
}

/// What a set call asks the C backing to do.
#[derive(Debug, PartialEq, Eq)]
pub enum SetPlan {
    Seek(u64),
    Allocation(u64),
    EndOfFile(u64),
    /// T3786: FILE_BASIC_INFORMATION (times, attributes) is accepted and stores nothing. The host derives attributes
    /// (directory, archive) from the backing and keeps host times, so a set that clears attributes (XAPI's recursive
    /// delete `_XapiNukeDirectory` clears them on each directory before deleting it) has nothing to change. INFERRED:
    /// a set of read-only or hidden is not kept.
    Basic,
    /// Answered without touching the backing.
    Refuse {
        status: u32,
        unknown_class: bool,
    },
}

/// NtSetInformationFile: `value` is the 64-bit payload when the caller could read it (`None` when it could not, or `length`
/// is below 8).
pub fn set_plan(class: u32, length: u32, value: Option<u64>) -> SetPlan {
    if class == CLASS_BASIC {
        return match value {
            Some(_) if length >= BASIC_BYTES => SetPlan::Basic,
            _ => SetPlan::Refuse {
                status: STATUS_INFO_LENGTH_MISMATCH,
                unknown_class: false,
            },
        };
    }
    let make: fn(u64) -> SetPlan = match class {
        CLASS_POSITION => SetPlan::Seek,
        CLASS_ALLOCATION => SetPlan::Allocation,
        CLASS_END_OF_FILE => SetPlan::EndOfFile,
        _ => {
            return SetPlan::Refuse {
                status: STATUS_INVALID_INFO_CLASS,
                unknown_class: true,
            };
        }
    };
    match value {
        Some(v) if length >= SET_BYTES => make(v),
        _ => SetPlan::Refuse {
            status: STATUS_INFO_LENGTH_MISMATCH,
            unknown_class: false,
        },
    }
}
