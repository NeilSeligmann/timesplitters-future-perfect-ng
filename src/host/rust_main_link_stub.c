/* SPDX-License-Identifier: GPL-3.0-or-later
 * T2001: a C main for the tsfp_host_rustlink target only. It lets CMake/ninja produce the exact link line of
 * the Rust-hosted executable; crates/tsfp-game/build.rs drops this object and supplies the Rust main. */
int tsfp_host_entry(int argc, char **argv);

int main(int argc, char **argv)
{
    return tsfp_host_entry(argc, argv);
}
