//! Guest object namespace: the mount table and the symbolic-link table (T2104, rule 16 S1).
//! Ports `resolve_symlinks_locked` and `volume_for_locked` of src/xbox/kernel_file.c. The tables are indexed by the C slot
//! numbers so scan order, and with it every tie break, matches the C loops.

/// Same bound as KERNEL_FILE_SYMLINK_DEPTH_MAX.
pub const LINK_DEPTH_MAX: usize = 8;

fn fold(byte: u8) -> u8 {
    match byte.to_ascii_lowercase() {
        b'\\' => b'/',
        other => other,
    }
}

fn link_name_eq(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

/// Length of `prefix` when `text` starts with it (ASCII case-insensitive, `\` equals `/`), else 0.
pub fn prefix_match_length(text: &[u8], prefix: &[u8]) -> usize {
    if prefix.len() > text.len() || !prefix.iter().zip(text).all(|(p, t)| fold(*p) == fold(*t)) {
        return 0;
    }
    prefix.len()
}

fn ends_component(text: &[u8], at: usize) -> bool {
    matches!(text.get(at), None | Some(b'\\') | Some(b'/'))
}

#[derive(Default, Clone)]
pub struct Namespace {
    mounts: Vec<Option<Vec<u8>>>,
    links: Vec<Option<(Vec<u8>, Vec<u8>)>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Resolve {
    Settled(Vec<u8>),
    /// The rewritten name would not fit `capacity` (including the terminator).
    TooLong,
    /// LINK_DEPTH_MAX rewrites without settling (cyclic link table).
    Cyclic,
}

/// Result of inserting a guest symbolic link into the Rust-owned namespace.
#[derive(Debug, PartialEq, Eq)]
pub enum LinkInsertError {
    EmptyName,
    EmptyTarget,
    Duplicate,
    Full,
}

impl Namespace {
    pub const fn new() -> Self {
        Self {
            mounts: Vec::new(),
            links: Vec::new(),
        }
    }

    pub fn set_mount(&mut self, slot: usize, prefix: Option<&[u8]>) {
        if self.mounts.len() <= slot {
            self.mounts.resize(slot + 1, None);
        }
        self.mounts[slot] = prefix.map(<[u8]>::to_vec);
    }

    pub fn set_link(&mut self, slot: usize, link: Option<(&[u8], &[u8])>) {
        if self.links.len() <= slot {
            self.links.resize(slot + 1, None);
        }
        self.links[slot] = link.map(|(name, target)| (name.to_vec(), target.to_vec()));
    }

    /// Insert with the kernel table's lowest-free-slot policy. The C table compares
    /// stored names case-insensitively but does not fold slash characters.
    pub fn insert_link(
        &mut self,
        name: &[u8],
        target: &[u8],
        capacity: usize,
    ) -> Result<usize, LinkInsertError> {
        if name.is_empty() {
            return Err(LinkInsertError::EmptyName);
        }
        if target.is_empty() {
            return Err(LinkInsertError::EmptyTarget);
        }
        let mut first_free = None;
        for (slot, link) in self.links.iter().enumerate().take(capacity) {
            match link {
                Some((existing, _)) if link_name_eq(existing, name) => {
                    return Err(LinkInsertError::Duplicate);
                }
                None if first_free.is_none() => first_free = Some(slot),
                _ => {}
            }
        }
        let slot = first_free
            .or_else(|| (self.links.len() < capacity).then_some(self.links.len()))
            .ok_or(LinkInsertError::Full)?;
        self.set_link(slot, Some((name, target)));
        Ok(slot)
    }

    /// Replace a matching link, or insert into the first free kernel-table slot.
    pub fn replace_link(&mut self, name: &[u8], target: &[u8], capacity: usize) -> Option<usize> {
        if name.is_empty() || target.is_empty() || name.len() >= 260 || target.len() >= 260 {
            return None;
        }
        let mut first_free = None;
        for (slot, link) in self.links.iter().enumerate().take(capacity) {
            match link {
                Some((existing, _)) if link_name_eq(existing, name) => {
                    self.links[slot] = Some((name.to_vec(), target.to_vec()));
                    return Some(slot);
                }
                None if first_free.is_none() => first_free = Some(slot),
                _ => {}
            }
        }
        let slot = first_free.unwrap_or(self.links.len());
        if slot >= capacity {
            return None;
        }
        if self.links.len() <= slot {
            self.links.resize(slot + 1, None);
        }
        self.links[slot] = Some((name.to_vec(), target.to_vec()));
        Some(slot)
    }

    /// Remove the first link matching the C table's ASCII case-insensitive identity.
    pub fn remove_link(&mut self, name: &[u8]) -> Option<usize> {
        let slot = self.links.iter().position(|link| {
            link.as_ref()
                .is_some_and(|(existing, _)| link_name_eq(existing, name))
        })?;
        self.links[slot] = None;
        Some(slot)
    }

    /// Lowest free mount slot, with the existing optional byte-exact duplicate check.
    /// Resource handles remain owned by the host; this selects the logical namespace slot.
    pub fn mount_slot(&self, unique: Option<&[u8]>) -> Option<usize> {
        let mut first_free = None;
        for (slot, prefix) in self.mounts.iter().enumerate() {
            match prefix {
                Some(prefix) if unique == Some(prefix.as_slice()) => return None,
                None if first_free.is_none() => first_free = Some(slot),
                _ => (),
            }
        }
        first_free
    }

    pub fn clear(&mut self) {
        self.mounts.clear();
        self.links.clear();
    }

    /// Rewrite leading whole-component links until none match. `capacity` is the C buffer size.
    pub fn resolve(&self, path: &[u8], capacity: usize) -> Resolve {
        if path.len() >= capacity {
            return Resolve::TooLong;
        }
        let mut out = path.to_vec();
        for _ in 0..LINK_DEPTH_MAX {
            let hit = self.links.iter().flatten().find_map(|(name, target)| {
                let matched = prefix_match_length(&out, name);
                (matched != 0 && ends_component(&out, matched)).then_some((matched, target))
            });
            let Some((matched, target)) = hit else {
                return Resolve::Settled(out);
            };
            if target.len() + out.len() - matched >= capacity {
                return Resolve::TooLong;
            }
            let mut rebuilt = target.clone();
            rebuilt.extend_from_slice(&out[matched..]);
            out = rebuilt;
        }
        Resolve::Cyclic
    }

    /// Longest whole-component mount prefix: (slot, length of the prefix in `path`). A later slot wins a tie.
    pub fn volume_for(&self, path: &[u8]) -> Option<(usize, usize)> {
        let mut best = None;
        for (slot, prefix) in self.mounts.iter().enumerate() {
            let Some(prefix) = prefix else { continue };
            let matched = prefix_match_length(path, prefix);
            if matched == 0
                || best.is_some_and(|(_, length)| matched < length)
                || !ends_component(path, matched)
            {
                continue;
            }
            best = Some((slot, matched));
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Namespace {
        let mut ns = Namespace::default();
        ns.set_link(0, Some((b"\\??\\D:", b"\\Device\\CdRom0")));
        ns.set_link(
            1,
            Some((b"\\??\\T:", b"\\Device\\Harddisk0\\Partition1\\TDATA")),
        );
        ns.set_mount(0, Some(b"\\Device\\Cdrom0"));
        ns.set_mount(1, Some(b"\\Device\\Harddisk0\\Partition1"));
        ns.set_mount(2, Some(b"\\Device\\Harddisk0\\Partition1\\TDATA"));
        ns
    }

    #[test]
    fn link_then_mount() {
        let ns = sample();
        let Resolve::Settled(out) = ns.resolve(b"\\??\\d:\\pak/chr.pak", 261) else {
            panic!()
        };
        assert_eq!(out, b"\\Device\\CdRom0\\pak/chr.pak");
        assert_eq!(ns.volume_for(&out), Some((0, 14)));
        let Resolve::Settled(t) = ns.resolve(b"\\??\\T:\\x", 261) else {
            panic!()
        };
        assert_eq!(ns.volume_for(&t), Some((2, 34)));
    }

    #[test]
    fn whole_component_only() {
        let ns = sample();
        assert_eq!(
            ns.resolve(b"\\??\\D:X", 261),
            Resolve::Settled(b"\\??\\D:X".to_vec())
        );
        assert_eq!(ns.volume_for(b"\\Device\\Cdrom01"), None);
    }

    #[test]
    fn mount_slot_uses_first_hole_and_byte_exact_duplicate_policy() {
        let mut ns = Namespace::default();
        ns.set_mount(0, Some(b"\\Device\\Cdrom0"));
        ns.set_mount(1, None);
        ns.set_mount(2, Some(b"\\Device\\Fatx"));
        ns.set_mount(3, None);
        assert_eq!(ns.mount_slot(None), Some(1));
        assert_eq!(ns.mount_slot(Some(b"\\Device\\Fatx")), None);
        assert_eq!(ns.mount_slot(Some(b"\\Device\\FATX")), Some(1));
        ns.set_mount(1, Some(b"\\Device\\Other"));
        assert_eq!(ns.mount_slot(None), Some(3));
    }

    #[test]
    fn link_insert_uses_first_hole_and_guest_path_identity() {
        let mut ns = Namespace::default();
        ns.set_link(0, Some((b"\\??\\D:", b"\\Device\\CdRom0")));
        ns.set_link(1, None);
        ns.set_link(2, Some((b"X:", b"\\Device\\Other")));
        assert_eq!(ns.insert_link(b"T:", b"\\Device\\Fatx", 4), Ok(1));
        assert_eq!(
            ns.insert_link(b"x:", b"\\Device\\Again", 4),
            Err(LinkInsertError::Duplicate)
        );
        assert_eq!(
            ns.insert_link(b"Z:", b"\\Device\\Full", 3),
            Err(LinkInsertError::Full)
        );
        assert_eq!(
            ns.insert_link(b"", b"target", 4),
            Err(LinkInsertError::EmptyName)
        );
    }

    #[test]
    fn link_remove_uses_guest_path_identity_and_preserves_other_slots() {
        let mut ns = Namespace::new();
        ns.set_link(0, Some((b"\\??\\D:", b"\\Device\\CdRom0")));
        ns.set_link(1, Some((b"X:", b"\\Device\\Other")));
        assert_eq!(ns.remove_link(b"/??/d:"), None);
        assert_eq!(ns.remove_link(b"\\??\\d:"), Some(0));
        assert_eq!(ns.remove_link(b"missing:"), None);
        assert_eq!(
            ns.resolve(b"X:\\file", 32),
            Resolve::Settled(b"\\Device\\Other\\file".to_vec())
        );
    }

    #[test]
    fn generic_link_replace_preserves_existing_slot_and_uses_first_hole() {
        let mut ns = Namespace::new();
        ns.set_link(0, Some((b"X:", b"old")));
        ns.set_link(2, Some((b"Y:", b"other")));
        assert_eq!(ns.replace_link(b"x:", b"new", 3), Some(0));
        assert_eq!(ns.replace_link(b"Z:", b"third", 3), Some(1));
        assert_eq!(
            ns.resolve(b"X:\\file", 32),
            Resolve::Settled(b"new\\file".to_vec())
        );
        assert_eq!(ns.replace_link(b"W:", b"full", 3), None);
    }

    #[test]
    fn cycle_and_length() {
        let mut ns = Namespace::default();
        ns.set_link(3, Some((b"\\a", b"\\b")));
        ns.set_link(4, Some((b"\\b", b"\\a")));
        assert_eq!(ns.resolve(b"\\a\\f", 261), Resolve::Cyclic);
        ns.clear();
        ns.set_link(0, Some((b"\\a", b"\\aaaaaaaa")));
        assert_eq!(ns.resolve(b"\\a\\0123", 12), Resolve::TooLong);
        assert_eq!(ns.resolve(&[b'x'; 12], 12), Resolve::TooLong);
    }
}
