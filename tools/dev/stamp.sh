#!/bin/sh
# T2150: build-input fingerprint shared by build.sh (writes the stamp) and play.sh (checks it). Fast: git and sha256sum only.
# Prints three words (the third is the private lift key): <full git HEAD sha> <sha256 of the staged blob list of the build inputs>.
# Run from the repository root. Docs-only commits change the sha but not the fingerprint, so they do not make a binary stale.
set -eu
inputs="src crates cmake CMakeLists.txt tools/build_bevy.sh"
if [ -n "${TSFP_SOURCE_COMMIT:-}" ]; then
    # T3760: a git-less remote exec tree answers from its verified tree listing, same bytes as git; a
    # missing or inconsistent identity fails here instead of stamping "unknown".
    sha=$(python3 -m tools.wt.source_identity head)
    # shellcheck disable=SC2086
    staged=$(python3 -m tools.wt.source_identity ls-files-s $inputs && echo .)
    fp=$(printf '%s' "${staged%.}" | sha256sum | cut -d' ' -f1)
else
    sha=$(git rev-parse HEAD 2>/dev/null || echo unknown)
    # shellcheck disable=SC2086
    fp=$(git ls-files -s $inputs 2>/dev/null | sha256sum | cut -d' ' -f1)
fi
lift_key=${TSFP_STAMP_LIFT_KEY:-$(python3 -c 'from tools.private_host import owner_lift_key; print(owner_lift_key())')}
echo "$sha $fp $lift_key"
