/* SPDX-License-Identifier: GPL-3.0-or-later
 * T2309 UI-11: per-page switch for the original page draw.
 *
 * The original front end draws a page and its widget tree through 0x76810 (cdecl
 * `(widget, pass, depth)`, recursive over the children at widget + 0x16C). When the Rust UI draws
 * a page inside the engine frame (crates/tsfp-ui-wgpu/src/in_frame.rs), the original draw of that
 * page must not run as well (double drawing, a darker fill). This strong `sub_00076810` asks the
 * shell per widget through the weak `tsfp_ui_page_draw_owned(widget, builder, fade_bits)` (builder = the
 * page slot's builder at widget + 8, fade_bits = the page fade f32 at widget + 0xF4, T2313) and skips the original draw, children included, of a page the Rust
 * UI owns. Nothing is written to guest memory: the page keeps its state, input and logic, only its
 * picture comes from Rust. Without the shell (standalone C host) or for any other widget the
 * retained lifted original runs unchanged (tools/play/ui_draw_hooks.py preserves it).
 * TSFP_UI_DRAW_LOG=1 prints the first distinct widgets drawn and the switch decision. */
#include "game_replace.h"
#include "recomp_guest_call.h"
#include "kernel_call.h"
#include "rust_ext_bridge.h"
#include "host_runtime.h"
#include <stdio.h>
#include <stdlib.h>

extern int tsfp_ui_page_draw_owned(uint32_t widget, uint32_t builder, uint32_t fade_bits) __attribute__((weak));
extern void tsfp_uihook_original_00076810(void);
extern void tsfp_uihook_original_00081c60(void);
extern int tsfp_ui_editor_cursor_icon_owned(uint32_t return_pc) __attribute__((weak));

/* T2328: native OS cursor replaces only the editor's cursor icons. The Rust
 * predicate owns device policy; this adapter only forwards the cdecl boundary.
 * The original hint draw and all other icon callers remain independent. */
void sub_00081C60(void);
void sub_00081C60(void)
{
    if (tsfp_ui_editor_cursor_icon_owned != NULL
        && tsfp_ui_editor_cursor_icon_owned(guest_read32(g_esp))) {
        g_esp += 4u;
        return;
    }
    tsfp_uihook_original_00081c60();
}

static int ui_draw_log = -1;
static uint32_t ui_draw_logged[16];
static unsigned ui_draw_logged_count;
static uint64_t ui_draw_skipped;

static void ui_draw_note(uint32_t widget, uint32_t builder, int owned)
{
    if (ui_draw_log < 0) {
        const char *value = getenv("TSFP_UI_DRAW_LOG");
        ui_draw_log = value != NULL && value[0] == '1';
    }
    if (!ui_draw_log) return;
    for (unsigned i = 0; i < ui_draw_logged_count; ++i)
        if (ui_draw_logged[i] == (widget ^ (uint32_t)owned)) return;
    if (ui_draw_logged_count >= sizeof ui_draw_logged / sizeof ui_draw_logged[0]) return;
    ui_draw_logged[ui_draw_logged_count++] = widget ^ (uint32_t)owned;
    fprintf(stderr, "ui-draw 0x76810 widget=0x%08x builder=0x%08x pass=%u depth=%u -> %s (skipped so far %llu)\n",
            widget, builder, guest_read32(g_esp + 8u), guest_read32(g_esp + 12u), owned == 2 ? "rust callback; original framework" : owned ? "rust" : "original",
            (unsigned long long)ui_draw_skipped);
}

void sub_00076810(void);
void sub_00076810(void)
{
    if (tsfp_ui_page_draw_owned != NULL) {
        const uint32_t widget = game_stack_arg(0);
        const uint32_t builder = widget != 0u ? guest_read32(widget + 8u) : 0u;
        const int owned = widget != 0u ? tsfp_ui_page_draw_owned(widget, builder, guest_read32(widget + 0xF4u)) : 0;
        ui_draw_note(widget, builder, owned);
        if (owned == 1) {
            ++ui_draw_skipped;
            g_esp += 4u; /* cdecl: the `ret` pops the return address only */
            return;
        }
    }
    tsfp_uihook_original_00076810();
}

/* T2319: the shell owns device availability. This boundary only marshals the
 * original callback ABI on the stack; it never writes device records or menu state. */
extern int tsfp_storage_device_record(uint32_t *id) __attribute__((weak));
static int native_storage_query(uint32_t va)
{
    if (va == 0x24EC0u && tsfp_storage_device_record != NULL) {
        uint32_t record[9] = {0};
        if (!tsfp_storage_device_record(&record[0])) return 0;
        /* Original callback(ctx, &{slot, display_name[32]}), not three arguments.
         * Enumeration start orders multiple devices; with one device it is immaterial. */
        memcpy(&record[1], "Local PC", sizeof "Local PC");
        const uint32_t top = g_esp;
        if (top < sizeof record) tsfp_bridge_fail(HOST_STOP_BRIDGE_CONTRACT, va, top, top, "storage callback stack underflow");
        const uint32_t address = top - (uint32_t)sizeof record;
        if (!kernel_guest_write_bytes(address, record, sizeof record))
            tsfp_bridge_fail(HOST_STOP_BRIDGE_CONTRACT, va, top, top, "storage callback stack not writable");
        const uint32_t args[2] = {game_stack_arg(2), address};
        const uint32_t callback = game_stack_arg(1);
        g_esp = address;
        uint32_t result = 0;
        const bool ok = recomp_guest_call_cdecl(callback, args, 2u, &result);
        g_esp = top;
        if (!ok) tsfp_bridge_fail(HOST_STOP_BRIDGE_CONTRACT, callback, top, top, "storage enumeration callback failed");
        g_eax = result;
        return 1;
    }
    return 0;
}

extern void tsfp_uihook_original_00024ec0(void);
void sub_00024EC0(void);
void sub_00024EC0(void)
{
    if (native_storage_query(0x24EC0u)) { g_esp += 4u; return; }
    tsfp_uihook_original_00024ec0();
}


/* T1916: the Map Maker Controls picture callback is cdecl `(page)`. A scope-2
 * page retains 0x76810's frame/chrome and replaces only this custom callback.
 * The same per-frame Rust latch decides both suppression and native drawing.
 * F9, unsupported mode, a different widget, absent shell or failed layer keeps
 * the retained original. This boundary writes no guest state or registers
 * except the cdecl return-address pop. */
extern void tsfp_uihook_original_00314a50(void);
void sub_00314A50(void);
void sub_00314A50(void)
{
    static unsigned logged_native;
    const uint32_t widget = game_stack_arg(0);
    if (widget != 0u && tsfp_ui_page_draw_owned != NULL &&
        tsfp_ui_page_draw_owned(widget, guest_read32(widget + 8u), guest_read32(widget + 0xF4u)) == 2) {
        if (ui_draw_log > 0 && !logged_native++)
            fprintf(stderr, "ui-draw 0x314A50 widget=0x%08x -> rust callback; framework retained\n", widget);
        g_esp += 4u;
        return;
    }
    tsfp_uihook_original_00314a50();
}
