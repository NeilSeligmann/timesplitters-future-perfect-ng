/* SPDX-License-Identifier: GPL-3.0-or-later
 * T2001: the entry the Rust binary (crates/tsfp-game) calls instead of the C host's main.
 * Only built with -DTSFP_RUST_MAIN=ON (cmake/rust-main.cmake). main.c is compiled once more with
 * main renamed to tsfp_host_main, the body is untouched, so behaviour is identical to the C host. */
int tsfp_host_main(int argc, char **argv);

int tsfp_host_entry(int argc, char **argv)
{
    return tsfp_host_main(argc, argv);
}
