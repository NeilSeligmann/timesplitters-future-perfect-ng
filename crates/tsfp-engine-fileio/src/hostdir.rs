//! Host-directory volume path resolution (T2074 slice 2). A statement-for-statement port of the walk and
//! name-lookup half of `src/xbox/kernel_file.c` (`hostdir_walk_locked`, `hostdir_real_name_locked`,
//! `hostdir_match_case_locked`), the part that carries the escape guarantee: a guest path never becomes a host
//! path string, it is walked one component at a time from the volume root through [`HostOps`] primitives that
//! open with `O_NOFOLLOW`, and a symbolic link, `.` or `..` is refused. The OS calls stay behind the trait (the C
//! host supplies them), so the algorithm is testable on an in-memory tree.
//! Evidence class: ported from the C original, Tier 1 against it (tests/c/t2074_hostdir_diff.c) and Tier 2 on the Story route.

/// What a directory entry is, as seen with `AT_SYMLINK_NOFOLLOW`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Dir,
    Link,
    Other,
}

/// OS primitives. Descriptors are plain integers owned by the caller side; every `i32` handed out by `dup`/`open_dir`
/// is closed exactly once through `close`.
pub trait HostOps {
    fn dup(&mut self, fd: i32) -> Option<i32>;
    fn close(&mut self, fd: i32);
    /// `fstatat(dir, name, AT_SYMLINK_NOFOLLOW)`; the C side keeps the full `struct stat` of the last success.
    fn stat_at(&mut self, dir: i32, name: &[u8]) -> Option<Kind>;
    /// `openat(dir, name, O_RDONLY|O_DIRECTORY|O_NOFOLLOW)`.
    fn open_dir(&mut self, dir: i32, name: &[u8]) -> Option<i32>;
    /// Enumerate `dir` on a fresh descriptor; `visit` returns true to stop. False when the directory cannot be scanned.
    fn scan(&mut self, dir: i32, visit: &mut dyn FnMut(&[u8]) -> bool) -> bool;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WalkError {
    /// No start descriptor (volume not prepared).
    NoStart,
    /// `dup` failed.
    NoDescriptor,
    /// A component of this many bytes does not fit.
    TooLong(usize),
    /// A `.` or `..` component.
    Dot,
    /// An intermediate component is missing or not a directory.
    PathNotFound,
    /// An intermediate component is a symbolic link (its real spelling).
    Link(Vec<u8>),
}

fn is_sep(b: u8) -> bool {
    b == b'\\' || b == b'/'
}

/// ASCII case-insensitive equality, as `paths_equal` in the C module (never locale dependent).
pub fn names_equal(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_ascii_lowercase() == y.to_ascii_lowercase())
}

/// `hostdir_real_name_locked`: the spelling `wanted` has inside `dir`, with its kind; exact probe first, then a
/// case-insensitive scan (first match wins), `out_bytes` bounds the name like the C buffer (`len < out_bytes`).
pub fn real_name(ops: &mut dyn HostOps, dir: i32, wanted: &[u8], out_bytes: usize) -> Option<(Vec<u8>, Kind)> {
    if let Some(kind) = ops.stat_at(dir, wanted) {
        return if wanted.len() >= out_bytes { None } else { Some((wanted.to_vec(), kind)) };
    }
    let mut found: Option<Vec<u8>> = None;
    let scanned = ops.scan(dir, &mut |name| {
        if !names_equal(name, wanted) {
            return false;
        }
        if name.len() < out_bytes {
            found = Some(name.to_vec());
        }
        true
    });
    if !scanned {
        return None;
    }
    let name = found?;
    let kind = ops.stat_at(dir, &name)?;
    Some((name, kind))
}

/// `hostdir_walk_locked`: walk `rest` from `start_fd`, return the descriptor of the directory holding the last
/// component plus that component (empty for the volume root). `pending_cap` is `KERNEL_FILE_PATH_MAX`, `leaf_bytes`
/// the caller's leaf buffer size.
pub fn walk(ops: &mut dyn HostOps, start_fd: i32, rest: &[u8], pending_cap: usize, leaf_bytes: usize) -> Result<(i32, Vec<u8>), WalkError> {
    if start_fd < 0 {
        return Err(WalkError::NoStart);
    }
    let mut dir = ops.dup(start_fd).ok_or(WalkError::NoDescriptor)?;
    let mut pending: Vec<u8> = Vec::new();
    let mut at = 0usize;
    loop {
        while at < rest.len() && is_sep(rest[at]) {
            at += 1;
        }
        if at >= rest.len() {
            break;
        }
        let start = at;
        while at < rest.len() && !is_sep(rest[at]) {
            at += 1;
        }
        let comp = &rest[start..at];
        if comp.len() >= pending_cap || comp.len() >= leaf_bytes {
            ops.close(dir);
            return Err(WalkError::TooLong(comp.len()));
        }
        if comp == b"." || comp == b".." {
            ops.close(dir);
            return Err(WalkError::Dot);
        }
        if !pending.is_empty() {
            let Some((real, kind)) = real_name(ops, dir, &pending, pending_cap) else {
                ops.close(dir);
                return Err(WalkError::PathNotFound);
            };
            if kind == Kind::Link {
                ops.close(dir);
                return Err(WalkError::Link(real));
            }
            let next = ops.open_dir(dir, &real);
            ops.close(dir);
            dir = next.ok_or(WalkError::PathNotFound)?;
        }
        pending = comp.to_vec();
    }
    Ok((dir, pending))
}
