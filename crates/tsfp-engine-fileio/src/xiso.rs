//! XDVDFS (Xbox DVD filesystem) reader over a host file. Contract: docs/t-contract-data-assets.md.
//!
//! Format (MEASURED on the owner's disc, mirrored from the C host `src/xbox/xdvdfs.c`):
//! 2048-byte sectors, volume descriptor at sector 32 with the 20-byte magic at +0x00 and
//! +0x7EC, root sector/size (bytes) at +0x14/+0x18. A directory is a binary tree of entries
//! inside one byte blob. Link value 0 and 0xFFFF both mean "no child". Nothing is trusted:
//! extents are checked against the image size, walks are iterative and visit-bounded.
use std::collections::HashMap;
use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::Path;
use std::os::fd::AsRawFd;
use std::sync::{Arc, Mutex};

pub const SECTOR: u64 = 2048;
pub const DESCRIPTOR_SECTOR: u64 = 32;
pub const MAGIC: &[u8; 20] = b"MICROSOFT*XBOX*MEDIA";
pub const ATTR_DIRECTORY: u8 = 0x10;
pub const DIR_BYTES_MAX: u32 = 262_144;
const FIXED: usize = 14;
const MAX_DEPTH: usize = 32;

/// Why an operation failed. Mirrors the C `xdvdfs_result` values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XisoError {
    Image(String),
    Magic,
    Truncated,
    Malformed(&'static str),
    WalkLimit,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: Vec<u8>,
    pub start_sector: u32,
    pub size: u32,
    pub attributes: u8,
}
impl Entry {
    pub fn is_directory(&self) -> bool { self.attributes & ATTR_DIRECTORY != 0 }
    pub fn byte_offset(&self) -> u64 { u64::from(self.start_sector) * SECTOR }
}

/// T2143: one parsed directory with a case-insensitive name index. Before this cache every `lookup` re-read each directory of the
/// path from the image (up to 256 KiB) and re-parsed it with a hash set and one allocation per entry, which made every file open
/// of a large directory cost milliseconds and slowed the loading of `--rust-fileio` runs.
struct DirCache {
    entries: Vec<Entry>,
    /// ASCII-lowercased name to the index of the FIRST entry of that name in traversal order (what `find` returned).
    index: HashMap<Vec<u8>, usize>,
}

pub struct Xiso {
    file: File,
    image_bytes: u64,
    root: Entry,
    dirs: Mutex<HashMap<(u32, u32), Arc<DirCache>>>,
}

fn read_exact_at(file: &File, buf: &mut [u8], offset: u64) -> Result<(), XisoError> {
    file.read_exact_at(buf, offset).map_err(|_| XisoError::Truncated)
}

impl Xiso {
    pub fn open(path: &Path) -> Result<Self, XisoError> {
        let file = File::open(path).map_err(|e| XisoError::Image(e.to_string()))?;
        let image_bytes = file.metadata().map_err(|e| XisoError::Image(e.to_string()))?.len();
        let mut d = [0u8; SECTOR as usize];
        read_exact_at(&file, &mut d, DESCRIPTOR_SECTOR * SECTOR).map_err(|_| XisoError::Magic)?;
        if &d[..20] != MAGIC || &d[0x7EC..0x7EC + 20] != MAGIC { return Err(XisoError::Magic); }
        let sector = u32::from_le_bytes(d[0x14..0x18].try_into().unwrap());
        let size = u32::from_le_bytes(d[0x18..0x1C].try_into().unwrap());
        let root = Entry { name: Vec::new(), start_sector: sector, size, attributes: ATTR_DIRECTORY };
        let this = Self { file, image_bytes, root, dirs: Mutex::new(HashMap::new()) };
        this.check_extent(&this.root)?;
        Ok(this)
    }
    pub fn image_bytes(&self) -> u64 { self.image_bytes }
    /// The descriptor of the image file (T2143: the C ABI layer gives the kernel readahead advice for it).
    pub fn raw_fd(&self) -> i32 { self.file.as_raw_fd() }
    /// Image byte offset of `offset` inside `entry` (T2143: readahead).
    pub fn image_offset(entry: &Entry, offset: u64) -> u64 { entry.byte_offset() + offset }
    pub fn root(&self) -> &Entry { &self.root }

    fn check_extent(&self, e: &Entry) -> Result<(), XisoError> {
        if e.byte_offset() + u64::from(e.size) > self.image_bytes { return Err(XisoError::Malformed("extent past image")); }
        Ok(())
    }

    fn cached_dir(&self, dir: &Entry) -> Result<Arc<DirCache>, XisoError> {
        let key = (dir.start_sector, dir.size);
        if let Some(found) = self.dirs.lock().unwrap().get(&key) { return Ok(Arc::clone(found)); }
        let entries = self.parse_dir(dir)?;
        let mut index = HashMap::with_capacity(entries.len());
        for (position, entry) in entries.iter().enumerate() {
            index.entry(entry.name.to_ascii_lowercase()).or_insert(position);
        }
        let built = Arc::new(DirCache { entries, index });
        self.dirs.lock().unwrap().insert(key, Arc::clone(&built));
        Ok(built)
    }

