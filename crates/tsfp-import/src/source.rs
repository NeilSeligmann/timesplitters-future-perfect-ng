//! Source abstraction: one trait for every release the importer can read.
//!
//! A source is a read-only set of named files. Implemented: XDVDFS (Xbox), ISO 9660 (PS2) and a
//! loose directory (an already extracted tree, or a Dolphin/other extraction of the GameCube
//! disc), and GameCube GCM/ISO (FST reader). Recognised but unsupported: RVZ/WIA (needs zstd,
//! see docs/t-asset-importer.md).
use crate::Result;
use std::cell::RefCell;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub const SECTOR: u64 = 2048;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Xbox,
    Ps2,
    GameCube,
}
impl Platform {
    pub fn name(self) -> &'static str {
        match self {
            Platform::Xbox => "xbox",
            Platform::Ps2 => "ps2",
            Platform::GameCube => "gamecube",
        }
    }
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "xbox" => Ok(Platform::Xbox),
            "ps2" => Ok(Platform::Ps2),
            "gamecube" | "gc" => Ok(Platform::GameCube),
            _ => Err(format!("unknown platform {s:?} (xbox|ps2|gamecube)")),
        }
    }
}

/// One file inside a source. `locator` is layout-specific (start byte for images).
#[derive(Clone, Debug)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub locator: u64,
}

/// Identity of a source. `id` names the version, for example `xbox-usa`.
#[derive(Clone, Debug)]
pub struct Descriptor {
    pub id: String,
    pub platform: Platform,
    pub layout: &'static str,
}

pub trait Source {
    fn descriptor(&self) -> &Descriptor;
    fn list(&self) -> Result<Vec<FileEntry>>;
    fn read(&self, e: &FileEntry) -> Result<Vec<u8>>;
    /// Cheap change stamp of one file (size and modification time, never its content). Empty
    /// when the source cannot tell, which disables hash reuse for it.
    fn stamp(&self, _e: &FileEntry) -> String {
        String::new()
    }
}

fn mtime_nanos(m: &std::fs::Metadata) -> u128 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos())
}

/// Open `path` as a source, detecting the layout.
pub fn open(path: &Path, id: &str, platform: Platform) -> Result<Box<dyn Source>> {
    let d = |layout| Descriptor {
        id: id.to_string(),
        platform,
        layout,
    };
    if path.is_dir() {
        return Ok(Box::new(DirSource {
            root: path.to_path_buf(),
            desc: d("directory"),
        }));
    }
    let mut f = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut probe = |off: u64, n: usize| -> Vec<u8> {
        let mut b = vec![0u8; n];
        if f.seek(SeekFrom::Start(off)).is_ok() && f.read_exact(&mut b).is_ok() {
            b
        } else {
            vec![]
        }
    };
    if probe(0, 4) == b"RVZ\x01" || probe(0, 4) == b"WIA\x01" {
        return Err("RVZ/WIA images need zstd decompression: convert with dolphin-tool to ISO/GCM, or extract to a directory and pass that (planned, see docs/t-asset-importer.md)".into());
    }
    if probe(32 * SECTOR, 20) == b"MICROSOFT*XBOX*MEDIA" {
        let desc = d("xdvdfs");
        return Ok(Box::new(ImageSource {
            file: RefCell::new(File::open(path).map_err(|e| e.to_string())?),
            desc,
            kind: Kind::Xdvdfs,
        }));
    }
    if probe(0x1C, 4) == [0xC2, 0x33, 0x9F, 0x3D] {
        let desc = d("gcm");
        return Ok(Box::new(ImageSource {
            file: RefCell::new(File::open(path).map_err(|e| e.to_string())?),
            desc,
            kind: Kind::Gcm,
        }));
    }
    if probe(16 * SECTOR + 1, 5) == b"CD001" {
        let desc = d("iso9660");
        return Ok(Box::new(ImageSource {
            file: RefCell::new(File::open(path).map_err(|e| e.to_string())?),
            desc,
            kind: Kind::Iso9660,
        }));
    }
    Err(format!("{}: unrecognised source layout", path.display()))
}

/// An already extracted directory tree.
pub struct DirSource {
    root: PathBuf,
    desc: Descriptor,
}
impl DirSource {
    fn walk(&self, dir: &Path, prefix: &str, out: &mut Vec<FileEntry>) -> Result<()> {
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok())
            .collect();
        names.sort_by_key(|e| e.file_name());
        for e in names {
            let name = e.file_name().to_string_lossy().into_owned();
            let ty = e.file_type().map_err(|e| e.to_string())?;
            if ty.is_symlink() {
                continue;
            }
            let rel = format!("{prefix}{name}");
            if ty.is_dir() {
                self.walk(&e.path(), &format!("{rel}/"), out)?;
            } else {
                out.push(FileEntry {
                    path: rel,
                    size: e.metadata().map_err(|e| e.to_string())?.len(),
                    locator: 0,
                });
            }
        }
        Ok(())
    }
}
impl Source for DirSource {
    fn descriptor(&self) -> &Descriptor {
        &self.desc
    }
    fn list(&self) -> Result<Vec<FileEntry>> {
        let mut v = Vec::new();
        self.walk(&self.root, "", &mut v)?;
        Ok(v)
    }
    fn read(&self, e: &FileEntry) -> Result<Vec<u8>> {
        std::fs::read(self.root.join(safe(&e.path)?)).map_err(|x| x.to_string())
    }
    fn stamp(&self, e: &FileEntry) -> String {
        let Ok(rel) = safe(&e.path) else {
            return String::new();
        };
        std::fs::metadata(self.root.join(rel)).map_or(String::new(), |m| {
            format!("{}:{}", m.len(), mtime_nanos(&m))
        })
    }
}

