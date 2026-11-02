//! T1871: which engine addresses the ported effect helpers answer. A host calls [`native`] before treating an address as an unported
//! engine routine; the argument convention is `Guest::call`: stack arguments in push order (leftmost first), then the register arguments of
//! usercall routines in the order given below.

use super::eff_flare::flare_update;
use super::eff_guest::Guest;
use super::eff_helpers::*;
use super::eff_init::*;
use super::eff_update::*;
use super::eff_update2::*;
use super::eff_update3::*;

/// The addresses [`native`] implements.
pub const NATIVE: [u32; 59] = [
    0xE29D0, 0xE40E0, 0xE4EA0, 0xE6B30, 0xE3B00, 0xED510, 0xE3270, 0xF1A90, 0xEAD10, 0xE2ED0,
    0x389B0, 0xE0A80, 0xE0B50, 0xE0BD0, 0xE0C50, 0xE14B0, 0xE1620, 0xE1650, 0xE28D0, 0xE2DF0,
    0xE2F90, 0xE34D0, 0xE3570, 0xE3640, 0xE36D0, 0xE3AB0, 0xE4690, 0xE78F0, 0xECAC0, 0xE3680,
    0xECB90, 0xF51F0, 0xE4800, 0xEC120, 0xEBB00, 0xF61E0, 0xF5920, 0xF5D30, 0xF5FF0, 0xF4920,
    0xF3950, 0xDCB80, 0xDCEF0, 0xDC5D0, 0xE94E0, 0xE9690, 0xE97A0, 0xE9FE0, 0xF4A50, 0xF08D0,
    0xF2B80, 0xF2910, 0xF2AB0, 0xF39A0, 0xB91F0, 0xB9220, 0xBD0F0, 0xB9920, 0xB9960,
];

/// The class init callbacks (`+0x30`, T1876): the 19 functions of T1870 section 2 (`0x389B0` is a bare `ret`).
pub const INIT_CALLBACKS: [u32; 19] = [
    0x389B0, 0xE0A80, 0xE0B50, 0xE0BD0, 0xE0C50, 0xE14B0, 0xE1620, 0xE1650, 0xE28D0, 0xE2DF0,
    0xE2F90, 0xE34D0, 0xE3570, 0xE3640, 0xE36D0, 0xE3AB0, 0xE4690, 0xE78F0, 0xECAC0,
];

/// The class update callbacks (`+0x34`) the host runs through [`native`] (T1871: the first four, T1876: the rest).
pub const UPDATE_CALLBACKS: [u32; 14] = [
    0xF1A90, 0xEAD10, 0xF4A50, 0xF08D0, 0xF2B80, 0xF39A0, 0xF5FF0, 0xF5920, 0xF61E0, 0xEBB00,
    0xEC120, 0xE4800, 0xF51F0, 0xECB90,
];

/// The late callbacks whose render commands are ported (renderer leaves remain host calls).
pub const LATE_CALLBACKS: [u32; 6] = [0xE29D0, 0xE3270, 0xE40E0, 0xE4EA0, 0xE6B30, 0xED510];
pub const DESTROY_CALLBACKS: [u32; 1] = [0xE3B00];

