//! Differential dump: one line per disc file `path<TAB>size<TAB>sector<TAB>fnv`, sorted. The hash
//! covers three ranges read through the Vfs handle API (head 64 KiB, middle 64 KiB, last 4 KiB).
//! The C counterpart is tests/c/xdvdfs_dump.c; tools/t2047_fileio_diff.py compares them.
use std::{io::SeekFrom, path::PathBuf};
use tsfp_engine_fileio::vfs::{Access, Disposition, Vfs};
use tsfp_engine_fileio::xiso::Xiso;
use tsfp_engine_fileio::{FNV_OFFSET, fnv1a64};

fn main() {
    let Some(iso) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: tsfp-fileio-dump <disc.iso>");
        std::process::exit(2);
    };
    let disc = Xiso::open(&iso).expect("open disc");
    let mut listing = disc.walk().expect("walk disc");
    listing.retain(|(_, e)| !e.is_directory());
    let mut vfs = Vfs::new();
    vfs.mount_disc("\\Device\\CdRom0", disc);
    vfs.add_symlink("D:", "\\Device\\CdRom0");
    let mut lines = Vec::new();
    for (path, e) in listing {
        let h = vfs.open(&format!("D:\\{path}"), Access::READ, Disposition::Open).expect("open");
        let size = u64::from(e.size);
        let mut hash = FNV_OFFSET;
        let ranges = [(0, size.min(65536)), (size / 2, size.min(65536)), (size.saturating_sub(4096), size.min(4096))];
        for (off, len) in ranges {
            vfs.seek(h, SeekFrom::Start(off)).unwrap();
            let mut buf = vec![0u8; len as usize];
            let n = vfs.read(h, &mut buf).unwrap();
            hash = fnv1a64(hash, &buf[..n]);
        }
        vfs.close(h).unwrap();
        lines.push(format!("{path}\t{size}\t{}\t{hash:016x}", e.start_sector));
    }
    lines.sort();
    for l in lines { println!("{l}"); }
}
