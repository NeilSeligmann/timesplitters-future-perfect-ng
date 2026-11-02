// SPDX-License-Identifier: GPL-3.0-or-later
//! T2143: lookup and read cost of the Rust XDVDFS reader, cold (a fresh `Xiso` per lookup, which is what every lookup cost before the
//! directory cache) against warm (one `Xiso`, cached directories). Usage: `cargo run --release --example bench_xiso -- DISC.iso [N]`.
use std::path::Path;
use std::time::Instant;
use tsfp_engine_fileio::xiso::Xiso;

fn main() {
    let mut args = std::env::args().skip(1);
    let disc = args.next().expect("usage: bench_xiso DISC.iso [lookups]");
    let wanted: usize = args.next().map_or(2000, |n| n.parse().expect("lookups"));
    let image = Xiso::open(Path::new(&disc)).expect("open the disc");
    let files: Vec<String> =
        image.walk().expect("walk").into_iter().filter(|(_, e)| !e.is_directory()).map(|(path, _)| path).take(wanted).collect();
    println!("{} file paths (walk order), disc {} bytes", files.len(), image.image_bytes());
    let started = Instant::now();
    for path in &files {
        let fresh = Xiso::open(Path::new(&disc)).expect("open");
        fresh.lookup(path).expect("lookup");
    }
    let cold = started.elapsed();
    let started = Instant::now();
    for path in &files {
        image.lookup(path).expect("lookup");
    }
    let warm = started.elapsed();
    let started = Instant::now();
    let mut buffer = vec![0u8; 65536];
    let mut total = 0usize;
    for path in files.iter().take(200) {
        let entry = image.lookup(path).expect("lookup");
        total += image.read(&entry, 0, &mut buffer).expect("read");
    }
    let reads = started.elapsed();
    println!("lookup cold (old cost): {:.1} us each, warm: {:.2} us each ({:.0}x)", cold.as_secs_f64() * 1e6 / files.len() as f64,
        warm.as_secs_f64() * 1e6 / files.len() as f64, cold.as_secs_f64() / warm.as_secs_f64());
    println!("lookup + 64 KiB first read, 200 files: {:.1} us each ({total} bytes)", reads.as_secs_f64() * 1e6 / 200.0);
}
