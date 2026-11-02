//! cargo run --release --example adapter_replay -- <cmdlog>
fn main() {
    let path = std::env::args().nth(1).expect("usage: adapter_replay <TSFP_AUDIO_CMD_LOG file>");
    let log = std::fs::read_to_string(&path).expect("read log");
    println!("{:#?}", tsfp_audio::adapter_replay::replay_adapter(&log));
}
