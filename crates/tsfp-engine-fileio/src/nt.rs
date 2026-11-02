//! NT-flavoured behaviour layered on [`Vfs`] that the C host keeps partly or not at all (T2056):
//! directory enumeration with masks and restart, file-information classes, share-mode conflicts,
//! and deterministic asynchronous I/O completion. Contract: docs/t-contract-data-assets.md.
//!
//! Evidence class of this module: FABRICATED inputs, structural spec tests (no C counterpart to
//! diff for share modes and async completion, which the C host records but does not enforce).
use crate::vfs::{Access, DirEntry, Disposition, FileInfo, Handle, NtStatus, Vfs, status};
use std::collections::{HashMap, VecDeque};

pub mod nt_status {
    pub const PENDING: u32 = 0x0000_0103;
    pub const NO_MORE_FILES: u32 = 0x8000_0006;
    pub const NO_SUCH_FILE: u32 = 0xC000_000F;
    pub const INFO_LENGTH_MISMATCH: u32 = 0xC000_0004;
    pub const SHARING_VIOLATION: u32 = 0xC000_0043;
    pub const INVALID_INFO_CLASS: u32 = 0xC000_0003;
}
pub const SHARE_READ: u32 = 1;
pub const SHARE_WRITE: u32 = 2;
pub const SHARE_DELETE: u32 = 4;
pub const ATTR_DIRECTORY: u32 = 0x10;
pub const ATTR_NORMAL: u32 = 0x80;
pub const ATTR_READONLY: u32 = 0x01;
const ALLOCATION_UNIT: u64 = 4096;

/// `*` and `?` wildcard match, ASCII case-insensitive, whole name (NtQueryDirectoryFile FileName mask).
pub fn mask_matches(mask: &str, name: &str) -> bool {
    let (m, n): (Vec<u8>, Vec<u8>) = (mask.to_ascii_lowercase().into_bytes(), name.to_ascii_lowercase().into_bytes());
    let (mut mi, mut ni, mut star, mut mark) = (0, 0, None, 0);
    while ni < n.len() {
        if mi < m.len() && (m[mi] == b'?' || m[mi] == n[ni]) { mi += 1; ni += 1; }
        else if mi < m.len() && m[mi] == b'*' { star = Some(mi); mark = ni; mi += 1; }
        else if let Some(s) = star { mi = s + 1; mark += 1; ni = mark; }
        else { return false; }
    }
    while mi < m.len() && m[mi] == b'*' { mi += 1; }
    mi == m.len()
}

/// Query classes with the on-wire layouts of the Xbox kernel (all little endian).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InfoClass { Basic, Standard, Position, NetworkOpen }
impl InfoClass {
    pub fn from_raw(raw: u32) -> Result<Self, NtStatus> {
        match raw { 4 => Ok(Self::Basic), 5 => Ok(Self::Standard), 14 => Ok(Self::Position), 34 => Ok(Self::NetworkOpen), _ => Err(nt_status::INVALID_INFO_CLASS) }
    }
    pub fn length(self) -> usize { match self { Self::Basic => 36, Self::Standard => 24, Self::Position => 8, Self::NetworkOpen => 56 } }
}

struct DirCursor { entries: Vec<DirEntry>, next: usize, mask: Option<String> }
struct ShareEntry { access: Access, share: u32 }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion { pub id: u32, pub status: NtStatus, pub information: usize, pub data: Vec<u8> }
struct Pending { id: u32, handle: Handle, offset: u64, length: usize }

pub struct NtFiles {
    pub vfs: Vfs,
    shares: HashMap<u32, (String, ShareEntry)>,
    cursors: HashMap<u32, DirCursor>,
    queue: VecDeque<Pending>,
    next_id: u32,
    dir_paths: HashMap<u32, String>,
}

impl NtFiles {
    pub fn new(vfs: Vfs) -> Self {
        Self { vfs, shares: HashMap::new(), cursors: HashMap::new(), queue: VecDeque::new(), next_id: 1, dir_paths: HashMap::new() }
    }

    /// Open with share-mode enforcement. A new open conflicts when any open of the same file denies
    /// what it asks for, or when it denies what an open already holds. Directories are not tracked.
    pub fn open(&mut self, path: &str, access: Access, share: u32, disposition: Disposition) -> Result<Handle, NtStatus> {
        let key = self.vfs.canonical(path)?;
        for (k, held) in self.shares.values() {
            if *k != key { continue; }
            let denied = (access.read && held.share & SHARE_READ == 0) || (access.write && held.share & SHARE_WRITE == 0);
            let denies = (held.access.read && share & SHARE_READ == 0) || (held.access.write && share & SHARE_WRITE == 0);
            if denied || denies { return Err(nt_status::SHARING_VIOLATION); }
        }
        let h = self.vfs.open(path, access, disposition)?;
        self.shares.insert(h.0, (key, ShareEntry { access, share }));
        self.dir_paths.insert(h.0, path.to_string());
        Ok(h)
    }
    pub fn close(&mut self, h: Handle) -> Result<(), NtStatus> {
        self.vfs.close(h)?;
        self.shares.remove(&h.0);
        self.cursors.remove(&h.0);
        self.dir_paths.remove(&h.0);
        self.queue.retain(|p| p.handle != h);
        Ok(())
    }

