/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_HOST_RUST_FILEIO_H
#define TSFP_HOST_RUST_FILEIO_H
#include <stdbool.h>
/* T2056: install the Rust disc reader (crates/tsfp-fileio-capi) behind xdvdfs_*, and/or open the disc call log.
 * Build-enabled hosts default to Rust; the retired C implementation is unavailable.
 * Returns false in a build without TSFP_RUST_FILEIO or when the log cannot be opened. Call before mounting. */
bool host_rust_fileio_configure(bool enable, const char *log_path);
#endif
