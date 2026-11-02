#!/bin/sh
set -eu
cd "$(dirname "$0")"
disc=${TSFP_DISC:-$PWD/discs/timesplitters-future.iso}
if [ ! -f "$disc" ]; then
  echo "Disc image not found: $disc" >&2
  echo 'Copy your disc image into discs/ or set TSFP_DISC.' >&2
  exit 2
fi
if ! command -v cmake >/dev/null 2>&1 || ! command -v ninja >/dev/null 2>&1 ||
   ! command -v cc >/dev/null 2>&1 || ! command -v c++ >/dev/null 2>&1 ||
   ! command -v python3 >/dev/null 2>&1; then
  if command -v apt-get >/dev/null 2>&1 && command -v sudo >/dev/null 2>&1; then
    sudo apt-get update
    sudo apt-get install -y python3 python3-venv cmake ninja-build \
      build-essential pkg-config
  else
    echo 'Install Python 3, CMake, Ninja, C/C++ and pkg-config.' >&2
    exit 2
  fi
fi
sh tools/dev/build.sh --check-toolchain
if ! python3 -m venv .venv; then
  if command -v apt-get >/dev/null 2>&1 && command -v sudo >/dev/null 2>&1; then
    sudo apt-get update
    sudo apt-get install -y python3-venv
    python3 -m venv --clear .venv
  else
    echo 'Python venv support is missing; install the python3-venv package.' >&2
    exit 2
  fi
fi
.venv/bin/pip install --disable-pip-version-check capstone
export PATH="$PWD/.venv/bin:$PATH"
mkdir -p tmp
ln -sf "$(realpath "$disc")" 'tmp/timesplitters-future (USA) (XBOX).iso'
export TSFP_DISC="$disc"
export CARGO_HOME="$PWD/tmp/release-cargo-home"
mkdir -p "$CARGO_HOME"
cargo fetch --locked
export TSFP_BEVY_CARGO_HOME="$CARGO_HOME"
mkdir -p build
python3 -m tools.xdvdfs.cli "$disc" --extract default.xbe --dest build
mkdir -p tmp/analysis_scratch
python3 -c 'import urllib.request; urllib.request.urlretrieve("https://github.com/XboxDev/xtlid/releases/download/v0.1.2/xtlid.xml", "tmp/analysis_scratch/xtlid.xml")'
python3 -m tools.private_host --xbe build/default.xbe --force-lift --lift-only build
sh tools/dev/build.sh
sh tools/dev/play.sh default
