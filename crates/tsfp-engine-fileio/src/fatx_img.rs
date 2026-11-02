//! Read/write FATX volume over a flat byte image (T2074): the Rust twin of `src/input/mu_fatx.c`.
//!
//! Same format, same quirks, same status codes (memory-unit and HDD save images). Layout INFERRED from
//! the public FATX description exactly as in the C implementation: 4096-byte superblock, one FAT,
//! 16 KiB clusters, FAT16 below 65525 clusters, 64-byte entries, bounded single-cluster directories
//! (256 entries). Verified against the C implementation with a differential test (`tests/fatx_diff.rs`
//! compares images byte for byte after identical operation sequences).

pub const SECTOR_BYTES: usize = 512;
pub const SUPERBLOCK_BYTES: usize = 4096;
pub const CLUSTER_SECTORS: u32 = 32;
pub const CLUSTER_BYTES: usize = CLUSTER_SECTORS as usize * SECTOR_BYTES;
pub const NAME_MAX: usize = 42;
pub const ENTRY_BYTES: usize = 64;
pub const MAGIC: u32 = 0x5854_4146;
pub const MIN_IMAGE: usize = SUPERBLOCK_BYTES * 2 + CLUSTER_BYTES * 2;
const DIR_END: u8 = 0xFF;
const DIR_DELETED: u8 = 0xE5;
const ENTRIES_PER_DIR: usize = CLUSTER_BYTES / ENTRY_BYTES;

/// Status codes, numerically equal to the C `fatx_status`.
pub const OK: u32 = 0;
pub const E_ARGUMENT: u32 = 1;
pub const E_BAD_MAGIC: u32 = 2;
pub const E_GEOMETRY: u32 = 3;
pub const E_NOT_FOUND: u32 = 4;
pub const E_NO_SPACE: u32 = 5;
pub const E_DIR_FULL: u32 = 6;
pub const E_NAME: u32 = 7;
pub const E_CORRUPT: u32 = 8;
pub const E_NOT_DIR: u32 = 9;
pub const E_IS_DIR: u32 = 10;
pub const E_EXISTS: u32 = 11;
pub const E_PATH_NOT_FOUND: u32 = 12;

/// Volume geometry (the C `fatx_volume` without the image pointer). `root_cluster` is the directory in use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geo {
    pub size: usize,
    pub volume_id: u32,
    pub root_cluster: u32,
    pub cluster_count: u32,
    pub fat_offset: usize,
    pub fat_size: usize,
    pub data_offset: usize,
    pub fat32: bool,
}

/// Decoded directory slot (the C `fatx_directory_entry`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DirEntry {
    pub name: Vec<u8>,
    pub directory: bool,
    pub attributes: u8,
    pub size: u32,
    pub allocation: u64,
    pub creation_time: u64,
    pub access_time: u64,
    pub write_time: u64,
}

fn rd32(p: &[u8]) -> u32 { u32::from_le_bytes([p[0], p[1], p[2], p[3]]) }
fn wr32(p: &mut [u8], v: u32) { p[..4].copy_from_slice(&v.to_le_bytes()); }

fn geometry(size: usize) -> Option<(u32, usize, bool)> {
    if size < MIN_IMAGE || size as u64 > 0xFFFF_FFF0u64 * CLUSTER_BYTES as u64 { return None; }
    let count = ((size - SUPERBLOCK_BYTES) / CLUSTER_BYTES) as u32;
    let wide = count >= 65525;
    let raw = (count as usize + 1) * if wide { 4 } else { 2 };
    let fat = raw.div_ceil(SUPERBLOCK_BYTES) * SUPERBLOCK_BYTES;
    let count = ((size.checked_sub(SUPERBLOCK_BYTES + fat)?) / CLUSTER_BYTES) as u32;
    if count < 2 { return None; }
    Some((count, fat, wide))
}

impl Geo {
    fn eoc(&self) -> u32 { if self.fat32 { 0xFFFF_FFFF } else { 0xFFFF } }
    fn is_eoc(&self, v: u32) -> bool { if self.fat32 { v >= 0xFFFF_FFF8 } else { v >= 0xFFF8 } }
    fn fat_pos(&self, cluster: u32) -> usize { self.fat_offset + cluster as usize * if self.fat32 { 4 } else { 2 } }
    fn fat_get(&self, img: &[u8], cluster: u32) -> u32 {
        let p = self.fat_pos(cluster);
        if self.fat32 { rd32(&img[p..]) } else { u32::from(img[p]) | u32::from(img[p + 1]) << 8 }
    }
    fn fat_set(&self, img: &mut [u8], cluster: u32, value: u32) {
        let p = self.fat_pos(cluster);
        if self.fat32 { wr32(&mut img[p..], value) } else { img[p] = value as u8; img[p + 1] = (value >> 8) as u8; }
    }
    fn cluster_pos(&self, cluster: u32) -> usize { self.data_offset + (cluster as usize - 1) * CLUSTER_BYTES }
}

