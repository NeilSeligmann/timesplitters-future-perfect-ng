// SPDX-License-Identifier: GPL-3.0-or-later
use tsfp_coop_net::census::{Impairment, measure};
fn main() {
    for (name, config, perturb) in [
        ("baseline", Impairment::default(), None),
        (
            "loss-jitter-duplicates",
            Impairment {
                loss_percent: 5,
                duplicate_percent: 20,
                latency_ticks: 2,
                jitter_ticks: 3,
                ..Impairment::default()
            },
            None,
        ),
        ("injected-state-drift", Impairment::default(), Some(37)),
        (
            "total-loss",
            Impairment {
                loss_percent: 100,
                ..Impairment::default()
            },
            None,
        ),
    ] {
        println!("{name}: {:?}", measure(600, config, perturb));
    }
}
