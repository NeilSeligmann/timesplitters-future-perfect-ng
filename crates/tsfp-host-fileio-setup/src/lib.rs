//! Typed host mount sequencing for T3879 FIO-1.
//!
//! This crate owns setup order, cache-partition names, and the decision to skip an
//! already prepared partition0. It does not own the runtime volume table. The
//! hybrid host's callback is a TEMPORARY-SHIM over that table; T3879 FIO-3 removes
//! the C table adapter. Moving setup here is not evidence of runtime retirement.
//!
//! Paths are bytes, not UTF-8 strings. The caller supplies platform limits and
//! partition0 policy from their existing owner. There is no filesystem access.

pub mod identity;

/// A guest device prefix and its explicitly selected host backing path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mount<'a> {
    pub device: &'a [u8],
    pub path: &'a [u8],
}

/// The existing host's raw configuration-area device policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Partition0<'a> {
    pub device: &'a [u8],
    pub file_name: &'a [u8],
    pub capacity: u64,
}

/// Validated host options, borrowed for one setup attempt.
#[derive(Clone, Copy, Debug)]
pub struct Config<'a> {
    pub mounts: &'a [&'a [u8]],
    pub disc: Option<Mount<'a>>,
    pub hdd: Option<Mount<'a>>,
    pub partition0: Partition0<'a>,
    /// Early ATA identity setup already mounted the configured partition0.
    pub pre_mounted: bool,
    pub window_mapped: bool,
    pub missing_empty: bool,
    pub cache_count: u32,
    pub cache_first: u32,
    pub cache_capacity: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    Empty,
    Disc,
    HostDir,
    Partition0,
    PublishCacheCount,
    CachePartition,
    MissingEmpty,
}

/// One operation and its diagnostic event, performed by the host callback.
///
/// Every callback invocation represents an event, including a skipped partition0
/// mount. Generated cache names are owned by the action and are valid throughout
/// the callback. Input paths retain their original bytes.
#[derive(Debug, PartialEq, Eq)]
pub enum Action<'a> {
    Empty {
        path: &'a [u8],
    },
    Disc {
        device: &'a [u8],
        path: &'a [u8],
    },
    HostDir {
        device: &'a [u8],
        path: &'a [u8],
    },
    Partition0 {
        device: &'a [u8],
        host_dir: &'a [u8],
        file_name: &'a [u8],
        capacity: u64,
        skip_mount: bool,
    },
    PublishCacheCount {
        count: u32,
    },
    CachePartition {
        device: Vec<u8>,
        host_dir: &'a [u8],
        number: u32,
        image_name: Vec<u8>,
        content_dir: Vec<u8>,
        capacity: u64,
    },
    MissingEmpty,
}

impl Action<'_> {
    pub fn kind(&self) -> ActionKind {
        match self {
            Self::Empty { .. } => ActionKind::Empty,
            Self::Disc { .. } => ActionKind::Disc,
            Self::HostDir { .. } => ActionKind::HostDir,
            Self::Partition0 { .. } => ActionKind::Partition0,
            Self::PublishCacheCount { .. } => ActionKind::PublishCacheCount,
            Self::CachePartition { .. } => ActionKind::CachePartition,
            Self::MissingEmpty => ActionKind::MissingEmpty,
        }
    }
}

/// The first rejected action. Earlier successful actions remain in effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunError {
    /// Zero-based callback index, including the failed callback.
    pub index: usize,
    pub kind: ActionKind,
}

fn emit(
    action: Action<'_>,
    index: &mut usize,
    execute: &mut impl FnMut(&Action<'_>) -> bool,
) -> Result<(), RunError> {
    let current = *index;
    *index += 1;
    if execute(&action) {
        Ok(())
    } else {
        Err(RunError {
            index: current,
            kind: action.kind(),
        })
    }
}

/// Execute ordinary startup mounts in their established order.
///
/// The callback performs the operation and emits the corresponding success or
/// failure diagnostic. A false result stops immediately. No rollback or implicit
/// unmount occurs: the caller retains the original startup failure policy.
///
/// ATA identity preparation happens earlier and separately. When it succeeded,
/// `pre_mounted` skips the duplicate mount but still emits the partition0 event.
pub fn run(
    config: &Config<'_>,
    mut execute: impl FnMut(&Action<'_>) -> bool,
) -> Result<(), RunError> {
    let mut index = 0;
    for path in config.mounts {
        emit(Action::Empty { path }, &mut index, &mut execute)?;
    }
    if let Some(mount) = config.disc {
        emit(
            Action::Disc {
                device: mount.device,
                path: mount.path,
            },
            &mut index,
            &mut execute,
        )?;
    }
    if let Some(mount) = config.hdd {
        emit(
            Action::HostDir {
                device: mount.device,
                path: mount.path,
            },
            &mut index,
            &mut execute,
        )?;
        emit(
            Action::Partition0 {
                device: config.partition0.device,
                host_dir: mount.path,
                file_name: config.partition0.file_name,
                capacity: config.partition0.capacity,
                skip_mount: config.pre_mounted,
            },
            &mut index,
            &mut execute,
        )?;
        if config.cache_count != 0 && config.window_mapped {
            emit(
                Action::PublishCacheCount {
                    count: config.cache_count,
                },
                &mut index,
                &mut execute,
            )?;
        }
        for slot in 0..config.cache_count {
            // Match the original unsigned C arithmetic. Accepted host options
            // constrain the range; this layer does not duplicate their limits.
            let number = config.cache_first.wrapping_add(slot);
            emit(
                Action::CachePartition {
                    device: format!("\\Device\\Harddisk0\\Partition{number}").into_bytes(),
                    host_dir: mount.path,
                    number,
                    image_name: format!(".tsfp-cache{number}.bin").into_bytes(),
                    content_dir: format!(".tsfp-cache{number}").into_bytes(),
                    capacity: config.cache_capacity,
                },
                &mut index,
                &mut execute,
            )?;
        }
    }
    if config.missing_empty {
        emit(Action::MissingEmpty, &mut index, &mut execute)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
