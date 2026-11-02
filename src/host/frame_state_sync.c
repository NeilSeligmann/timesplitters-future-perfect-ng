/* SPDX-License-Identifier: GPL-3.0-or-later
 * T3879 TEMPORARY-SHIM (removal: route the raw lifted accessors of the owned frame words through the typed owner, see
 * docs/t-core-timers-gate.md "Raw lifted writers"). Raw lifted C reads and writes guest memory directly, so it never sees
 * the Rust-owned frame counters and its writes (the session-start reset of [0x7497B0] and [0x7DE318]) never reach them.
 * This keeps both views equal at every Rust <-> lifted call boundary: `sync_in` adopts a guest word that changed since the
 * last boundary into the Rust owner, `sync_out` publishes the Rust value back to guest memory. */
#include "../game/game_guest.h"
#include "frame_state_sync.h"

/* Mirrors the frame_state.rs ADDRESSES of FrameCounters, FramePauseState, FrameStepOutputs, FrameClockState and FrameTimingHistory (crates/tsfp-engine-core/src/frame_state.rs). */
static const guest_addr owned_words[] = {
    0x007497B0u, 0x007497B8u, 0x007497BCu, 0x007DE318u, 0x007497B4u,
    0x004E7940u, 0x004E7944u, 0x004E7948u, 0x004E794Cu,
    /* FrameClockState */
    0x007497A8u, 0x007497ACu, 0x006B7AF0u, 0x006B7AD4u,
    /* FrameTimingHistory */
    0x007A58A0u, 0x007A58A4u, 0x007A58A8u, 0x007A58ACu, 0x007A58B0u, 0x007A58C0u, 0x007A58C4u, 0x007A58C8u,
    0x007A58CCu, 0x007A58D0u, 0x007A58E0u, 0x007A58E4u, 0x007A58E8u, 0x007A58ECu, 0x007A58F0u,
};
#define OWNED_WORD_COUNT (sizeof owned_words / sizeof owned_words[0])

static uint32_t shadow[OWNED_WORD_COUNT];
static int primed;

static int owner_selected(void)
{
    uint32_t probe;
    return game_owned_frame_word_access(owned_words[0], 4u, 0u, &probe);
}

static uint32_t raw_read(guest_addr address)
{
    uint32_t value;
    memcpy(&value, game_host_ptr(address), sizeof value);
    return value;
}

void tsfp_frame_state_sync_in(void)
{
    if (!owner_selected()) return;
    for (size_t i = 0; i < OWNED_WORD_COUNT; ++i) {
        const uint32_t guest = raw_read(owned_words[i]);
        if (primed && guest != shadow[i]) {
            uint32_t value = guest;
            (void)game_owned_frame_word_access(owned_words[i], 4u, 1u, &value);
        }
        shadow[i] = guest;
    }
    primed = 1;
}

void tsfp_frame_state_sync_out(void)
{
    if (!owner_selected()) return;
    for (size_t i = 0; i < OWNED_WORD_COUNT; ++i) {
        uint32_t value;
        if (!game_owned_frame_word_access(owned_words[i], 4u, 0u, &value)) continue;
        memcpy(game_host_ptr(owned_words[i]), &value, sizeof value);
        shadow[i] = value;
    }
    primed = 1;
}