/// Lay down an empty volume over the whole image (erases it).
pub fn format(img: &mut [u8], volume_id: u32) -> u32 {
    let Some((clusters, fat, wide)) = geometry(img.len()) else { return E_GEOMETRY };
    img.fill(0);
    wr32(img, MAGIC);
    wr32(&mut img[4..], volume_id);
    wr32(&mut img[8..], CLUSTER_SECTORS);
    wr32(&mut img[12..], 1);
    img[16] = 1;
    let g = Geo { size: img.len(), volume_id, root_cluster: 1, cluster_count: clusters, fat_offset: SUPERBLOCK_BYTES, fat_size: fat, data_offset: SUPERBLOCK_BYTES + fat, fat32: wide };
    g.fat_set(img, 0, if wide { 0xFFFF_FFF8 } else { 0xFFF8 });
    g.fat_set(img, 1, g.eoc());
    let p = g.cluster_pos(1);
    img[p..p + CLUSTER_BYTES].fill(DIR_END);
    OK
}

/// Validate the superblock and geometry; never writes.
pub fn open(img: &[u8]) -> Result<Geo, u32> {
    let Some((clusters, fat, wide)) = geometry(img.len()) else { return Err(E_GEOMETRY) };
    if rd32(img) != MAGIC { return Err(E_BAD_MAGIC); }
    if rd32(&img[8..]) != CLUSTER_SECTORS || rd32(&img[12..]) != 1 { return Err(E_GEOMETRY); }
    let g = Geo { size: img.len(), volume_id: rd32(&img[4..]), root_cluster: 1, cluster_count: clusters, fat_offset: SUPERBLOCK_BYTES, fat_size: fat, data_offset: SUPERBLOCK_BYTES + fat, fat32: wide };
    if !g.is_eoc(g.fat_get(img, 1)) { return Err(E_CORRUPT); }
    Ok(g)
}

fn name_ok(name: &[u8]) -> bool {
    !name.is_empty() && name.len() <= NAME_MAX && name.iter().all(|c| c.is_ascii_alphanumeric() || b"$%'-_@~`!(){}^#&.".contains(c))
}

fn entry_matches(e: &[u8], name: &[u8]) -> bool {
    usize::from(e[0]) == name.len() && e[2..2 + name.len()].eq_ignore_ascii_case(name)
}

/// Offset of the matching entry, and the first free slot (deleted or end marker) seen before the end.
fn find_entry(g: &Geo, img: &[u8], name: &[u8]) -> (Option<usize>, Option<usize>) {
    let dir = g.cluster_pos(g.root_cluster);
    let mut free = None;
    for i in 0..ENTRIES_PER_DIR {
        let e = dir + i * ENTRY_BYTES;
        match img[e] {
            DIR_END => { return (None, free.or(Some(e))); }
            DIR_DELETED => { free.get_or_insert(e); }
            _ => if entry_matches(&img[e..], name) { return (Some(e), free); }
        }
    }
    (None, free)
}

fn free_chain(g: &Geo, img: &mut [u8], mut cluster: u32) {
    let mut guard = 0;
    while cluster >= 2 && cluster <= g.cluster_count && guard <= g.cluster_count {
        let next = g.fat_get(img, cluster);
        g.fat_set(img, cluster, 0);
        if g.is_eoc(next) || next == 0 { break; }
        cluster = next;
        guard += 1;
    }
}

pub fn free_clusters(g: &Geo, img: &[u8]) -> u32 { (2..=g.cluster_count).filter(|&c| g.fat_get(img, c) == 0).count() as u32 }

pub fn file_count(g: &Geo, img: &[u8]) -> u32 {
    let dir = g.cluster_pos(g.root_cluster);
    let mut count = 0;
    for i in 0..ENTRIES_PER_DIR {
        match img[dir + i * ENTRY_BYTES] { DIR_END => break, DIR_DELETED => {} _ => count += 1 }
    }
    count
}

