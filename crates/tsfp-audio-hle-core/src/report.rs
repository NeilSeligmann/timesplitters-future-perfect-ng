// SPDX-License-Identifier: GPL-3.0-or-later
//! `dsound_hle_report`: the end-of-run account of what the HLE did and did not implement.
use crate::log;
use crate::readiness::{AC97_PRIMARY_CODEC_READY, DSP_ACK_UNSET, NULL_DEVICE_FAULT_VA, STATE};
use crate::table::{ENTRY_IMPLEMENTED, FUNCTION_COUNT, SITE_COUNT, calls, label, snapshot};

/// `dsound_hle_report`: missing functions ranked by observed calls, then measured sites, then address.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_report() {
    let entries = snapshot();
    let missing: Vec<usize> = (0..FUNCTION_COUNT)
        .filter(|&i| entries[i].state != ENTRY_IMPLEMENTED)
        .collect();
    let observed: u64 = entries.iter().map(calls).sum();
    log(&format!(
        "dsound: {} of {FUNCTION_COUNT} measured functions still need implementations ({SITE_COUNT} call sites \
         measured, {observed} calls observed this run)\n",
        missing.len()
    ));
    log(
        "dsound: output is RENDER-TO-NOTHING -- no host audio device is opened, no samples are mixed and none are emitted\n",
    );
    let (status, queries, base, offset, address, writes, refusals) = {
        let s = crate::lock(&STATE);
        (
            s.ac97_status,
            s.not_ready_queries,
            s.ack_base,
            s.ack_offset,
            s.ack_address,
            s.ack_writes,
            s.ack_refusals,
        )
    };
    if status & AC97_PRIMARY_CODEC_READY != 0 {
        log(
            "dsound: codec reports READY (FABRICATED -- there is no AC97 codec; DirectSoundCreate is being allowed to succeed)\n",
        );
    } else {
        log(&format!(
            "dsound: codec reports NOT READY (honest default; {queries} readiness quer{} answered no) -- expect \
             DirectSoundCreate to fail; upstream titles then fault at guest {NULL_DEVICE_FAULT_VA:#010x}, but that \
             address is AMBIGUOUS (on this title it was a missing fs:[4], not audio), so a fault there is not by \
             itself evidence of this cause\n",
            if queries == 1 { "y" } else { "ies" }
        ));
    }
    if address == DSP_ACK_UNSET {
        log(&format!(
            "dsound: DSP command-word ack NOT CONFIGURED -- the scratch offset for this title has not been derived \
             from the binary ({refusals} acknowledgement{} refused)\n",
            if refusals == 1 { "" } else { "s" }
        ));
    } else {
        log(&format!(
            "dsound: DSP command-word ack at {address:#010x} (base {base:#010x} + {offset:#x}, DERIVED) -- {writes} acknowledged\n"
        ));
    }
    let unknown = crate::dsound_hle_unknown_call_count();
    if unknown > 0 {
        log(&format!(
            "dsound: {unknown} call{} landed on an address absent from the surface table -- the table and the binary DISAGREE\n",
            if unknown == 1 { "" } else { "s" }
        ));
    }
    let mut ranked = missing;
    ranked.sort_by(|&a, &b| {
        calls(&entries[b])
            .cmp(&calls(&entries[a]))
            .then(entries[b].sites.cmp(&entries[a].sites))
            .then(entries[a].address.cmp(&entries[b].address))
    });
    for index in ranked {
        let entry = &entries[index];
        log(&format!(
            "  {:#010x}  {:<40} sites {:2}  calls {}\n",
            entry.address,
            label(entry),
            entry.sites,
            calls(entry)
        ));
    }
}
