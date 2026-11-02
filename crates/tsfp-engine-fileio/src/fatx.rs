//! Minimal read-only FATX volume reader (T2056): memory-unit and partition images.
//!
//! Layout (INFERRED from the public FATX description and the C host's `fatx_volume` use, not yet
//! compared with the C implementation on a real image): superblock `"FATX"` at 0 (volume id u32,
//! sectors per cluster u32, root cluster u32), FAT at 0x1000, FAT16 when the cluster count is below
//! 65525 else FAT32, data area after the FAT (rounded up to 0x1000), root directory at the root
//! cluster, 64-byte directory entries (name length u8, attributes u8, 42-byte name, first cluster
//! u32, size u32, three u32 timestamps). Name length 0xE5 is deleted, 0 and 0xFF end a directory.
use std::fs::File;
use std::os::unix::fs::FileExt;

pub const SECTOR: u64 = 512;
pub const FAT_OFFSET: u64 = 0x1000;
pub const ENTRY_BYTES: usize = 64;
pub const ATTR_DIRECTORY: u8 = 0x10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FatxError { Magic, Geometry, Truncated, Chain, NotFound, NotDirectory }

/// Random-access byte source: an in-memory image or a file.
pub trait ReadAt { fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<(), FatxError>; }
impl ReadAt for Vec<u8> {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<(), FatxError> {
        let start = usize::try_from(offset).map_err(|_| FatxError::Truncated)?;
        let src = self.get(start..start + buf.len()).ok_or(FatxError::Truncated)?;
        buf.copy_from_slice(src);
        Ok(())
    }
}
impl ReadAt for File {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<(), FatxError> { self.read_exact_at(buf, offset).map_err(|_| FatxError::Truncated) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FatxEntry { pub name: String, pub attributes: u8, pub first_cluster: u32, pub size: u32 }
impl FatxEntry { pub fn is_directory(&self) -> bool { self.attributes & ATTR_DIRECTORY != 0 } }

pub struct Fatx<S: ReadAt> {
    src: S,
    cluster_bytes: u64,
    clusters: u64,
    fat32: bool,
    data_offset: u64,
    root_cluster: u32,
}

impl<S: ReadAt> Fatx<S> {
    /// `volume_bytes` is the size of the partition or image.
    pub fn open(src: S, volume_bytes: u64) -> Result<Self, FatxError> {
        let mut sb = [0u8; 16];
        src.read_at(&mut sb, 0)?;
        if &sb[..4] != b"FATX" { return Err(FatxError::Magic); }
        let spc = u64::from(u32::from_le_bytes(sb[8..12].try_into().unwrap()));
        let root_cluster = u32::from_le_bytes(sb[12..16].try_into().unwrap());
        if spc == 0 || !spc.is_power_of_two() || spc > 128 { return Err(FatxError::Geometry); }
        let cluster_bytes = spc * SECTOR;
        let clusters = volume_bytes.saturating_sub(FAT_OFFSET) / cluster_bytes;
        let fat32 = clusters >= 65_525;
        let fat_bytes = (clusters + 2) * if fat32 { 4 } else { 2 };
        let data_offset = FAT_OFFSET + fat_bytes.div_ceil(0x1000) * 0x1000;
        if root_cluster < 1 || u64::from(root_cluster) > clusters { return Err(FatxError::Geometry); }
        Ok(Self { src, cluster_bytes, clusters, fat32, data_offset, root_cluster })
    }
    pub fn is_fat32(&self) -> bool { self.fat32 }
    pub fn cluster_bytes(&self) -> u64 { self.cluster_bytes }

