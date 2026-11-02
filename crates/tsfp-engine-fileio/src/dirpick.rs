//! Directory enumeration cursor selection (T2074 slice 4 of S1): which entry NtQueryDirectoryFile returns next.
//!
//! The C layer (`src/xbox/kernel_file.c`, `kernel_file_dir_next`) keeps the OS work (readdir + fstatat on a host-dir volume, FATX
//! directory bytes) and offers every candidate entry once; this module owns the decision: wildcard mask, cursor order (ASCII
//! case-folded, byte order breaking ties), "after the last name returned", the smallest remaining candidate, the buffer-length
//! refusal. The cursor is the last name returned, not an index, so a directory that changes between calls stays consistent.
//! Statement-for-statement port of the C code it replaces (the A/B partner stays in C). Evidence: FABRICATED spec tests plus the
//! C-versus-Rust differential in tests/c/t2074_dirpick_diff.c.

pub const NAME_MAX: usize = 255;

fn fold(byte: u8) -> u8 {
    if byte.is_ascii_uppercase() {
        byte + 32
    } else {
        byte
    }
}

/// `*.*` and the empty mask mean everything (the guest maps `*.*` to a zero-length mask itself); otherwise `*` any run, `?` one
/// byte, ASCII case-insensitive, whole name.
pub fn mask_matches(mask: &[u8], name: &[u8]) -> bool {
    if mask.is_empty() || mask == b"*.*" {
        return true;
    }
    let (mut mi, mut ni) = (0usize, 0usize);
    let (mut star, mut resume) = (None::<usize>, 0usize);
    while ni < name.len() {
        if mi < mask.len() && mask[mi] == b'*' {
            star = Some(mi);
            mi += 1;
            resume = ni;
        } else if mi < mask.len() && (mask[mi] == b'?' || fold(mask[mi]) == fold(name[ni])) {
            mi += 1;
            ni += 1;
        } else if let Some(s) = star {
            mi = s + 1;
            resume += 1;
            ni = resume;
        } else {
            return false;
        }
    }
    while mi < mask.len() && mask[mi] == b'*' {
        mi += 1;
    }
    mi == mask.len()
}

/// Cursor order: ASCII case-folded, byte order breaking ties.
pub fn name_order(a: &[u8], b: &[u8]) -> std::cmp::Ordering {
    let folded = a.iter().map(|c| fold(*c)).cmp(b.iter().map(|c| fold(*c)));
    folded.then_with(|| a.cmp(b))
}

#[derive(Debug, PartialEq, Eq)]
pub enum Pick {
    /// Index of the chosen candidate.
    Found(usize),
    /// Nothing matches after the cursor.
    None,
    /// The chosen name is longer than the caller's buffer; the cursor must not advance.
    TooLong,
}

/// Streaming selection keeps one bounded best name, independently of the number
/// of directory entries. The caller retains the corresponding metadata.
pub struct Selection<'a> {
    mask: &'a [u8],
    last: Option<&'a [u8]>,
    best: Option<(usize, Vec<u8>)>,
}
impl<'a> Selection<'a> {
    pub fn new(mask: &'a [u8], last: Option<&'a [u8]>) -> Self {
        Self {
            mask,
            last,
            best: None,
        }
    }
    /// True iff the caller must replace its retained candidate with this one.
    pub fn offer(&mut self, index: usize, name: &[u8]) -> bool {
        if name.len() > NAME_MAX
            || !mask_matches(self.mask, name)
            || self.last.is_some_and(|last| name_order(name, last).is_le())
            || self
                .best
                .as_ref()
                .is_some_and(|(_, best)| name_order(name, best).is_ge())
        {
            return false;
        }
        self.best = Some((index, name.to_vec()));
        true
    }
    pub fn finish(self, max_name: usize) -> Pick {
        match self.best {
            None => Pick::None,
            Some((_, name)) if name.len() > max_name => Pick::TooLong,
            Some((index, _)) => Pick::Found(index),
        }
    }
}
/// Choose among names in any order; buffer refusal never falls back to a later,
/// shorter name, because that would skip the first item in cursor order.
pub fn pick<'a>(
    candidates: impl Iterator<Item = &'a [u8]>,
    mask: &[u8],
    last: Option<&[u8]>,
    max_name: usize,
) -> Pick {
    let mut selection = Selection::new(mask, last);
    for (index, name) in candidates.enumerate() {
        selection.offer(index, name);
    }
    selection.finish(max_name)
}

