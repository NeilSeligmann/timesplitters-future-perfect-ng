/* T3702: the bytes a call-args pointer digest covers. A 64-byte window that STRADDLES the call-time ESP
 * inside the calling thread's stack (pointer < esp < pointer + length) is digested over its live part
 * [esp, pointer + length) only: the bytes below ESP are dead stack, the callee's own scratch, and differ
 * between a lifted and a Rust callee without any behaviour change (claude-sup's ui-framework and missions
 * re-gates, 2026-10-25). Windows wholly below ESP stay whole: tools.rust_trace_call_args excuses them as
 * stack residue. Unknown stack ranges (low == high) never trim. */
#ifndef TSFP_CALL_ARGS_WINDOW_H
#define TSFP_CALL_ARGS_WINDOW_H
#include <stddef.h>
#include <stdint.h>

static inline void call_args_live_window(uint32_t pointer, size_t length, uint32_t esp, uint32_t low,
                                         uint32_t high, uint32_t *start, size_t *size)
{
    *start = pointer, *size = length;
    if (esp == 0u || low >= high || esp < low || esp > high) return;
    if (pointer >= low && pointer < esp && (uint64_t)pointer + length > esp) {
        *start = esp;
        *size = (size_t)((uint64_t)pointer + length - esp);
    }
}
#endif
