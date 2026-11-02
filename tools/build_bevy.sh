#!/bin/sh
# T2023: rebuild the Bevy shell from the repo root, the sibling of tools/build_game.sh (T2030).
# Usage: tools/build_bevy.sh BUILD_DIR_NAME [ninja-target]
# BUILD_DIR_NAME is a directory under tmp/ configured with -DTSFP_RUST_PORTS=ON -DTSFP_RUST_MAIN=ON -DTSFP_RUST_BEVY=ON
# (docs/t-bevy-shell.md). CARGO_TARGET_DIR defaults to tmp/bevy-target. Bevy's crates come from the offline cargo home of the spike
# (TSFP_BEVY_CARGO_HOME, default tmp/spikes-cargo-home of the main checkout), not from the toolchain's own CARGO_HOME.
set -eu
[ -f tools/build_bevy.sh ] || { echo "run from the repository root" >&2; exit 2; }
[ $# -ge 1 ] || { echo "usage: $0 BUILD_DIR_NAME [ninja-target]" >&2; exit 2; }
name=$1
target=${2:-tsfp_host_bevylink}
dir=tmp/$name
[ -d "$dir" ] || { echo "no build dir $dir" >&2; exit 2; }
if [ -n "${TSFP_BEVY_CARGO_HOME:-}" ]; then
    cargo_home=$TSFP_BEVY_CARGO_HOME  # build.sh and prebuilt pass it; no git needed (T3760 exec trees)
else
    main_root=$(cd "$(git rev-parse --git-common-dir)/.." && pwd)
    legacy_home=$main_root/tmp/spikes-cargo-home
    [ -f "$main_root/tmp/rust/cargo/registry/.tsfp-t3472-merged.json" ] && legacy_home=$main_root/tmp/rust/cargo  # T3472
    cargo_home=$legacy_home
fi
[ -d "$cargo_home/registry" ] || { echo "no cargo home with the Bevy crates at $cargo_home (set TSFP_BEVY_CARGO_HOME)" >&2; exit 2; }
build_stage() { shift; "$@"; }
if [ -n "${TSFP_BUILD_STAGE_RECEIPT:-}" ]; then
    . tools/dev/build_stage.sh
fi
build_stage host ninja -C "$dir" "$target"
target_dir=${CARGO_TARGET_DIR:-$(pwd)/tmp/bevy-target}
# Cargo runs from the crate directory; resolve caller-relative paths first.
case $target_dir in
    /*) ;;
    *) target_dir=$(pwd)/$target_dir ;;
esac
# The runtime's rlib/cdylib outputs have un-hashed names. A merge verifier using
# this target from another checkout can overwrite them while Cargo retains this
# checkout's fingerprint, silently linking two frame-layer registries (T1916).
# Rebuild only that package when its dependency file names a different checkout.
if ! python3 - "$target_dir" "$(pwd)" <<'PY_CHECK'
import sys
from pathlib import Path
runtime_deps = Path(sys.argv[1]) / "release/deps/tsfp_rt_host.d"
source = str(Path(sys.argv[2]).resolve() / "crates/tsfp-rt-host/src/lib.rs")
raise SystemExit(0 if not runtime_deps.exists() or source in runtime_deps.read_text() else 1)
PY_CHECK
then
    echo "build_bevy.sh: render runtime belongs to another checkout; rebuilding its artifacts" >&2
    (cd crates/tsfp-bevy && CARGO_HOME=$cargo_home CARGO_TARGET_DIR=$target_dir cargo clean --release -p tsfp-rt-host)
fi
if command -v ld.lld >/dev/null 2>&1; then
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=clang
    export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER
    case " ${RUSTFLAGS:-} " in
        *" -C link-arg=-fuse-ld=lld "*) ;;
        *) RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C link-arg=-fuse-ld=lld"; export RUSTFLAGS ;;
    esac
fi
(cd crates/tsfp-bevy && CARGO_HOME=$cargo_home TSFP_BEVY_HOST_BUILD_DIR=$(pwd)/../../$dir CARGO_TARGET_DIR=$target_dir build_stage cargo cargo build --release --offline)
bin=$target_dir/release/tsfp-bevy
build_stage bundle sh tools/bundle_bevy_libs.sh "$dir" "$target_dir/release"
echo "binary: $bin"
ls -l "$bin"
"$bin" --bevy-info | grep 'build stamp'