pub fn write_file(g: &Geo, img: &mut [u8], name: &[u8], data: &[u8]) -> u32 {
    if !name_ok(name) { return E_NAME; }
    let length = data.len() as u32;
    let (existing, slot) = find_entry(g, img, name);
    let need = (u64::from(length)).div_ceil(CLUSTER_BYTES as u64) as u32;
    if existing.is_none() && slot.is_none() { return E_DIR_FULL; }
    let mut reusable = 0u32;
    if let Some(e) = existing {
        if img[e + 1] & 0x10 != 0 { return E_IS_DIR; }
        let mut c = rd32(&img[e + 0x2C..]);
        if c != 0 {
            loop {
                if c < 2 || c > g.cluster_count || reusable >= g.cluster_count { return E_CORRUPT; }
                reusable += 1;
                let next = g.fat_get(img, c);
                if g.is_eoc(next) { break; }
                c = next;
            }
        }
    }
    if need > free_clusters(g, img) + reusable { return E_NO_SPACE; }
    let slot = if let Some(e) = existing { free_chain(g, img, rd32(&img[e + 0x2C..])); e } else { slot.unwrap_or(0) };
    let (mut first, mut previous, mut cursor) = (0u32, 0u32, 2u32);
    for i in 0..need {
        while cursor <= g.cluster_count && g.fat_get(img, cursor) != 0 { cursor += 1; }
        if cursor > g.cluster_count { return E_CORRUPT; }
        g.fat_set(img, cursor, g.eoc());
        if previous != 0 { g.fat_set(img, previous, cursor); } else { first = cursor; }
        let offset = i as usize * CLUSTER_BYTES;
        let chunk = (data.len() - offset).min(CLUSTER_BYTES);
        let p = g.cluster_pos(cursor);
        img[p..p + CLUSTER_BYTES].fill(0);
        img[p..p + chunk].copy_from_slice(&data[offset..offset + chunk]);
        previous = cursor;
        cursor += 1;
    }
    let n = name.len();
    img[slot..slot + ENTRY_BYTES].fill(0);
    img[slot] = n as u8;
    img[slot + 1] = 0;
    img[slot + 2..slot + 2 + n].copy_from_slice(name);
    img[slot + 2 + n..slot + 2 + NAME_MAX].fill(0xFF);
    wr32(&mut img[slot + 0x2C..], first);
    wr32(&mut img[slot + 0x30..], length);
    OK
}

/// Copies up to `out.len()` bytes; `size` is the full file size.
pub fn read_file(g: &Geo, img: &[u8], name: &[u8], out: &mut [u8], size_out: &mut u32) -> u32 {
    if !name_ok(name) { return E_NAME; }
    let Some(e) = find_entry(g, img, name).0 else { return E_NOT_FOUND };
    if img[e + 1] & 0x10 != 0 { return E_IS_DIR; }
    let size = rd32(&img[e + 0x30..]) as usize;
    let mut cluster = rd32(&img[e + 0x2C..]);
    let mut copied = 0usize;
    *size_out = size as u32;
    let mut guard = 0;
    while copied < size {
        if cluster < 2 || cluster > g.cluster_count || guard > g.cluster_count { return E_CORRUPT; }
        let chunk = (size - copied).min(CLUSTER_BYTES);
        if copied < out.len() {
            let n = chunk.min(out.len() - copied);
            let p = g.cluster_pos(cluster);
            out[copied..copied + n].copy_from_slice(&img[p..p + n]);
        }
        copied += chunk;
        let next = g.fat_get(img, cluster);
        if copied >= size { break; }
        if g.is_eoc(next) { return E_CORRUPT; }
        cluster = next;
        guard += 1;
    }
    OK
}

pub fn remove_file(g: &Geo, img: &mut [u8], name: &[u8]) -> u32 {
    if !name_ok(name) { return E_NAME; }
    let Some(e) = find_entry(g, img, name).0 else { return E_NOT_FOUND };
    if img[e + 1] & 0x10 != 0 { return E_IS_DIR; }
    free_chain(g, img, rd32(&img[e + 0x2C..]));
    img[e] = DIR_DELETED;
    OK
}