    /// NtQueryDirectoryFile model: one entry per call, `restart` rewinds, `mask` honoured on the first
    /// call after open or restart. Errors: NO_SUCH_FILE (first call, no match), NO_MORE_FILES.
    pub fn query_directory(&mut self, h: Handle, restart: bool, mask: Option<&str>) -> Result<DirEntry, NtStatus> {
        let path = self.dir_paths.get(&h.0).cloned().ok_or(status::INVALID_HANDLE)?;
        if restart || !self.cursors.contains_key(&h.0) {
            let mask = mask.filter(|m| !m.is_empty() && *m != "*").map(str::to_string);
            let entries = self.vfs.list_dir(&path)?;
            self.cursors.insert(h.0, DirCursor { entries, next: 0, mask });
            let c = &self.cursors[&h.0];
            if !c.entries.iter().any(|e| c.mask.as_deref().is_none_or(|m| mask_matches(m, &e.name))) {
                return Err(nt_status::NO_SUCH_FILE);
            }
        }
        let c = self.cursors.get_mut(&h.0).ok_or(status::INVALID_HANDLE)?;
        while c.next < c.entries.len() {
            let e = c.entries[c.next].clone();
            c.next += 1;
            if c.mask.as_deref().is_none_or(|m| mask_matches(m, &e.name)) { return Ok(e); }
        }
        Err(nt_status::NO_MORE_FILES)
    }

    /// NtQueryInformationFile for the four classes the title can ask for. Times are 0 (the disc has
    /// none, the C host reports host times only for host-dir objects, not modelled here).
    pub fn query_information(&mut self, h: Handle, class: InfoClass, out_len: usize) -> Result<Vec<u8>, NtStatus> {
        if out_len < class.length() { return Err(nt_status::INFO_LENGTH_MISMATCH); }
        let size = self.vfs.size(h)?;
        let is_dir = self.dir_paths.get(&h.0).and_then(|p| self.vfs.query(p).ok()).is_some_and(|FileInfo { is_directory, .. }| is_directory);
        let attrs = if is_dir { ATTR_DIRECTORY } else if self.shares.get(&h.0).is_some_and(|(_, s)| s.access.write) { ATTR_NORMAL } else { ATTR_READONLY | ATTR_NORMAL };
        let alloc = size.div_ceil(ALLOCATION_UNIT) * ALLOCATION_UNIT;
        let mut b = Vec::with_capacity(class.length());
        match class {
            InfoClass::Basic => { for _ in 0..4 { b.extend_from_slice(&0u64.to_le_bytes()); } b.extend_from_slice(&attrs.to_le_bytes()); }
            InfoClass::Standard => {
                b.extend_from_slice(&alloc.to_le_bytes()); b.extend_from_slice(&size.to_le_bytes());
                b.extend_from_slice(&1u32.to_le_bytes()); b.extend_from_slice(&[0, u8::from(is_dir), 0, 0]);
            }
            InfoClass::Position => b.extend_from_slice(&self.vfs.position(h)?.to_le_bytes()),
            InfoClass::NetworkOpen => {
                for _ in 0..4 { b.extend_from_slice(&0u64.to_le_bytes()); }
                b.extend_from_slice(&alloc.to_le_bytes()); b.extend_from_slice(&size.to_le_bytes());
                b.extend_from_slice(&attrs.to_le_bytes()); b.extend_from_slice(&[0; 4]);
            }
        }
        b.truncate(class.length());
        Ok(b)
    }

    /// Queue an asynchronous read. The call returns PENDING with an id; [`Self::complete_next`] completes
    /// requests strictly in submission order (deterministic, so the event the title waits on is signalled
    /// at a reproducible point). A close cancels the handle's pending requests.
    pub fn submit_read(&mut self, h: Handle, offset: u64, length: usize) -> Result<u32, NtStatus> {
        self.vfs.size(h)?;
        let id = self.next_id;
        self.next_id += 1;
        self.queue.push_back(Pending { id, handle: h, offset, length });
        Ok(id)
    }
    pub fn pending_count(&self) -> usize { self.queue.len() }
    /// Perform the oldest pending request. A zero-byte read of a positive length is END_OF_FILE.
    pub fn complete_next(&mut self) -> Option<Completion> {
        let p = self.queue.pop_front()?;
        let mut data = vec![0u8; p.length];
        Some(match self.vfs.read_at(p.handle, p.offset, &mut data) {
            Ok(0) if p.length > 0 => Completion { id: p.id, status: status::END_OF_FILE, information: 0, data: Vec::new() },
            Ok(n) => { data.truncate(n); Completion { id: p.id, status: status::SUCCESS, information: n, data } }
            Err(s) => Completion { id: p.id, status: s, information: 0, data: Vec::new() },
        })
    }
}
