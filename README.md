# TimeSplitters: Future — community build

This source-only project requires your own Xbox game disc. No game data is included.

## Quick start

1. Install Python 3, CMake, Ninja, C/C++, and Rust stable when prompted.
2. Copy your game disc image to `discs/timesplitters-future.iso`.
3. Run `./run.sh` to extract, build the host, and launch the main menu.

## Launcher

`cargo run --release --manifest-path crates/tsfp-launcher/Cargo.toml` opens the
launcher: guided install from your disc image, file verification, settings and mods.
Prebuilt launchers for Linux, macOS and Windows (x64 and ARM64) are attached to releases.

Planned features: [docs/GOALS.md](docs/GOALS.md).

See [Troubleshooting](docs/TROUBLESHOOTING.md) for setup problems.