    /// The entry called `name` (ASCII case-insensitive) directly inside `dir`, from the directory cache (T2143).
    pub fn find_child(&self, dir: &Entry, name: &[u8]) -> Result<Option<Entry>, XisoError> {
        let directory = self.cached_dir(dir)?;
        Ok(directory.index.get(&name.to_ascii_lowercase()).map(|&position| directory.entries[position].clone()))
    }

    /// Every entry of one directory (unordered).
    pub fn read_dir(&self, dir: &Entry) -> Result<Vec<Entry>, XisoError> {
        if !dir.is_directory() { return Err(XisoError::Malformed("not a directory")); }
        Ok(self.cached_dir(dir)?.entries.clone())
    }

    fn parse_dir(&self, dir: &Entry) -> Result<Vec<Entry>, XisoError> {
        if !dir.is_directory() { return Err(XisoError::Malformed("not a directory")); }
        if dir.size == 0 { return Ok(Vec::new()); }
        if dir.size > DIR_BYTES_MAX { return Err(XisoError::Malformed("directory blob too large")); }
        self.check_extent(dir)?;
        let mut blob = vec![0u8; dir.size as usize];
        read_exact_at(&self.file, &mut blob, dir.byte_offset())?;
        let (mut pending, mut seen, mut out) = (vec![0usize], std::collections::HashSet::new(), Vec::new());
        while let Some(off) = pending.pop() {
            if !seen.insert(off) { continue; }
            if seen.len() > blob.len() / 4 + 1 { return Err(XisoError::WalkLimit); }
            if off + FIXED > blob.len() { return Err(XisoError::Malformed("entry header past blob")); }
            let b = &blob[off..];
            let (left, right) = (u16::from_le_bytes([b[0], b[1]]), u16::from_le_bytes([b[2], b[3]]));
            let start_sector = u32::from_le_bytes(b[4..8].try_into().unwrap());
            let size = u32::from_le_bytes(b[8..12].try_into().unwrap());
            let (attributes, name_len) = (b[12], b[13] as usize);
            if off + FIXED + name_len > blob.len() { return Err(XisoError::Malformed("name past blob")); }
            let entry = Entry { name: b[FIXED..FIXED + name_len].to_vec(), start_sector, size, attributes };
            self.check_extent(&entry)?;
            out.push(entry);
            for link in [left, right] {
                if link != 0 && link != 0xFFFF { pending.push(link as usize * 4); }
            }
        }
        Ok(out)
    }

    /// Resolve a disc-relative path (either separator, case-insensitive ASCII, no `.`/`..`).
    pub fn lookup(&self, path: &str) -> Result<Entry, XisoError> {
        let mut cur = self.root.clone();
        for part in path.split(['/', '\\']).filter(|p| !p.is_empty()) {
            if part == "." || part == ".." { return Err(XisoError::NotFound); }
            if !cur.is_directory() { return Err(XisoError::NotFound); }
            let directory = self.cached_dir(&cur)?;
            let position = directory.index.get(&part.as_bytes().to_ascii_lowercase()).ok_or(XisoError::NotFound)?;
            cur = directory.entries[*position].clone();
        }
        Ok(cur)
    }

    /// Read at most `buf.len()` bytes from `offset` in the entry. Clamped at EOF; EOF is `Ok(0)`.
    pub fn read(&self, e: &Entry, offset: u64, buf: &mut [u8]) -> Result<usize, XisoError> {
        if offset >= u64::from(e.size) { return Ok(0); }
        let n = buf.len().min((u64::from(e.size) - offset) as usize);
        read_exact_at(&self.file, &mut buf[..n], e.byte_offset() + offset)?;
        Ok(n)
    }

    /// Depth-first listing `(path with '/', entry)` of the whole disc, directories included.
    pub fn walk(&self) -> Result<Vec<(String, Entry)>, XisoError> {
        let mut out = Vec::new();
        self.walk_into(&self.root, "", 0, &mut out)?;
        Ok(out)
    }
    fn walk_into(&self, dir: &Entry, prefix: &str, depth: usize, out: &mut Vec<(String, Entry)>) -> Result<(), XisoError> {
        if depth > MAX_DEPTH { return Err(XisoError::WalkLimit); }
        for e in self.read_dir(dir)? {
            let path = format!("{prefix}{}", e.name.iter().map(|&c| c as char).collect::<String>());
            out.push((path.clone(), e.clone()));
            if e.is_directory() { self.walk_into(&e, &format!("{path}/"), depth + 1, out)?; }
        }
        Ok(())
    }
}