/// Resolve the parent directory of `path`; the returned leaf is empty for the root.
fn path_parent(g: &Geo, img: &[u8], path: &[u8]) -> Result<(Geo, Vec<u8>), u32> {
    if path.len() > 1024 { return Err(E_ARGUMENT); }
    let mut parent = *g;
    let sep = |c: &u8| *c == b'\\' || *c == b'/';
    let mut rest = path;
    while rest.first().is_some_and(sep) { rest = &rest[1..]; }
    if rest.is_empty() { return Ok((parent, Vec::new())); }
    for _ in 0..32 {
        let n = rest.iter().position(sep).unwrap_or(rest.len());
        if n > NAME_MAX { return Err(E_NAME); }
        let leaf = &rest[..n];
        if !name_ok(leaf) || leaf == b"." || leaf == b".." { return Err(E_NAME); }
        rest = &rest[n..];
        while rest.first().is_some_and(sep) { rest = &rest[1..]; }
        if rest.is_empty() { return Ok((parent, leaf.to_vec())); }
        let Some(e) = find_entry(&parent, img, leaf).0 else { return Err(E_PATH_NOT_FOUND) };
        if img[e + 1] & 0x10 == 0 { return Err(E_NOT_DIR); }
        let cluster = rd32(&img[e + 0x2C..]);
        if cluster < 2 || cluster > g.cluster_count || !g.is_eoc(g.fat_get(img, cluster)) { return Err(E_CORRUPT); }
        parent.root_cluster = cluster;
    }
    Err(E_NAME)
}

pub fn stat_path(g: &Geo, img: &[u8], path: &[u8]) -> Result<(bool, u32), u32> {
    let (parent, leaf) = path_parent(g, img, path)?;
    if leaf.is_empty() { return Ok((true, 0)); }
    let Some(e) = find_entry(&parent, img, &leaf).0 else { return Err(E_NOT_FOUND) };
    let dir = img[e + 1] & 0x10 != 0;
    Ok((dir, if dir { 0 } else { rd32(&img[e + 0x30..]) }))
}

pub fn mkdir_path(g: &Geo, img: &mut [u8], path: &[u8]) -> u32 {
    let (parent, leaf) = match path_parent(g, img, path) { Ok(v) => v, Err(e) => return e };
    if leaf.is_empty() { return E_EXISTS; }
    let (found, slot) = find_entry(&parent, img, &leaf);
    if found.is_some() { return E_EXISTS; }
    let Some(slot) = slot else { return E_DIR_FULL };
    let mut cluster = 2u32;
    while cluster <= g.cluster_count && g.fat_get(img, cluster) != 0 { cluster += 1; }
    if cluster > g.cluster_count { return E_NO_SPACE; }
    g.fat_set(img, cluster, g.eoc());
    let p = g.cluster_pos(cluster);
    img[p..p + CLUSTER_BYTES].fill(DIR_END);
    img[slot..slot + ENTRY_BYTES].fill(0);
    img[slot] = leaf.len() as u8;
    img[slot + 1] = 0x10;
    img[slot + 2..slot + 2 + leaf.len()].copy_from_slice(&leaf);
    wr32(&mut img[slot + 0x2C..], cluster);
    OK
}

pub fn write_path(g: &Geo, img: &mut [u8], path: &[u8], data: &[u8]) -> u32 {
    match path_parent(g, img, path) { Ok((parent, leaf)) => write_file(&parent, img, &leaf, data), Err(e) => e }
}

/// Reads up to `out.len()` bytes at `offset`; returns the byte count.
pub fn read_range(g: &Geo, img: &[u8], path: &[u8], offset: u64, out: &mut [u8]) -> Result<u32, u32> {
    let (parent, leaf) = path_parent(g, img, path)?;
    let Some(e) = find_entry(&parent, img, &leaf).0 else { return Err(E_NOT_FOUND) };
    if img[e + 1] & 0x10 != 0 { return Err(E_IS_DIR); }
    let size = u64::from(rd32(&img[e + 0x30..]));
    if offset >= size || out.is_empty() { return Ok(0); }
    let wanted = (out.len() as u64).min(size - offset) as usize;
    let mut cluster = rd32(&img[e + 0x2C..]);
    let (mut done, mut position, mut guard) = (0usize, 0u64, 0u32);
    while done < wanted {
        if cluster < 2 || cluster > g.cluster_count || guard > g.cluster_count { return Err(E_CORRUPT); }
        if offset < position + CLUSTER_BYTES as u64 {
            let start = if offset > position { (offset - position) as usize } else { 0 };
            let part = (wanted - done).min(CLUSTER_BYTES - start);
            let p = g.cluster_pos(cluster) + start;
            out[done..done + part].copy_from_slice(&img[p..p + part]);
            done += part;
        }
        position += CLUSTER_BYTES as u64;
        if done < wanted {
            cluster = g.fat_get(img, cluster);
            if g.is_eoc(cluster) { return Err(E_CORRUPT); }
        }
        guard += 1;
    }
    Ok(done as u32)
}

