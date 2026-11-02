//! T2319: one PC save device. Owned state and filesystem operations, no guest-memory access.
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::vfs::{NtStatus, Vfs};

/// The legacy HDD slot identifies this device at the hybrid boundary. U: and T: are
/// namespaces on the same device, never additional devices.
pub const LOCAL_DEVICE_ID: u32 = 9;
pub const TITLE_ID: &str = "45410066";

/// XDG data location on Linux. Relative XDG values are ignored as required by XDG.
/// Environment arguments keep resolution deterministic and testable without mutating env.
pub fn standard_path(home: Option<&Path>, xdg_data_home: Option<&Path>) -> io::Result<PathBuf> {
    let base = match xdg_data_home.filter(|p| p.is_absolute()) {
        Some(path) => path.to_path_buf(),
        None => home
            .filter(|p| p.is_absolute())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "no absolute HOME or XDG_DATA_HOME for PC saves",
                )
            })?
            .join(".local/share"),
    };
    Ok(base.join("tsfp/saves"))
}

pub fn user_path() -> io::Result<PathBuf> {
    standard_path(
        std::env::var_os("HOME").as_deref().map(Path::new),
        std::env::var_os("XDG_DATA_HOME").as_deref().map(Path::new),
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalDevice {
    pub id: u32,
    pub root: PathBuf,
}

/// Successful construction makes one device available. Filesystem failure is an error,
/// never fabricated availability. Logical device presence is stable for its lifetime;
/// save operations still surface I/O errors (permissions, disk full, removed directories).
pub struct LocalStorage {
    device: LocalDevice,
}

impl LocalStorage {
    pub fn open(root: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&root)?;
        if !fs::metadata(&root)?.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "save root is not a directory",
            ));
        }
        Ok(Self {
            device: LocalDevice {
                id: LOCAL_DEVICE_ID,
                root,
            },
        })
    }
    pub fn for_user() -> io::Result<Self> {
        Self::open(user_path()?)
    }
    pub fn devices(&self) -> &[LocalDevice] {
        std::slice::from_ref(&self.device)
    }
    pub fn device(&self, id: u32) -> Option<&LocalDevice> {
        (id == LOCAL_DEVICE_ID).then_some(&self.device)
    }
    pub fn root(&self) -> &Path {
        &self.device.root
    }

    /// Native save APIs use the same U:/T: namespaces as the format-profile backend.
    pub fn vfs(&self) -> Result<Vfs, NtStatus> {
        let mut vfs = Vfs::new();
        vfs.mount_host_dir("\\Device\\Harddisk0\\partition1", self.root())?;
        for (letter, namespace) in [("U:", "UDATA"), ("T:", "TDATA")] {
            fs::create_dir_all(self.root().join(namespace).join(TITLE_ID))
                .map_err(|_| crate::vfs::status::ACCESS_DENIED)?;
            let target = format!("\\Device\\Harddisk0\\partition1\\{namespace}\\{TITLE_ID}");
            vfs.add_symlink(letter, &target);
            vfs.add_symlink(&format!("\\??\\{letter}"), &target);
        }
        Ok(vfs)
    }
}

/// Migration copies complete title save directories; existing destination saves are never
/// overwritten. A conflict is reported even when only SaveMeta.xbx differs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MigrationReport {
    pub imported: Vec<PathBuf>,
    pub already_present: Vec<PathBuf>,
    pub conflicts: Vec<PathBuf>,
}

fn valid_component(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', '\0'])
}

fn validate_save_source_paths(files: &[(PathBuf, Vec<u8>)]) -> io::Result<()> {
    use std::collections::BTreeMap;
    let mut names = BTreeMap::<String, (String, bool)>::new();
    for (path, _) in files {
        let parts: Vec<_> = path.iter().map(|part| part.to_str()).collect();
        if parts.is_empty()
            || parts
                .iter()
                .any(|part| part.is_none_or(|s| !valid_component(s)))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsafe title save path",
            ));
        }
        let (mut original, mut folded) = (String::new(), String::new());
        for (index, component) in parts.iter().map(|part| part.unwrap()).enumerate() {
            if !original.is_empty() {
                original.push('/');
                folded.push('/');
            }
            original.push_str(component);
            folded.push_str(&component.to_ascii_lowercase());
            let is_file = index + 1 == parts.len();
            if let Some((previous, previous_is_file)) = names.get(&folded) {
                if previous != &original || *previous_is_file != is_file {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "case-colliding or structurally conflicting source paths",
                    ));
                }
            } else {
                names.insert(folded.clone(), (original.clone(), is_file));
            }
        }
    }
    Ok(())
}

