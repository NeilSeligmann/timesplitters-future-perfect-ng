# Building from source

## Launcher

Needs a recent stable [Rust](https://rustup.rs/) toolchain. On Linux, also `libxkbcommon-dev`, `libwayland-dev` and
`libfontconfig1-dev`.

```sh
cargo run --release --manifest-path crates/tsfp-launcher/Cargo.toml   # build and run
cargo test --manifest-path crates/tsfp-launcher/Cargo.toml            # launcher tests
cargo test -p tsfp-launcher-core                                      # core logic tests
```

The launcher UI is written in [Slint](https://slint.dev/) (`crates/tsfp-launcher/ui/launcher.slint`, Fluent dark
style) over a toolkit-independent controller and `crates/tsfp-launcher-core`, which holds disc
identification, install, verify, settings, mods, packs and update selection. Intel Mac users build the launcher this
way; it runs natively but does not auto-update.

## Game (recompiled host)

The recompiled game needs your own disc and a C/C++ toolchain:

1. Install Python 3, CMake, Ninja, a C/C++ compiler, pkg-config and Rust stable.
2. Copy your disc image to `discs/timesplitters-future.iso` (or set `TSFP_DISC`).
3. Run `./run.sh`: it extracts `default.xbe` from your disc, lifts it locally, builds the host and starts the game.

Nothing derived from your disc leaves your machine or belongs in a commit.

## Releases

Pushing a `vX.Y.Z` tag (matching `crates/tsfp-launcher/Cargo.toml`) builds the launcher for Linux x64/arm64,
macOS Apple Silicon and Windows x64/arm64, adds AppImage and `.dmg` installers, `SHA256SUMS` and minisign
signatures, and publishes a GitHub Release. See `.github/workflows/launcher.yml`.

### When a platform is missing from a release

Each platform publishes on its own, so a release can go out without one of them; GitHub's macOS arm64 runners in
particular sometimes cannot start ("capacity constraints", "not acquired by Runner"). To add the missing platform
later, open the tag's run under **Actions** and choose **Re-run failed jobs**. Only the missing platform builds; it
attaches its files to the same release, and the `checksums` job runs again so `SHA256SUMS` includes them.

macOS releases are built on GitHub's Mac runners only: building them on Linux would need Apple's SDK, which Apple
licenses for Apple hardware only.
