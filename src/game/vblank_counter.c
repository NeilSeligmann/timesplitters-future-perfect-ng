/* SPDX-License-Identifier: GPL-3.0-or-later */
#include <stdbool.h>
#include "game_replace.h"
#define VBLANK_COUNTER 0x00563918u
/* T3879: with the native frame-state port selected the counter lives in Rust, not at VBLANK_COUNTER (the
 * delivery seam never writes the guest word), so every reader and incrementer must go through the owner. */
extern bool tsfp_frame_state_native_selected(void) __attribute__((weak));
extern unsigned tsfp_frame_state_vblank_counter_read(void) __attribute__((weak));
extern unsigned tsfp_frame_state_record_vblank_delivery(void) __attribute__((weak));
static bool counter_is_native(void)
{
    return tsfp_frame_state_native_selected != NULL && tsfp_frame_state_vblank_counter_read != NULL &&
           tsfp_frame_state_record_vblank_delivery != NULL && tsfp_frame_state_native_selected();
}
/* Executes only when invoked; callback delivery/timing is the caller's concern.
 * Original ignores its callback argument and preserves all integer registers.
 * Unsigned addition reproduces INC modulo32. EFLAGS is outside replacement ABI. */
GAME_REPLACE_EXACT(00022020, cdecl, 1, void, game_vblank_counter_increment)
{
    if (counter_is_native()) { (void)tsfp_frame_state_record_vblank_delivery(); return; }
    guest_write32(VBLANK_COUNTER,guest_read32(VBLANK_COUNTER)+1u);
}
GAME_REPLACE_EXACT(00022030, cdecl, 0, u32, game_vblank_counter_read)
{
    g_eax=counter_is_native()?tsfp_frame_state_vblank_counter_read():guest_read32(VBLANK_COUNTER);
}