fn case_child(parent: &Path, name: &str) -> io::Result<Option<PathBuf>> {
    let mut found = None;
    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        if entry
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case(name)
        {
            if found.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "case-colliding save paths",
                ));
            }
            if entry.file_type()?.is_symlink() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "save path symlink refused",
                ));
            }
            found = Some(entry.path());
        }
    }
    Ok(found)
}

/// Read-only source abstraction permits directory and FATX migration through one contract.
/// Entries must be complete file payloads, relative to the title directory.
pub trait SaveSource {
    fn title_files(&self, namespace: &str) -> io::Result<Vec<(PathBuf, Vec<u8>)>>;
}

pub struct DirectorySource(pub PathBuf);
impl SaveSource for DirectorySource {
    fn title_files(&self, namespace: &str) -> io::Result<Vec<(PathBuf, Vec<u8>)>> {
        let Some(base) = case_child(&self.0, namespace)? else {
            return Ok(Vec::new());
        };
        let Some(title) = case_child(&base, TITLE_ID)? else {
            return Ok(Vec::new());
        };
        let mut files = Vec::new();
        fn walk(
            base: &Path,
            rel: &Path,
            files: &mut Vec<(PathBuf, Vec<u8>)>,
            depth: u32,
        ) -> io::Result<()> {
            if depth > 16 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "save tree too deep",
                ));
            }
            for entry in fs::read_dir(base.join(rel))? {
                let entry = entry?;
                let kind = entry.file_type()?;
                let name = entry.file_name().into_string().map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "non-UTF8 save path")
                })?;
                if !valid_component(&name) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "unsafe save path",
                    ));
                }
                let next = rel.join(name);
                if kind.is_symlink() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "save source symlink refused",
                    ));
                }
                if kind.is_dir() {
                    walk(base, &next, files, depth + 1)?;
                } else if kind.is_file() {
                    files.push((next, fs::read(entry.path())?));
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "save source special file refused",
                    ));
                }
            }
            Ok(())
        }
        walk(&title, Path::new(""), &mut files, 0)?;
        Ok(files)
    }
}

/// FATX partition source (read only, streaming random access). Full disk callers pass an
/// explicit partition offset and size; migration never guesses a disk's partition table.
pub struct FatxSource<S: crate::fatx::ReadAt>(pub crate::fatx::Fatx<S>);
impl<S: crate::fatx::ReadAt> SaveSource for FatxSource<S> {
    fn title_files(&self, namespace: &str) -> io::Result<Vec<(PathBuf, Vec<u8>)>> {
        use crate::fatx::FatxError;
        fn error(e: FatxError) -> io::Error {
            io::Error::new(io::ErrorKind::InvalidData, format!("FATX: {e:?}"))
        }
        let base = format!("{namespace}/{TITLE_ID}");
        match self.0.read_dir(&base) {
            Err(FatxError::NotFound) => return Ok(Vec::new()),
            Err(e) => return Err(error(e)),
            Ok(_) => {}
        }
        let mut files = Vec::new();
        fn walk<S: crate::fatx::ReadAt>(
            fatx: &crate::fatx::Fatx<S>,
            base: &str,
            rel: &Path,
            files: &mut Vec<(PathBuf, Vec<u8>)>,
            depth: u32,
        ) -> io::Result<()> {
            if depth > 16 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "FATX save tree too deep",
                ));
            }
            let path = if rel.as_os_str().is_empty() {
                base.to_string()
            } else {
                format!("{base}/{}", rel.display())
            };
            for entry in fatx.read_dir(&path).map_err(error)? {
                if !valid_component(&entry.name) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "unsafe FATX save path",
                    ));
                }
                let next = rel.join(&entry.name);
                if entry.is_directory() {
                    walk(fatx, base, &next, files, depth + 1)?;
                } else {
                    // Profiles and Map Maker files are small; refuse corrupt giant declarations.
                    if entry.size > 64 * 1024 * 1024 {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "save exceeds 64 MiB",
                        ));
                    }
                    let mut bytes = vec![0; entry.size as usize];
                    let n = fatx.read(&entry, 0, &mut bytes).map_err(error)?;
                    if n != bytes.len() {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "short FATX save",
                        ));
                    }
                    files.push((next, bytes));
                }
            }
            Ok(())
        }
        walk(&self.0, &base, Path::new(""), &mut files, 0)?;
        Ok(files)
    }
}

