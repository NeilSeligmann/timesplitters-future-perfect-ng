#!/bin/sh
# T2235: copy (never symlink) enabled host cdylibs beside the Bevy executable.
set -eu
[ "$#" = 2 ] || { echo "usage: $0 HOST_BUILD_DIR BINARY_DIR" >&2; exit 2; }
host_dir=$1
binary_dir=$2
cache=$host_dir/CMakeCache.txt
mkdir -p "$binary_dir"
for spec in 'FILEIO:rust-fileio:tsfp_fileio_capi' 'RT_HOST:rust-rt-host:tsfp_rt_host' 'KMEM:rust-kmem:tsfp_kmem_capi'; do
    option=${spec%%:*}
    rest=${spec#*:}
    subdir=${rest%%:*}
    library=${rest#*:}
    if grep -qx "TSFP_RUST_$option:BOOL=ON" "$cache"; then
        cargo_dir=$(sed -n "s/^TSFP_${option}_TARGET_DIR:[^=]*=//p" "$cache")
        if [ "$option" = FILEIO ] && grep -qx 'TSFP_NATIVE_FILEIO:BOOL=ON' "$cache"; then
            cargo_dir=$host_dir/rust-fileio-native
        fi
        [ -n "$cargo_dir" ] || cargo_dir=$host_dir/$subdir
        cp "$cargo_dir/release/lib$library.so" "$binary_dir/lib$library.so"
    fi
done
if grep -qx 'TSFP_NATIVE_FILEIO:BOOL=ON' "$cache"; then
    cp "$host_dir/rust-kernel-file/release/libtsfp_kernel_file_runtime.so" \
        "$binary_dir/libtsfp_kernel_file_runtime.so"
fi
if grep -Eq '^TSFP_RUST_AUDIO_(HOST|STREAM_FREQUENCY):BOOL=ON$' "$cache"; then
    cargo_dir=$(sed -n 's/^TSFP_AUDIO_CMP_TARGET_DIR:[^=]*=//p' "$cache")
    [ -n "$cargo_dir" ] || cargo_dir=$host_dir/rust-audio-cmp
    cp "$cargo_dir/release/libtsfp_audio_capi.so" "$binary_dir/libtsfp_audio_capi.so"
fi
