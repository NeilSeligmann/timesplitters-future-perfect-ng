#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# C compiler launcher for CMake (CMAKE_C_COMPILER_LAUNCHER). sccache aborts every call ("increment
# compilation is  prohibited.", even for C) while CARGO_INCREMENTAL is set to anything but 0, and the
# prebuilt build exports CARGO_INCREMENTAL=1 (T3605) into the ninja run, so a bare `sccache` launcher
# failed every C file (hud-messages gate log 2026-10-24 23:12). C never reads the variable: remove it.
unset CARGO_INCREMENTAL
exec "${TSFP_SCCACHE:-sccache}" "$@"
