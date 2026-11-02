#!/bin/sh
# T2030: rebuild the Rust-main game from the repo root. Usage: tools/build_game.sh BUILD_DIR_NAME [tsfp_host|tsfp_host_rustlink]
# BUILD_DIR_NAME is a directory under tmp/ (for example hybrid-build-host). CARGO_TARGET_DIR defaults to tmp/game-target-host.
set -eu
[ -f tools/build_game.sh ] || { echo "run from the repository root" >&2; exit 2; }
[ $# -ge 1 ] || { echo "usage: $0 BUILD_DIR_NAME [ninja-target]" >&2; exit 2; }
name=$1
target=${2:-tsfp_host_rustlink}
dir=tmp/$name
[ -d "$dir" ] || { echo "no build dir $dir" >&2; exit 2; }
ninja -C "$dir" "$target"
target_dir=${CARGO_TARGET_DIR:-../../tmp/game-target-host}
(cd crates/tsfp-game && TSFP_GAME_HOST_BUILD_DIR=$dir CARGO_TARGET_DIR=$target_dir cargo build --release --offline)
bin=$(cd crates/tsfp-game && cd "$target_dir/release" && pwd)/tsfp-game
echo "binary: $bin"
ls -l "$bin"
"$bin" --rust-main-info | grep 'build stamp'
