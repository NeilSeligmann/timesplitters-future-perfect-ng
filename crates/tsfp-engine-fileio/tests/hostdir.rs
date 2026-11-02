//! Spec tests for the host-dir walk on an in-memory tree (T2074 slice 2). FABRICATED inputs, Tier 0/1.
use std::collections::HashMap;
use tsfp_engine_fileio::hostdir::{HostOps, Kind, WalkError, names_equal, real_name, walk};

#[derive(Default)]
struct Tree {
    // dir id -> entries
    dirs: HashMap<i32, Vec<(Vec<u8>, Kind, i32)>>,
    open: Vec<i32>,
    next: i32,
}
impl Tree {
    fn add(&mut self, dir: i32, name: &str, kind: Kind) -> i32 {
        self.next += 1;
        let id = 100 + self.next;
        self.dirs.entry(dir).or_default().push((name.as_bytes().to_vec(), kind, id));
        if kind == Kind::Dir {
            self.dirs.entry(id).or_default();
        }
        id
    }
}
impl HostOps for Tree {
    fn dup(&mut self, fd: i32) -> Option<i32> {
        self.open.push(fd);
        Some(fd)
    }
    fn close(&mut self, fd: i32) {
        let at = self.open.iter().position(|d| *d == fd).expect("close of a descriptor that is not open");
        self.open.remove(at);
    }
    fn stat_at(&mut self, dir: i32, name: &[u8]) -> Option<Kind> {
        self.dirs.get(&dir)?.iter().find(|e| e.0 == name).map(|e| e.1)
    }
    fn open_dir(&mut self, dir: i32, name: &[u8]) -> Option<i32> {
        let e = self.dirs.get(&dir)?.iter().find(|e| e.0 == name && e.1 == Kind::Dir)?;
        let id = e.2;
        self.open.push(id);
        Some(id)
    }
    fn scan(&mut self, dir: i32, visit: &mut dyn FnMut(&[u8]) -> bool) -> bool {
        let Some(list) = self.dirs.get(&dir) else { return false };
        for e in list {
            if visit(&e.0) {
                break;
            }
        }
        true
    }
}

fn tree() -> Tree {
    let mut t = Tree::default();
    t.dirs.insert(1, vec![]);
    let tdata = t.add(1, "TDATA", Kind::Dir);
    t.add(tdata, "save.dat", Kind::Other);
    t.add(1, "evil", Kind::Link);
    t
}

#[test]
fn walks_case_insensitively_and_closes_everything() {
    let mut t = tree();
    t.open.push(1);
    let (dir, leaf) = walk(&mut t, 1, b"\\tdata\\SAVE.DAT", 256, 256).unwrap();
    assert_eq!(leaf, b"SAVE.DAT");
    assert_ne!(dir, 1);
    t.close(dir);
    assert_eq!(t.open, vec![1]);
}

#[test]
fn refuses_dots_links_and_missing_parents() {
    let mut t = tree();
    assert_eq!(walk(&mut t, 1, b"a/../b", 256, 256), Err(WalkError::Dot));
    assert_eq!(walk(&mut t, 1, b"evil/x", 256, 256), Err(WalkError::Link(b"evil".to_vec())));
    assert_eq!(walk(&mut t, 1, b"nope/x", 256, 256), Err(WalkError::PathNotFound));
    assert_eq!(walk(&mut t, -1, b"x", 256, 256), Err(WalkError::NoStart));
    assert!(t.open.is_empty());
}

#[test]
fn root_and_too_long() {
    let mut t = tree();
    let (dir, leaf) = walk(&mut t, 1, b"//", 256, 256).unwrap();
    assert!(leaf.is_empty());
    t.close(dir);
    let long = vec![b'a'; 300];
    assert_eq!(walk(&mut t, 1, &long, 256, 256), Err(WalkError::TooLong(300)));
    assert!(t.open.is_empty());
}

#[test]
fn real_name_scan_and_bounds() {
    let mut t = tree();
    assert_eq!(real_name(&mut t, 1, b"tdata", 64).unwrap().0, b"TDATA");
    assert!(real_name(&mut t, 1, b"tdata", 5).is_none());
    assert!(real_name(&mut t, 1, b"missing", 64).is_none());
    assert!(names_equal(b"AbC", b"aBc") && !names_equal(b"ab", b"abc"));
}