/// NT status values the cursor lifecycle answers with (same values as `kernel_file.h`).
pub mod status {
    pub const SUCCESS: u32 = 0;
    pub const INVALID_PARAMETER: u32 = 0xC000_000D;
    pub const INFO_LENGTH_MISMATCH: u32 = 0xC000_0004;
    pub const NO_MORE_FILES: u32 = 0x8000_0006;
    pub const NO_SUCH_FILE: u32 = 0xC000_000F;
}

/// Per-open cursor flags (T2225). The C open table still stores them; Rust decides every transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cursor {
    pub started: bool,
    pub has_last: bool,
}

/// Rust-owned per-open directory state for the selected kernel backend.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OwnedCursor {
    pub started: bool,
    pub mask: Vec<u8>,
    pub last: Option<Vec<u8>>,
}

impl OwnedCursor {
    pub fn begin(&mut self, host_directory: bool, restart: bool, mask: Option<&[u8]>) -> Begin {
        let mut flags = Cursor {
            started: self.started,
            has_last: self.last.is_some(),
        };
        let result = if host_directory {
            begin_host(&mut flags, restart, mask)
        } else {
            begin(&mut flags, restart, mask)
        };
        self.started = flags.started;
        if result == Begin::Latch {
            self.mask = mask.unwrap_or_default().to_vec();
            self.last = None;
        } else if host_directory && (restart || !flags.started) {
            self.last = None;
        }
        result
    }

    pub fn finish(&mut self, picked: i32, name: Option<&[u8]>) -> u32 {
        if picked == 1 {
            if let Some(name) = name.filter(|name| name.len() <= NAME_MAX) {
                self.last = Some(name.to_vec());
                return status::SUCCESS;
            }
        }
        if picked == -1 {
            return status::INFO_LENGTH_MISMATCH;
        }
        end_status(Cursor {
            started: self.started,
            has_last: self.last.is_some(),
        })
    }
}

/// What [`begin`] did with the caller's mask.
#[derive(Debug, PartialEq, Eq)]
pub enum Begin {
    /// The cursor was already running: the mask of this call is ignored (MEASURED: only the first call carries one).
    Continue,
    /// A fresh listing: the caller must store this mask and forget its last name.
    Latch,
    /// The mask is longer than a name can be; the cursor stays unstarted.
    Refused,
}

/// Start or continue a listing. `restart` (SL_RESTART_SCAN) forgets the cursor; the first call after open or restart latches
/// the mask (absent mask = empty = everything).
pub fn begin(cursor: &mut Cursor, restart: bool, mask: Option<&[u8]>) -> Begin {
    if restart {
        cursor.started = false;
    }
    if cursor.started {
        return Begin::Continue;
    }
    if mask.map_or(0, <[u8]>::len) > NAME_MAX {
        return Begin::Refused;
    }
    cursor.started = true;
    cursor.has_last = false;
    Begin::Latch
}

/// Host-directory C clears the previous result before checking a fresh mask;
/// FATX checks the mask first. Preserve that difference on refusal.
pub fn begin_host(cursor: &mut Cursor, restart: bool, mask: Option<&[u8]>) -> Begin {
    if restart || !cursor.started {
        cursor.has_last = false;
    }
    begin(cursor, restart, mask)
}

/// Status when nothing is left after the cursor: NO_SUCH_FILE when the listing never returned a name, NO_MORE_FILES once it
/// ran out (INFERRED from desktop NT; the measured guest site treats both as the end).
pub fn end_status(cursor: Cursor) -> u32 {
    if cursor.has_last {
        status::NO_MORE_FILES
    } else {
        status::NO_SUCH_FILE
    }
}

/// Kind of a host directory item as `fstatat(AT_SYMLINK_NOFOLLOW)` saw it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKind {
    Regular,
    Directory,
    /// Symlink, device node, FIFO, socket: hidden, matching the open path refusing it.
    Other,
}