/// Run a ported routine. `None` = not ported (the host's engine answers it).
///
/// * `0xDCB80`: stack `a1 a2 a3 a4`, then esi = entry.
/// * `0xE97A0`: stack `s1 s2 s3`, then eax = entry, ebx = hit.
/// * `0xF2AB0`: stack `hit normal`, then esi = entry, edi = struck object. `0xF2910`: eax = entry.
pub fn native(g: &mut dyn Guest, va: u32, args: &[u32]) -> Option<u32> {
    match va {
        0xE29D0 => super::eff_late::orbit_draw(g, args[0]),
        0xE40E0 => super::eff_late::flame_beam_draw(g, args[0]),
        0xE4EA0 => super::eff_late::ring_beam_draw(g, args[0]),
        0xE6B30 => super::eff_late::helix_beam_draw(g, args[0]),
        0xE3B00 => super::eff_late::beam_destroy(g, args[0]),
        0xED510 => super::eff_late::trail_draw(g, args[0]),
        0xE3270 => super::eff_late::flare_draw(g, args[0]),
        0xF4920 => ground_effect(g, args[0], f32::from_bits(args[1]), args[2]),
        0xF3950 => release_hit(g, args[0], args[1]),
        0xDCB80 => return Some(source_ray(g, args[4], args[0], args[1], args[2], args[3])),
        0xDCEF0 => start_light(g, args[0], args[1]),
        0xDC5D0 => attached_channels(g, args[0]),
        0xE94E0 => mark_players(g, args[0], args[1], args[2], f32::from_bits(args[3])),
        0xE9690 => refresh_contact(g, args[0]),
        0xE97A0 => return Some(sweep_move(g, args[3], args[4], args[0], args[1], args[2])),
        0xE9FE0 => hit_reaction(
            g, args[0], args[1], args[2], args[3], args[4], args[5], args[6],
        ),
        0xF4A50 => projectile_update(g, args[0]),
        0xF08D0 => particle_update(g, args[0]),
        0xF2B80 => debris_update(g, args[0]),
        0xF39A0 => stuck_update(g, args[0]),
        0xB91F0 => record_init(g, args[0]),
        0xB9220 => record_fill(g, args[0], args[1]),
        0xBD0F0 => resolve_hit_amount(g, args[0]),
        0xB9920 => return Some(strength_a(g, args[0], args[1], args[2]).to_bits()),
        0xB9960 => return Some(strength_b(g, args[0], args[1], args[2]).to_bits()),
        0xF2910 => impact_death(g, args[0]),
        0x389B0 => init_nothing(g, args[0]),
        0xE0A80 => init_arrow(g, args[0]),
        0xE0B50 => init_owner_70b(g, args[0]),
        0xE0BD0 => init_owner_70e(g, args[0]),
        0xE0C50 => init_owner_711(g, args[0]),
        0xE14B0 => init_sound_quad(g, args[0]),
        0xE1620 => init_sound_single(g, args[0]),
        0xE1650 => init_trail_sounds(g, args[0]),
        0xE28D0 => init_burst(g, args[0]),
        0xE2DF0 => init_part(g, args[0]),
        0xE2F90 => init_shell(g, args[0]),
        0xE34D0 => init_debris(g, args[0]),
        0xE3570 => init_orb(g, args[0]),
        0xE3640 => init_bolt(g, args[0]),
        0xE36D0 => init_beam(g, args[0]),
        0xE3AB0 => init_jet(g, args[0]),
        0xE4690 => init_trace(g, args[0]),
        0xE78F0 => init_flash(g, args[0]),
        0xECAC0 => init_drone(g, args[0]),
        0xF1A90 => flare_update(g, args[0]),
        0x350E70 => return Some(block_flag(g, args[0], args[1])),
        0xE3170 => return Some(u32::from(flare_hit(g, args[2], args[3], args[0], args[1]))),
        0xEAD10 => part_update(g, args[0]),
        0xE2ED0 => return Some(u32::from(part_expire(g, args[1], args[0]))),
        0xF5D30 => beam_trace(g, args[0]),
        0xE3680 => return Some(u32::from(beam_viewer_ready(g, args[0]))),
        0xECB90 => drone_update(g, args[0]),
        0xF51F0 => bounce_update(g, args[0]),
        0xE4800 => trace_update(g, args[0]),
        0xEC120 => spark_update(g, args[0]),
        0xEBB00 => flame_update(g, args[0]),
        0xF61E0 => splash_update(g, args[0]),
        0xF5920 => bolt_update(g, args[0]),
        0xF5FF0 => beam_update(g, args[0]),
        0xF2AB0 => impact_effect(g, args[2], args[3], args[0], args[1]),
        _ => return None,
    }
    Some(0)
}
