//! cargo run --release --example replay -- <cmdlog>   (compares against the C host's render hashes)
fn main() {
    let path = std::env::args().nth(1).expect("usage: replay <TSFP_AUDIO_CMD_LOG file>");
    let log = std::fs::read_to_string(&path).expect("read log");
    println!("{:#?}", tsfp_audio::replay::replay(&log));
}