/// Whether a host directory item is offered to the cursor at all: no dot entries (FATX has none, INFERRED, and the guest
/// would recurse into them), no name longer than [`NAME_MAX`], only regular files and directories.
pub fn admit_host_entry(name: &[u8], kind: HostKind) -> bool {
    name != b"." && name != b".." && name.len() <= NAME_MAX && kind != HostKind::Other
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    #[test]
    fn owned_cursor_latches_mask_advances_and_restarts() {
        let mut cursor = OwnedCursor::default();
        assert_eq!(cursor.begin(true, false, Some(b"*.xbe")), Begin::Latch);
        assert_eq!(cursor.mask, b"*.xbe");
        assert_eq!(cursor.finish(1, Some(b"alpha.xbe")), status::SUCCESS);
        assert_eq!(cursor.last.as_deref(), Some(b"alpha.xbe".as_slice()));
        assert_eq!(cursor.begin(true, false, Some(b"ignored")), Begin::Continue);
        assert_eq!(cursor.mask, b"*.xbe");
        assert_eq!(
            cursor.begin(false, true, Some(&vec![b'x'; NAME_MAX + 1])),
            Begin::Refused
        );
        assert_eq!(cursor.last.as_deref(), Some(b"alpha.xbe".as_slice()));
        assert_eq!(cursor.begin(false, true, Some(b"*.xbe")), Begin::Latch);
        assert_eq!(cursor.last, None);
    }

    #[test]
    fn first_call_latches_and_later_calls_continue() {
        let mut cursor = Cursor::default();
        assert_eq!(begin(&mut cursor, false, Some(b"*.xbe")), Begin::Latch);
        cursor.has_last = true;
        assert_eq!(begin(&mut cursor, false, Some(b"other")), Begin::Continue);
        assert!(cursor.has_last);
        assert_eq!(begin(&mut cursor, true, None), Begin::Latch);
        assert_eq!(
            cursor,
            Cursor {
                started: true,
                has_last: false
            }
        );
    }

    #[test]
    fn overlong_mask_is_refused_and_stays_unstarted() {
        let mut cursor = Cursor {
            started: false,
            has_last: true,
        };
        assert_eq!(
            begin(&mut cursor, false, Some(&[b'a'; NAME_MAX + 1])),
            Begin::Refused
        );
        assert_eq!(
            cursor,
            Cursor {
                started: false,
                has_last: true
            }
        );
        assert_eq!(
            begin(&mut cursor, false, Some(&[b'a'; NAME_MAX])),
            Begin::Latch
        );
    }

    #[test]
    fn refused_restart_preserves_backend_specific_last_flag() {
        let previous = Cursor {
            started: true,
            has_last: true,
        };
        let mut host = previous;
        let mut fatx = previous;
        let mask = [b'x'; NAME_MAX + 1];
        assert_eq!(begin_host(&mut host, true, Some(&mask)), Begin::Refused);
        assert_eq!(begin(&mut fatx, true, Some(&mask)), Begin::Refused);
        assert_eq!(
            host,
            Cursor {
                started: false,
                has_last: false
            }
        );
        assert_eq!(
            fatx,
            Cursor {
                started: false,
                has_last: true
            }
        );
    }

    #[test]
    fn end_status_distinguishes_empty_from_exhausted() {
        assert_eq!(
            end_status(Cursor {
                started: true,
                has_last: false
            }),
            status::NO_SUCH_FILE
        );
        assert_eq!(
            end_status(Cursor {
                started: true,
                has_last: true
            }),
            status::NO_MORE_FILES
        );
    }

    #[test]
    fn host_admission_hides_dots_links_and_long_names() {
        assert!(!admit_host_entry(b".", HostKind::Directory));
        assert!(!admit_host_entry(b"..", HostKind::Directory));
        assert!(admit_host_entry(b"...", HostKind::Regular));
        assert!(!admit_host_entry(b"link", HostKind::Other));
        assert!(admit_host_entry(&[b'n'; NAME_MAX], HostKind::Regular));
        assert!(!admit_host_entry(&[b'n'; NAME_MAX + 1], HostKind::Regular));
    }
}
