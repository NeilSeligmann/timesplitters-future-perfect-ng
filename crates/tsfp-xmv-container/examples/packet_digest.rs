//! Prints one `xmv-packet[n]:` digest line per packet of an XMV file (lengths and digests only, no bytes).
//! Usage: `cargo run --offline -p tsfp-xmv-container --example packet_digest -- <file.xmv>`

use tsfp_xmv_container::{digest::packet_digest_line, walk_packets};

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: packet_digest <file.xmv>");
        std::process::exit(2);
    };
    let data = std::fs::read(&path).unwrap_or_else(|error| {
        eprintln!("packet_digest: {error}");
        std::process::exit(1);
    });
    match walk_packets(&data) {
        Ok((header, spans)) => {
            for (index, span) in spans.iter().enumerate() {
                println!("{}", packet_digest_line(&data, *span, &header, index));
            }
        }
        Err(error) => {
            eprintln!("packet_digest: {error:?}");
            std::process::exit(1);
        }
    }
}
