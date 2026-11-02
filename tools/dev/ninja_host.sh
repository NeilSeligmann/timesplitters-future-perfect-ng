#!/bin/sh
# T3806: build the C host target with ninja and heal stale bare-rustc codegen once.
#
# Usage: sh tools/dev/ninja_host.sh BUILD_DIR JOBS TARGET
#
# The Rust ports archive (libtsfp_ports.a) and its leaf rlibs are compiled by bare rustc with
# -Cincremental, -Copt-level=3 and many codegen units (cmake/rust-ports.cmake). On 2026-10-26 the
# owner's incremental state produced an archive whose reused codegen units referenced promoted
# local symbols (`anon.<hash>.N.llvm.<id>`, `<static>.0.llvm.<id>`) that no rebuilt unit defined,
# so the host link failed with "undefined hidden symbol". A fresh build of the same commit links.
# When the failed build output carries that signature, this script removes the rustc incremental
# directories and the Rust archives of BUILD_DIR, says so, and builds once more. Any other failure,
# or the same failure after the retry, exits with ninja's status.
set -u
[ $# -eq 3 ] || { echo "usage: $0 BUILD_DIR JOBS TARGET" >&2; exit 2; }
dir=$1
jobs=$2
target=$3
log=$dir/.tsfp-ninja-host.log
rc_file=$dir/.tsfp-ninja-host.rc

run_ninja() {
    # POSIX sh has no pipefail: keep ninja's status in a file while tee shows the output live.
    { ninja -C "$dir" -j "$jobs" "$target" 2>&1; echo $? >"$rc_file"; } | tee "$log"
    rc=$(cat "$rc_file" 2>/dev/null || echo 1)
    rm -f "$rc_file"
    return "$rc"
}

stale_rust_codegen() {
    grep -Eq 'undefined (hidden )?symbol: .*\.llvm\.[0-9]+' "$log"
}

rc=0
run_ninja || rc=$?
[ "$rc" != 0 ] || exit 0
if ! stale_rust_codegen; then
    exit "$rc"
fi
echo "ninja_host.sh: link failed on undefined .llvm. symbols from stale rustc incremental state (T3806);" \
    "removing $dir/rustc-*-incremental and the Rust archives, rebuilding once" >&2
rm -rf "$dir"/rustc-*-incremental
rm -f "$dir"/libtsfp_ports.a "$dir"/*.rlib
rc=0
run_ninja || rc=$?
if [ "$rc" != 0 ] && stale_rust_codegen; then
    echo "ninja_host.sh: the link still fails on .llvm. symbols after clearing the incremental state;" \
        "run sh tools/dev/build.sh --clean and send the output to the supervisor" >&2
fi
exit "$rc"