impl LocalStorage {
    pub fn migrate(&self, source: &impl SaveSource) -> io::Result<MigrationReport> {
        use std::collections::BTreeMap;
        use std::io::Write;
        let mut report = MigrationReport::default();
        let sources = ["UDATA", "TDATA"]
            .map(|namespace| source.title_files(namespace))
            .into_iter()
            .collect::<io::Result<Vec<_>>>()?;
        for files in &sources {
            validate_save_source_paths(files)?;
        }
        for (namespace, files) in ["UDATA", "TDATA"].into_iter().zip(sources) {
            let mut groups: BTreeMap<String, Vec<(PathBuf, Vec<u8>)>> = BTreeMap::new();
            let mut title_files = Vec::new();
            for (path, bytes) in files {
                let parts: Vec<_> = path.iter().map(|s| s.to_str()).collect();
                if parts.len() == 1 {
                    title_files.push((parts[0].unwrap().to_ascii_lowercase(), bytes));
                    continue;
                }
                let group = parts[0].unwrap().to_ascii_lowercase();
                groups
                    .entry(group)
                    .or_default()
                    .push((path.iter().skip(1).collect(), bytes));
            }
            let parent = self.root().join(namespace).join(TITLE_ID);
            fs::create_dir_all(&parent)?;
            for (name, bytes) in title_files {
                let relative = PathBuf::from(namespace).join(TITLE_ID).join(&name);
                if let Some(existing) = case_child(&parent, &name)? {
                    if fs::read(existing)? == bytes {
                        report.already_present.push(relative);
                    } else {
                        report.conflicts.push(relative);
                    }
                    continue;
                }
                let staging = parent.join(format!(".tsfp-migrate-{}-{name}", std::process::id()));
                let mut file = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&staging)?;
                file.write_all(&bytes)?;
                file.sync_all()?;
                // hard_link atomically refuses existing destinations; only the owned temporary
                // name is removed after publication. The source image/directory is untouched.
                fs::hard_link(&staging, parent.join(&name))?;
                fs::remove_file(&staging)?;
                fs::File::open(&parent)?.sync_all()?;
                report.imported.push(relative);
            }
            for (name, entries) in groups {
                let relative = PathBuf::from(namespace).join(TITLE_ID).join(&name);
                if let Some(existing) = case_child(&parent, &name)? {
                    let same = entries
                        .iter()
                        .all(|(p, b)| fs::read(existing.join(p)).is_ok_and(|old| old == *b));
                    if same {
                        report.already_present.push(relative);
                    } else {
                        report.conflicts.push(relative);
                    }
                    continue;
                }
                // create_dir is exclusive; rename publishes a complete directory atomically.
                let staging = parent.join(format!(".tsfp-migrate-{}-{name}", std::process::id()));
                fs::create_dir(&staging)?;
                let result = (|| {
                    let mut seen = std::collections::BTreeSet::new();
                    for (path, bytes) in &entries {
                        if !seen.insert(path.to_string_lossy().to_ascii_lowercase()) {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "case-colliding source files",
                            ));
                        }
                        let out = staging.join(path);
                        if let Some(dir) = out.parent() {
                            fs::create_dir_all(dir)?;
                        }
                        let mut file = fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(out)?;
                        file.write_all(bytes)?;
                        file.sync_all()?;
                    }
                    // Never replace a concurrent import, even an empty destination directory.
                    // Linux renameat2(RENAME_NOREPLACE) is used by publish_directory below.
                    publish_directory(&staging, &parent.join(&name))?;
                    fs::File::open(&parent)?.sync_all()?;
                    Ok(())
                })();
                if let Err(e) = result {
                    // Retain a failed staged copy for recovery; no broad recursive deletion.
                    return Err(io::Error::new(
                        e.kind(),
                        format!("migration retained at {}: {e}", staging.display()),
                    ));
                }
                report.imported.push(relative);
            }
        }
        Ok(report)
    }
}

#[cfg(target_os = "linux")]
fn publish_directory(from: &Path, to: &Path) -> io::Result<()> {
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        from,
        rustix::fs::CWD,
        to,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(io::Error::from)?;
    Ok(())
}
#[cfg(not(target_os = "linux"))]
fn publish_directory(_from: &Path, _to: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic no-replace directory migration requires Linux",
    ))
}
