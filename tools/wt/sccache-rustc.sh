#!/bin/bash
# Cargo exports binary paths as CARGO_BIN_EXE_<name>, including hyphens. dash drops
# these valid environment entries at startup; Bash preserves them for rustc.
# I-T3505-15 (2026-10-23): the container starts every shell at nice -4, and the sccache daemon this
# wrapper autostarts inherits it, so its rustc jobs outranked the owner's apps (load 34, 10 servers
# at -4). Lower this call, its rustc and any daemon it starts to nice 15 and idle I/O.
# TSFP_SCCACHE_NICE sets the floor (0 turns it off); a process that is already nicer keeps its value.
nice_floor=${TSFP_SCCACHE_NICE:-15}
current_nice=$(nice)
if [ "$nice_floor" -gt 0 ] && [ "$current_nice" -lt "$nice_floor" ]; then
    renice -n "$nice_floor" -p $$ >/dev/null 2>&1
    ionice -c3 -p $$ >/dev/null 2>&1
fi
# T3411: RUSTC_WRAPPER for tools.wt. sccache hashes every CARGO_* variable, and the
# per-worker CARGO_TARGET_DIR would make each target dir its own cache namespace. rustc
# never reads it, so drop it before the cache key is computed.
unset CARGO_TARGET_DIR CARGO_BUILD_TARGET_DIR
# T3472 (MEASURED): CARGO_BUILD_JOBS differs per host (droplet 6, Deck 4, ...) and split the
# shared store (laptop JOBS=8 then Deck JOBS=4: 0/332 hits). rustc reads none of these.
unset CARGO_BUILD_JOBS CARGO_TERM_COLOR CARGO_TERM_VERBOSE CARGO_TERM_QUIET \
    CARGO_TERM_PROGRESS_WHEN CARGO_TERM_PROGRESS_WIDTH CARGO_NET_OFFLINE
# T3472 phase 2: remote hosts hold their slot/source locks on fds 7-9; this wrapper may start the
# long-lived sccache daemon, which would inherit them and keep the host slot until it idles out.
if [ -n "${TSFP_CLOSE_LOCK_FDS:-}" ]; then exec 7>&- 8>&- 9>&-; fi
# T3472: the shared WebDAV store's password comes from a host-local file, read here so it
# never sits in an env script, a hook command line or a log (tools/wt/sccache_shared.py).
if [ -n "${SCCACHE_WEBDAV_ENDPOINT:-}" ] && [ -n "${TSFP_SCCACHE_PASS_FILE:-}" ]; then
    SCCACHE_WEBDAV_PASSWORD=$(cat "$TSFP_SCCACHE_PASS_FILE") && export SCCACHE_WEBDAV_PASSWORD
fi
# T3605: the tsfp-ports crate is one very large unit whose type check, borrow check and monomorphisation are serial in
# rustc (about 48 s of an 85 s cold build on 16 idle cores). The parallel front end (-Zthreads, nightly-only) cuts
# that build to 55 s and produces a byte-identical archive (MEASURED, docs/t3605-build-speed.md). The pinned stable
# compiler takes it with RUSTC_BOOTSTRAP=1, set here for this one compiler call only so no build script or
# registry crate ever sees it. Local work uses two threads; remote builders use eight.
# TSFP_RUST_PORTS_THREADS=0 or 1 turns it off.
case " $* " in
*" --crate-name tsfp_ports "*)
    default_threads=8
    case ${TSFP_BUILD_LOCAL:-0} in 1) default_threads=2 ;; esac
    case ${TSFP_RUST_PORTS_THREADS:-$default_threads} in
    0 | 1) ;;
    *)
        ports_rustc=$1
        shift
        set -- "$ports_rustc" "-Zthreads=${TSFP_RUST_PORTS_THREADS:-$default_threads}" "$@"
        RUSTC_BOOTSTRAP=1
        export RUSTC_BOOTSTRAP
        ;;
    esac
    ;;
esac
# T3469/T3472: sccache refuses incremental rustc (exit 101), but cargo makes only the local
# crates incremental; run those directly so registry crates stay cached under CARGO_INCREMENTAL=1.
for arg in "$@"; do
    case $arg in
    incremental=* | -Cincremental=*) exec "$@" ;;
    esac
done
# sccache itself aborts every call ("increment compilation is prohibited", even `rustc -vV`)
# while CARGO_INCREMENTAL is set to anything but 0, which broke every tools.wt dev run that
# defaults to CARGO_INCREMENTAL=1 (T2407v/T2408v/T2409v). The cached units are the
# non-incremental ones anyway, so sccache sees 0, the same value verify and remote builds use
# (so cross-host keys stay equal).
case ${CARGO_INCREMENTAL:-0} in 0) ;; *) CARGO_INCREMENTAL=0 && export CARGO_INCREMENTAL ;; esac
# A shared-store outage (WebDAV resets, server migration) must not fail the build: when sccache
# itself errors, compile once more without it (2026-10-21: T2122m merge verify failed on resets).
err=$(mktemp "${TMPDIR:-/tmp}/sccache-rustc.XXXXXX")
"${TSFP_SCCACHE:-sccache}" "$@" 2>"$err"
rc=$?
# Cache-control calls have no compiler to fall back to. Preserve their real error
# instead of trying to exec --show-stats/--start-server and hiding it behind 127.
case ${1:-} in
--*)
    cat "$err" >&2
    rm -f "$err"
    exit "$rc"
    ;;
esac
if [ "$rc" -ne 0 ] && grep -qiE '^sccache: (error|caused by)|failed to (connect|read|write) .*(webdav|cache)|connection (reset|refused)' "$err"; then
    echo "sccache-rustc: sccache failed (rc=$rc), compiling without the cache" >&2
    rm -f "$err"
    exec "$@"
fi
cat "$err" >&2
rm -f "$err"
exit "$rc"
