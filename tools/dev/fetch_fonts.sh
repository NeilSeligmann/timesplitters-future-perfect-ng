#!/bin/sh
# T2307: fetch the open (SIL OFL) outline fonts of the Rust UI `improved` preset at a pinned
# google/fonts commit into a gitignored directory (default tmp/ui-fonts), checksum-verified.
# Never committed (AGENTS.md rule 5). Impact and Arial themselves are proprietary (Monotype /
# Microsoft) and are not fetched: put your own impact.ttf / arialbold.ttf in TSFP_UI_FONT_DIR.
#
# usage: sh tools/dev/fetch_fonts.sh [DIR]
# Liberation Sans Bold (Arial Bold stand-in) is copied from the system when installed
# (apt fonts-liberation), else fetched from the pinned upstream release.
set -eu
root=$(cd "$(dirname "$0")/../.." && pwd)
dir=${1:-$root/tmp/ui-fonts}
mkdir -p "$dir"

GF=66928797f271fed5ee428544e7229685e2d90127
GF_URL=https://raw.githubusercontent.com/google/fonts/$GF
LIB_VER=2.1.5
LIB_URL=https://github.com/liberationfonts/liberation-fonts/files/7261482/liberation-fonts-ttf-$LIB_VER.tar.gz

# name|url|sha256
fetch() {
    name=$1 url=$2 sum=$3
    if [ -f "$dir/$name" ] && echo "$sum  $dir/$name" | sha256sum -c - >/dev/null 2>&1; then
        return 0
    fi
    timeout 120 curl -sfL -o "$dir/$name.part" "$url" || { echo "fetch_fonts.sh: download failed: $url" >&2; rm -f "$dir/$name.part"; return 1; }
    if ! echo "$sum  $dir/$name.part" | sha256sum -c - >/dev/null 2>&1; then
        echo "fetch_fonts.sh: checksum mismatch for $name, discarded" >&2
        rm -f "$dir/$name.part"
        return 1
    fi
    mv "$dir/$name.part" "$dir/$name"
}

status=0
# Impact look-alike (chosen by comparison with the decoded original atlas, docs/t2307-ui-visual-fidelity.md).
fetch Anton-Regular.ttf "$GF_URL/ofl/anton/Anton-Regular.ttf" a4ba3a92350ebb031da0cb47630ac49eb265082ca1bc0450442f4a83ab947cab || status=1
fetch Anton-OFL.txt "$GF_URL/ofl/anton/OFL.txt" ee67e6ee22790b7929f1a3769ca2801d565c64b5a9096942c1adf5596de9c9e4 || status=1

# Arial Bold metric-compatible stand-in.
sys=/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf
if [ ! -f "$dir/LiberationSans-Bold.ttf" ]; then
    if [ -f "$sys" ]; then
        cp "$sys" "$dir/LiberationSans-Bold.ttf"
    elif fetch liberation.tar.gz "$LIB_URL" 7191c669bf38899f73a2094ed00f7b800553364f90e2637010a69c0e268f25d0; then
        timeout 60 tar -xzf "$dir/liberation.tar.gz" -C "$dir" --strip-components=1 "liberation-fonts-ttf-$LIB_VER/LiberationSans-Bold.ttf" "liberation-fonts-ttf-$LIB_VER/LICENSE" || status=1
        [ ! -f "$dir/LICENSE" ] || mv "$dir/LICENSE" "$dir/Liberation-OFL.txt"
        rm -f "$dir/liberation.tar.gz"
    else
        status=1
    fi
fi

# Outline font of each original atlas (the names tsfp-ui-wgpu looks for).
[ ! -f "$dir/Anton-Regular.ttf" ] || cp "$dir/Anton-Regular.ttf" "$dir/impact-openalt.ttf"
[ ! -f "$dir/LiberationSans-Bold.ttf" ] || cp "$dir/LiberationSans-Bold.ttf" "$dir/arialbold-openalt.ttf"
echo "fetch_fonts.sh: open fonts in $dir (status $status)"
exit $status
