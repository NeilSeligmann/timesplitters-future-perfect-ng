// SPDX-License-Identifier: GPL-3.0-or-later
// T2136: tsfp-wgpu pulls tsfp-ports (std feature) in as an rlib. rustc exports every upstream #[no_mangle] symbol from a cdylib:
// 835 tsfp_lane_* / tsfp_port_* names (measured with nm -D) that also exist in the host's libtsfp_ports.a. A second version
// script cannot remove them (GNU ld merges the global lists, tried and measured), so bind every reference inside this shared
// object to its own definition (-Bsymbolic): the host's copies never interpose into the runtime, and the runtime never calls
// into the host's copies.
fn main() {
    println!("cargo:rustc-cdylib-link-arg=-Wl,-Bsymbolic");
}
