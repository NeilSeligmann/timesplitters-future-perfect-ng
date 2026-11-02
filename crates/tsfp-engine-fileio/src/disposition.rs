//! NtCreateFile CreateDisposition decision (T2074 slice 7). `kernel_file.c` keeps the resolution, the truncation, the
//! creation and the IO_STATUS block; this table says which of those the call takes and which information code it reports.
//! Same allowlists as the C original: an unknown disposition neither creates nor overwrites.

pub const FILE_SUPERSEDE: u32 = 0;
pub const FILE_OPEN: u32 = 1;
pub const FILE_CREATE: u32 = 2;
pub const FILE_OPEN_IF: u32 = 3;
pub const FILE_OVERWRITE: u32 = 4;
pub const FILE_OVERWRITE_IF: u32 = 5;

pub const INFO_SUPERSEDED: u32 = 0;
pub const INFO_OPENED: u32 = 1;
pub const INFO_CREATED: u32 = 2;
pub const INFO_OVERWRITTEN: u32 = 3;
pub const INFO_EXISTS: u32 = 4;

/// What the resolution found, as far as the decision needs it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Target {
    /// The name resolved.
    pub exists: bool,
    /// The resolution failed with OBJECT_NAME_NOT_FOUND (only meaningful when `exists` is false).
    pub name_not_found: bool,
    pub fatx: bool,
    pub host_dir: bool,
    pub is_directory: bool,
    pub writable: bool,
    pub has_host_fd: bool,
    pub device: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    /// FILE_CREATE on an existing name: OBJECT_NAME_COLLISION, information FILE_EXISTS.
    Collision,
    /// Truncate a FATX file to 0 (FATX reports its own refusal for a directory or read-only open).
    TruncateFatx { information: u32 },
    /// Truncate a writable host-directory file to 0.
    TruncateHost { information: u32 },
    /// Overwrite asked on a backing this host cannot truncate: ACCESS_DENIED.
    RefuseOverwrite,
    /// Open the existing object untouched, FILE_OPENED.
    Open,
    /// Name missing and the disposition creates: try the writable volume behind it.
    TryCreate,
    /// Name missing and nothing to do: report the resolution's status.
    Refuse,
}

pub fn creates(disposition: u32) -> bool {
    matches!(disposition, FILE_SUPERSEDE | FILE_CREATE | FILE_OPEN_IF | FILE_OVERWRITE_IF)
}

pub fn overwrites(disposition: u32) -> bool {
    matches!(disposition, FILE_SUPERSEDE | FILE_OVERWRITE | FILE_OVERWRITE_IF)
}

pub fn plan(disposition: u32, target: &Target) -> Plan {
    if !target.exists {
        return if target.name_not_found && creates(disposition) { Plan::TryCreate } else { Plan::Refuse };
    }
    if disposition == FILE_CREATE {
        return Plan::Collision;
    }
    if !overwrites(disposition) {
        return Plan::Open;
    }
    let information = if disposition == FILE_SUPERSEDE { INFO_SUPERSEDED } else { INFO_OVERWRITTEN };
    if target.fatx {
        return Plan::TruncateFatx { information };
    }
    let truncatable = target.host_dir && !target.is_directory && target.has_host_fd && target.writable && !target.device;
    if truncatable { Plan::TruncateHost { information } } else { Plan::RefuseOverwrite }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host_file() -> Target {
        Target { exists: true, host_dir: true, writable: true, has_host_fd: true, ..Target::default() }
    }

    #[test]
    fn measured_dispositions() {
        // 2 at 0x00380127 and 3 at 0x00381321 are the only ones this image passes.
        assert_eq!(plan(FILE_CREATE, &host_file()), Plan::Collision);
        assert_eq!(plan(FILE_OPEN_IF, &host_file()), Plan::Open);
        let missing = Target { name_not_found: true, ..Target::default() };
        assert_eq!(plan(FILE_CREATE, &missing), Plan::TryCreate);
        assert_eq!(plan(FILE_OPEN_IF, &missing), Plan::TryCreate);
        assert_eq!(plan(FILE_OPEN, &missing), Plan::Refuse);
    }

    #[test]
    fn overwrite_needs_a_truncatable_backing() {
        assert_eq!(plan(FILE_SUPERSEDE, &host_file()), Plan::TruncateHost { information: INFO_SUPERSEDED });
        assert_eq!(plan(FILE_OVERWRITE_IF, &host_file()), Plan::TruncateHost { information: INFO_OVERWRITTEN });
        assert_eq!(plan(FILE_OVERWRITE, &Target { device: true, ..host_file() }), Plan::RefuseOverwrite);
        assert_eq!(plan(FILE_OVERWRITE, &Target { writable: false, ..host_file() }), Plan::RefuseOverwrite);
        assert_eq!(plan(FILE_OVERWRITE, &Target { exists: true, fatx: true, ..Target::default() }),
                   Plan::TruncateFatx { information: INFO_OVERWRITTEN });
        // An unknown disposition neither creates nor overwrites.
        assert_eq!(plan(9, &host_file()), Plan::Open);
        assert_eq!(plan(9, &Target { name_not_found: true, ..Target::default() }), Plan::Refuse);
    }
}