    fn fat_entry(&self, cluster: u32) -> Result<u32, FatxError> {
        if u64::from(cluster) > self.clusters + 1 { return Err(FatxError::Chain); }
        if self.fat32 {
            let mut b = [0u8; 4];
            self.src.read_at(&mut b, FAT_OFFSET + u64::from(cluster) * 4)?;
            Ok(u32::from_le_bytes(b))
        } else {
            let mut b = [0u8; 2];
            self.src.read_at(&mut b, FAT_OFFSET + u64::from(cluster) * 2)?;
            Ok(u32::from(u16::from_le_bytes(b)))
        }
    }
    fn is_end(&self, v: u32) -> bool { if self.fat32 { v >= 0x0FFF_FFF8 } else { v >= 0xFFF8 } }
    /// Cluster chain starting at `first`, bounded by the cluster count (a loop is `Chain`).
    fn chain(&self, first: u32) -> Result<Vec<u32>, FatxError> {
        let mut out = Vec::new();
        let mut cur = first;
        while !self.is_end(cur) {
            if cur < 1 || u64::from(cur) > self.clusters || out.len() as u64 > self.clusters { return Err(FatxError::Chain); }
            out.push(cur);
            cur = self.fat_entry(cur)?;
        }
        Ok(out)
    }
    fn cluster_offset(&self, cluster: u32) -> u64 { self.data_offset + u64::from(cluster - 1) * self.cluster_bytes }

    fn read_dir_cluster(&self, first: u32) -> Result<Vec<FatxEntry>, FatxError> {
        let mut out = Vec::new();
        for c in self.chain(first)? {
            let mut buf = vec![0u8; self.cluster_bytes as usize];
            self.src.read_at(&mut buf, self.cluster_offset(c))?;
            for raw in buf.chunks_exact(ENTRY_BYTES) {
                let len = raw[0];
                if len == 0 || len == 0xFF { return Ok(out); }
                if len == 0xE5 || len > 42 { continue; }
                out.push(FatxEntry {
                    name: String::from_utf8_lossy(&raw[2..2 + usize::from(len)]).into_owned(),
                    attributes: raw[1],
                    first_cluster: u32::from_le_bytes(raw[44..48].try_into().unwrap()),
                    size: u32::from_le_bytes(raw[48..52].try_into().unwrap()),
                });
            }
        }
        Ok(out)
    }
    pub fn read_dir(&self, path: &str) -> Result<Vec<FatxEntry>, FatxError> {
        let mut cluster = self.root_cluster;
        for part in path.split(['/', '\\']).filter(|p| !p.is_empty()) {
            let e = self.read_dir_cluster(cluster)?.into_iter().find(|e| e.name.eq_ignore_ascii_case(part)).ok_or(FatxError::NotFound)?;
            if !e.is_directory() { return Err(FatxError::NotDirectory); }
            cluster = e.first_cluster;
        }
        self.read_dir_cluster(cluster)
    }
    pub fn lookup(&self, path: &str) -> Result<FatxEntry, FatxError> {
        let (dir, name) = path.trim_matches(['/', '\\']).rsplit_once(['/', '\\']).unwrap_or(("", path.trim_matches(['/', '\\'])));
        self.read_dir(dir)?.into_iter().find(|e| e.name.eq_ignore_ascii_case(name)).ok_or(FatxError::NotFound)
    }
    /// Read up to `buf.len()` bytes of a file at `offset`; clamped at the file size.
    pub fn read(&self, entry: &FatxEntry, offset: u64, buf: &mut [u8]) -> Result<usize, FatxError> {
        if entry.is_directory() || offset >= u64::from(entry.size) { return Ok(0); }
        let want = buf.len().min((u64::from(entry.size) - offset) as usize);
        let chain = self.chain(entry.first_cluster)?;
        let mut done = 0;
        while done < want {
            let pos = offset + done as u64;
            let index = (pos / self.cluster_bytes) as usize;
            let within = pos % self.cluster_bytes;
            let cluster = *chain.get(index).ok_or(FatxError::Chain)?;
            let n = (want - done).min((self.cluster_bytes - within) as usize);
            self.src.read_at(&mut buf[done..done + n], self.cluster_offset(cluster) + within)?;
            done += n;
        }
        Ok(want)
    }
}