pub fn resize_path(g: &Geo, img: &mut [u8], path: &[u8], size: u64, offset: u64, data: &[u8]) -> u32 {
    let count = data.len() as u64;
    if size > u64::from(u32::MAX) || size > g.size as u64 || offset > size || count > size - offset { return E_ARGUMENT; }
    let (parent, leaf) = match path_parent(g, img, path) { Ok(v) => v, Err(e) => return e };
    match stat_path(g, img, path) { Ok((true, _)) => return E_IS_DIR, Ok(_) => {} Err(e) => return e }
    let mut bytes = vec![0u8; size as usize];
    let mut ignored = 0;
    let r = read_file(&parent, img, &leaf, &mut bytes, &mut ignored);
    if r != OK { return r; }
    bytes[offset as usize..(offset + count) as usize].copy_from_slice(data);
    write_file(&parent, img, &leaf, &bytes)
}

fn timestamp(field: &[u8]) -> Option<u64> {
    let time = u64::from(field[0]) | u64::from(field[1]) << 8;
    let date = u64::from(field[2]) | u64::from(field[3]) << 8;
    if date == 0 { return if time == 0 { Some(0) } else { None }; }
    let (year, month, day) = (2000 + (date >> 9), (date >> 5) & 15, date & 31);
    let (hour, minute, second) = (time >> 11, (time >> 5) & 63, (time & 31) * 2);
    const MD: [u64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    if !(1..=12).contains(&month) || day < 1 || day > MD[month as usize - 1] + u64::from(month == 2 && leap) || hour > 23 || minute > 59 || second > 59 { return None; }
    let mut days = (year - 1601) * 365 + (year - 1) / 4 - (year - 1) / 100 + (year - 1) / 400 - 388;
    for m in 1..month { days += MD[m as usize - 1] + u64::from(m == 2 && leap); }
    days += day - 1;
    Some((days * 86400 + hour * 3600 + minute * 60 + second) * 10_000_000)
}

/// Physical directory slot `index` of directory `path`. `Ok((entry, end))`: a deleted slot gives an empty name.
pub fn directory_entry_at(g: &Geo, img: &[u8], path: &[u8], index: u32) -> Result<(DirEntry, bool), u32> {
    let (mut dir, leaf) = path_parent(g, img, path)?;
    if !leaf.is_empty() {
        let Some(e) = find_entry(&dir, img, &leaf).0 else { return Err(E_NOT_FOUND) };
        if img[e + 1] & 0x10 == 0 { return Err(E_NOT_DIR); }
        let cluster = rd32(&img[e + 0x2C..]);
        if cluster < 2 || cluster > g.cluster_count || !g.is_eoc(g.fat_get(img, cluster)) { return Err(E_CORRUPT); }
        dir.root_cluster = cluster;
    }
    let mut out = DirEntry::default();
    if index as usize >= ENTRIES_PER_DIR { return Ok((out, true)); }
    let e = g.cluster_pos(dir.root_cluster) + index as usize * ENTRY_BYTES;
    let n = usize::from(img[e]);
    if n == usize::from(DIR_END) || n == 0 { return Ok((out, true)); }
    if img[e] == DIR_DELETED { return Ok((out, false)); }
    if n > NAME_MAX || img[e + 2..e + 2 + n].contains(&0) { return Err(E_CORRUPT); }
    out.name = img[e + 2..e + 2 + n].to_vec();
    out.attributes = img[e + 1];
    out.directory = img[e + 1] & 0x10 != 0;
    out.size = if out.directory { 0 } else { rd32(&img[e + 0x30..]) };
    let (mut cluster, mut clusters) = (rd32(&img[e + 0x2C..]), 0u32);
    if cluster != 0 {
        loop {
            if cluster < 2 || cluster > g.cluster_count || clusters >= g.cluster_count { return Err(E_CORRUPT); }
            clusters += 1;
            let next = g.fat_get(img, cluster);
            if g.is_eoc(next) { break; }
            cluster = next;
        }
    }
    out.allocation = u64::from(clusters) * CLUSTER_BYTES as u64;
    if u64::from(out.size) > out.allocation || (out.directory && clusters != 1) { return Err(E_CORRUPT); }
    match (timestamp(&img[e + 0x34..]), timestamp(&img[e + 0x38..]), timestamp(&img[e + 0x3C..])) {
        (Some(w), Some(c), Some(a)) => { out.write_time = w; out.creation_time = c; out.access_time = a; }
        _ => return Err(E_CORRUPT),
    }
    Ok((out, false))
}

fn directory_empty(g: &Geo, img: &[u8], cluster: u32) -> bool {
    if cluster < 2 || cluster > g.cluster_count { return false; }
    let dir = g.cluster_pos(cluster);
    for i in 0..ENTRIES_PER_DIR {
        match img[dir + i * ENTRY_BYTES] {
            DIR_END => return true,
            DIR_DELETED => {}
            _ => return false,
        }
    }
    true
}

/// T2112 (port of T2110 `fatx_remove_path`): remove a file or an EMPTY directory (non-empty: `E_EXISTS`).
pub fn remove_path(g: &Geo, img: &mut [u8], path: &[u8]) -> u32 {
    let (parent, leaf) = match path_parent(g, img, path) { Ok(p) => p, Err(e) => return e };
    if leaf.is_empty() { return E_ARGUMENT; }
    let Some(e) = find_entry(&parent, img, &leaf).0 else { return E_NOT_FOUND };
    let cluster = rd32(&img[e + 0x2C..]);
    if img[e + 1] & 0x10 != 0 && !directory_empty(g, img, cluster) { return E_EXISTS; }
    free_chain(g, img, cluster);
    img[e] = DIR_DELETED;
    OK
}

/// Whether `inner` names `outer` or something below it (separators normalised, ASCII case-insensitive).
fn path_within(outer: &[u8], inner: &[u8]) -> bool {
    let norm = |p: &[u8]| -> Vec<u8> {
        let start = p.iter().position(|&c| c != b'\\' && c != b'/').unwrap_or(p.len());
        p[start..].iter().map(|&c| if c == b'/' { b'\\' } else { c.to_ascii_lowercase() }).collect()
    };
    let (outer, inner) = (norm(outer), norm(inner));
    inner.starts_with(&outer) && (inner.len() == outer.len() || inner[outer.len()] == b'\\')
}

fn set_name(entry: &mut [u8], name: &[u8]) {
    entry[2..2 + NAME_MAX].fill(0);
    entry[2..2 + name.len()].copy_from_slice(name);
    entry[0] = name.len() as u8;
}

/// T2112 (port of T2110 `fatx_rename_path`): rename/move an entry; an existing file target is replaced only
/// when `replace` is set.
pub fn rename_path(g: &Geo, img: &mut [u8], from: &[u8], to: &[u8], replace: bool) -> u32 {
    let (src_parent, src_leaf) = match path_parent(g, img, from) { Ok(p) => p, Err(e) => return e };
    let (dst_parent, dst_leaf) = match path_parent(g, img, to) { Ok(p) => p, Err(e) => return e };
    if src_leaf.is_empty() || dst_leaf.is_empty() { return E_ARGUMENT; }
    let Some(src) = find_entry(&src_parent, img, &src_leaf).0 else { return E_NOT_FOUND };
    let src_dir = img[src + 1] & 0x10 != 0;
    let (dst, mut slot) = find_entry(&dst_parent, img, &dst_leaf);
    if dst == Some(src) {
        set_name(&mut img[src..], &dst_leaf);
        return OK;
    }
    if src_dir && path_within(from, to) { return E_ARGUMENT; }
    if let Some(d) = dst {
        if !replace { return E_EXISTS; }
        if img[d + 1] & 0x10 != 0 || src_dir { return E_IS_DIR; }
        free_chain(g, img, rd32(&img[d + 0x2C..]));
        slot = Some(d);
    }
    let Some(slot) = slot else { return E_DIR_FULL };
    let mut moved = [0u8; ENTRY_BYTES];
    moved.copy_from_slice(&img[src..src + ENTRY_BYTES]);
    set_name(&mut moved, &dst_leaf);
    let was_end = img[slot] == DIR_END;
    img[src] = DIR_DELETED;
    img[slot..slot + ENTRY_BYTES].copy_from_slice(&moved);
    if was_end && slot + ENTRY_BYTES < g.cluster_pos(dst_parent.root_cluster) + CLUSTER_BYTES {
        img[slot + ENTRY_BYTES] = DIR_END;
    }
    OK
}