/// Reject absolute paths and `..` so an image can never write outside the output tree.
pub fn safe(p: &str) -> Result<PathBuf> {
    let p = p.replace('\\', "/");
    if p.starts_with('/')
        || p.split('/')
            .any(|c| c == ".." || c.is_empty() || c.contains('\0'))
    {
        return Err(format!("unsafe path in image: {p:?}"));
    }
    Ok(PathBuf::from(p))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Xdvdfs,
    Iso9660,
    Gcm,
}

/// A disc image (XDVDFS, ISO 9660 or GameCube GCM).
pub struct ImageSource {
    file: RefCell<File>,
    desc: Descriptor,
    kind: Kind,
}
impl ImageSource {
    fn at(&self, off: u64, n: usize) -> Result<Vec<u8>> {
        let mut f = self.file.borrow_mut();
        let len = f.metadata().map_err(|e| e.to_string())?.len();
        if off.checked_add(n as u64).is_none_or(|e| e > len) {
            return Err(format!("read past image end: {off}+{n}"));
        }
        f.seek(SeekFrom::Start(off)).map_err(|e| e.to_string())?;
        let mut b = vec![0u8; n];
        f.read_exact(&mut b).map_err(|e| e.to_string())?;
        Ok(b)
    }
    fn xdvdfs_dir(
        &self,
        sector: u64,
        size: u32,
        prefix: &str,
        depth: u32,
        out: &mut Vec<FileEntry>,
    ) -> Result<()> {
        if depth > 32 || size == 0 {
            return Ok(());
        }
        let data = self.at(sector * SECTOR, size as usize)?;
        let mut pending = vec![0usize];
        let mut seen = std::collections::HashSet::new();
        let mut subs = Vec::new();
        while let Some(o) = pending.pop() {
            if !seen.insert(o) || o + 14 > data.len() {
                continue;
            }
            let u16at = |i: usize| u16::from_le_bytes([data[o + i], data[o + i + 1]]);
            let u32at = |i: usize| u32::from_le_bytes(data[o + i..o + i + 4].try_into().unwrap());
            let (left, right, start, len, attr, nl) = (
                u16at(0),
                u16at(2),
                u32at(4),
                u32at(8),
                data[o + 12],
                data[o + 13] as usize,
            );
            let end = o + 14 + nl;
            if end > data.len() {
                continue;
            }
            let name: String = data[o + 14..end].iter().map(|c| *c as char).collect();
            let path = format!("{prefix}{name}");
            if attr & 0x10 != 0 {
                subs.push((start as u64, len, format!("{path}/")));
            } else {
                out.push(FileEntry {
                    path,
                    size: len as u64,
                    locator: start as u64 * SECTOR,
                });
            }
            if left != 0xFFFF {
                pending.push(left as usize * 4);
            }
            if right != 0xFFFF {
                pending.push(right as usize * 4);
            }
            if left == 0xFFFF && right == 0xFFFF && end.next_multiple_of(4) > o {
                pending.push(end.next_multiple_of(4));
            }
        }
        for (s, l, p) in subs {
            self.xdvdfs_dir(s, l, &p, depth + 1, out)?;
        }
        Ok(())
    }
    fn iso_dir(
        &self,
        extent: u64,
        size: u32,
        prefix: &str,
        depth: u32,
        out: &mut Vec<FileEntry>,
    ) -> Result<()> {
        if depth > 32 {
            return Ok(());
        }
        let data = self.at(extent * SECTOR, size as usize)?;
        let mut o = 0;
        while o < data.len() {
            let len = data[o] as usize;
            if len == 0 {
                o = (o / SECTOR as usize + 1) * SECTOR as usize;
                continue;
            }
            if o + len > data.len() || len < 34 {
                break;
            }
            let ext = u32::from_le_bytes(data[o + 2..o + 6].try_into().unwrap()) as u64;
            let sz = u32::from_le_bytes(data[o + 10..o + 14].try_into().unwrap());
            let nl = data[o + 32] as usize;
            let raw = &data[o + 33..(o + 33 + nl).min(o + len)];
            if !(raw == [0] || raw == [1]) {
                let mut name: String = raw.iter().map(|c| *c as char).collect();
                if let Some(i) = name.find(';') {
                    name.truncate(i);
                }
                let path = format!("{prefix}{name}");
                if data[o + 25] & 2 != 0 {
                    self.iso_dir(ext, sz, &format!("{path}/"), depth + 1, out)?;
                } else {
                    out.push(FileEntry {
                        path,
                        size: sz as u64,
                        locator: ext * SECTOR,
                    });
                }
            }
            o += len;
        }
        Ok(())
    }
}
impl Source for ImageSource {
    fn descriptor(&self) -> &Descriptor {
        &self.desc
    }
    fn list(&self) -> Result<Vec<FileEntry>> {
        let mut out = Vec::new();
        if self.kind == Kind::Gcm {
            self.gcm_list(&mut out)?;
        } else if self.kind == Kind::Iso9660 {
            let pvd = self.at(16 * SECTOR, SECTOR as usize)?;
            let ext = u32::from_le_bytes(pvd[158..162].try_into().unwrap()) as u64;
            let sz = u32::from_le_bytes(pvd[166..170].try_into().unwrap());
            self.iso_dir(ext, sz, "", 0, &mut out)?;
        } else {
            let d = self.at(32 * SECTOR, 28)?;
            let root = u32::from_le_bytes(d[20..24].try_into().unwrap()) as u64;
            let size = u32::from_le_bytes(d[24..28].try_into().unwrap());
            self.xdvdfs_dir(root, size, "", 0, &mut out)?;
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }
    fn read(&self, e: &FileEntry) -> Result<Vec<u8>> {
        self.at(e.locator, e.size as usize)
    }
    /// Image size and mtime plus the file's start byte: an unchanged image gives unchanged stamps.
    fn stamp(&self, e: &FileEntry) -> String {
        self.file.borrow().metadata().map_or(String::new(), |m| {
            format!("{}:{}:{}", m.len(), mtime_nanos(&m), e.locator)
        })
    }
}

fn be32(b: &[u8], o: usize) -> u32 {
    u32::from_be_bytes(b[o..o + 4].try_into().unwrap())
}

impl ImageSource {
    /// GameCube GCM: boot header, FST (12 byte big-endian entries plus a name table) and the
    /// `sys/` pseudo files Dolphin also exposes (boot.bin, bi2.bin, apploader.img, main.dol).
    fn gcm_list(&self, out: &mut Vec<FileEntry>) -> Result<()> {
        let hdr = self.at(0, 0x440)?;
        let (dol, fst, fst_size) = (be32(&hdr, 0x420), be32(&hdr, 0x424), be32(&hdr, 0x428));
        out.push(sys("sys/boot.bin", 0, 0x440));
        out.push(sys("sys/bi2.bin", 0x440, 0x2000));
        let ap = self.at(0x2440, 0x20)?;
        let ap_size = 0x20 + be32(&ap, 0x14) as u64 + be32(&ap, 0x18) as u64;
        out.push(sys("sys/apploader.img", 0x2440, ap_size));
        let dh = self.at(dol as u64, 0x100)?;
        let dol_size = (0..18)
            .filter(|i| be32(&dh, 0x90 + i * 4) != 0)
            .map(|i| be32(&dh, i * 4) as u64 + be32(&dh, 0x90 + i * 4) as u64)
            .max()
            .unwrap_or(0);
        out.push(sys("sys/main.dol", dol as u64, dol_size));
        if !(12..=64 << 20).contains(&fst_size) {
            return Err(format!("gcm: implausible FST size {fst_size}"));
        }
        let t = self.at(fst as u64, fst_size as usize)?;
        let count = be32(&t, 8) as usize;
        if count == 0 || count * 12 > t.len() {
            return Err("gcm: FST entry count exceeds table".into());
        }
        let names = &t[count * 12..];
        let name = |i: usize| -> Result<String> {
            let o = (be32(&t, i * 12) & 0xFF_FFFF) as usize;
            let rest = names.get(o..).ok_or("gcm: name offset out of range")?;
            let end = rest
                .iter()
                .position(|c| *c == 0)
                .ok_or("gcm: unterminated name")?;
            Ok(rest[..end].iter().map(|c| *c as char).collect())
        };
        // (directory end index, path prefix)
        let mut stack: Vec<(usize, String)> = vec![(count, String::new())];
        for i in 1..count {
            while stack.len() > 1 && i >= stack.last().unwrap().0 {
                stack.pop();
            }
            let prefix = stack.last().unwrap().1.clone();
            let n = name(i)?;
            let (a, b) = (be32(&t, i * 12 + 4), be32(&t, i * 12 + 8));
            if t[i * 12] == 1 {
                if (b as usize) > count || (b as usize) <= i {
                    return Err("gcm: bad directory extent".into());
                }
                stack.push((b as usize, format!("{prefix}{n}/")));
            } else {
                out.push(FileEntry {
                    path: format!("files/{prefix}{n}"),
                    size: b as u64,
                    locator: a as u64,
                });
            }
        }
        Ok(())
    }
}

fn sys(path: &str, locator: u64, size: u64) -> FileEntry {
    FileEntry {
        path: path.into(),
        size,
        locator,
    }
}
