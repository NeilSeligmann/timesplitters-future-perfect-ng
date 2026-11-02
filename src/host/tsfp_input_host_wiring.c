/* T3975: C implementation of the tsfp_input_host_* entry points, a verbatim move of the input blocks of
 * src/host/main.c (same calls, order and messages). The fallback behind TSFP_RUST_INPUT_HLE=0 and the baseline the
 * Rust input host must equal; deleted with the other input C units at retirement (T3879). */
#include "tsfp_input_host_wiring.h"

#include <stdio.h>
#include <stdlib.h>

#include "mu_device.h"
#include "xinput_devices.h"
#include "xinput_hle.h"
#include "xinput_hotkey.h"
#include "xinput_source.h"
#include "xinput_host_source.h"
#include "xvoice_media.h"
#include <string.h>

bool tsfp_input_host_pad_source_kind_parse(const char *text, uint32_t *kind)
{
    if (text == NULL || kind == NULL) return false;
    if (strcmp(text, "script") == 0) *kind = 0u;
    else if (strcmp(text, "keyboard") == 0) *kind = 1u;
    else if (strcmp(text, "gamepad") == 0) *kind = 2u;
    else return false;
    return true;
}

void tsfp_input_host_set_pre_install_hook(xinput_pre_install_hook_fn hook, void *user)
{
    xinput_source_set_pre_install_hook(hook, user);
}

void tsfp_input_host_translate_sdl_event(uint32_t kind, const char *name, int32_t raw, int32_t value,
                                         uint64_t poll, xinput_host_event *out)
{
    const bool keyboard = kind == 1u || kind == 2u;
    const xinput_host_action action = kind == 1u || kind == 3u ? XINPUT_HOST_DOWN
                                     : kind == 2u || kind == 4u ? XINPUT_HOST_UP : XINPUT_HOST_AXIS;
    uint32_t code = UINT32_MAX;
    if (name != NULL)
        code = action == XINPUT_HOST_AXIS ? xinput_host_axis_of(name)
                                          : xinput_host_code_of(keyboard ? XINPUT_HOST_KEYBOARD : XINPUT_HOST_GAMEPAD, name);
    *out = (xinput_host_event){.poll = poll, .device = keyboard ? XINPUT_HOST_KEYBOARD : XINPUT_HOST_GAMEPAD,
                               .action = action,
                               .code = code != UINT32_MAX ? code : 0x10000000u + ((uint32_t)raw & 0xFFFFFFu),
                               .value = value};
}

void tsfp_input_host_reset_all(void)
{
    xinput_hle_init();
    xinput_devices_reset();
}

void tsfp_input_host_set_log(xinput_log_fn printer)
{
    xinput_hle_set_log(printer);
}

bool tsfp_input_host_replay_budgets(xinput_record_budgets *out)
{
    return out != NULL && xinput_replay_budgets(out);
}

bool tsfp_input_host_configure_pads(const tsfp_input_host_pad_config *config)
{
    xinput_devices_enable_synthetic_pad(false);
    if (config->synthetic_pad) {
        (void)xinput_hle_attach_synthetic_pad(0u);
        xinput_devices_enable_synthetic_pad(true);
        printf("xinput policy  --synthetic-pad FABRICATED: port 0 reports one gamepad inserted, the title's "
               "open path is answered, buttons and sticks at rest (T717)\n");
    }
    if (config->controllers) {
        xinput_devices_enable_synthetic_pad(true);
        xinput_devices_enable_multiport(true);
    }
    if (config->pad_ports) {
        if (!config->pad_ports_router_present) {
            fprintf(stderr, "--pad-ports: needs the Bevy shell's input router (crates/tsfp-input-slots, T3771)\n");
            return false;
        }
        xinput_devices_enable_multiport(true);
        printf("xinput policy  --pad-ports: four-port adapter, port 0 the --synthetic-pad queue pad, ports 1-3 follow the "
               "input router slots 1-3 (Rust-owned presence and state, T3771)\n");
    }
    xinput_pad_remove_after_polls(config->synthetic_pad_remove_after_polls);
    if (config->synthetic_pad_remove_after_polls != 0u)
        printf("xinput policy  --synthetic-pad-remove-after-polls FABRICATED: the pad is unplugged after "
               "XInputGetState call %u (T731)\n", config->synthetic_pad_remove_after_polls);
    xinput_source_reset();
    if (config->pad_script != NULL && config->pad_script_live) {
        char live_error[160];
        xinput_live *live = xinput_live_open(config->pad_script, live_error, sizeof(live_error));
        if (live == NULL) {
            fprintf(stderr, "--pad-script-live %s: %s\n", config->pad_script, live_error);
            return false;
        }
        xinput_live_install(live);
        printf("xinput policy  --pad-script-live FABRICATED: one line of pad tokens re-read at every XInputGetState from %s, "
               "rest while empty (T1222)\n", config->pad_script);
    } else if (config->pad_script != NULL) {
        char script_error[160];
        xinput_script *script = xinput_script_load(config->pad_script, script_error, sizeof(script_error));
        if (script == NULL) {
            fprintf(stderr, "--pad-script %s: %s\n", config->pad_script, script_error);
            return false;
        }
        xinput_script_install(script);
        printf("xinput policy  --pad-script FABRICATED: %zu entries, %llu frames of scripted pad state from %s, "
               "one per XInputGetState, rest afterwards (T707)\n", xinput_script_entry_count(script),
               (unsigned long long)xinput_script_total_frames(script), config->pad_script);
    }
    return true;
}

void tsfp_input_host_register_all(const tsfp_input_host_register_config *config)
{
    xinput_devices_set_fatal(config->device_fatal);
    (void)xinput_devices_register();
    (void)xvoice_media_register(); /* T1086: measured disconnected-device path only. */
    (void)mu_register();
    if (config->pad_ports) {
        /* After every source reset: port 0 falls back to the legacy queue source, 1-3 read the router table. */
        xinput_source_install_ports(config->ports_sample, NULL);
        (void)config->ports_bind(xinput_pad_connect, xinput_pad_disconnect);
    }
    if (config->synthetic_pad || config->controllers) (void)xinput_devices_register_pad();
    printf("xinput handlers %zu registered\n", xinput_hle_implemented_count());
}

void tsfp_input_host_register_report_surface(void)
{
    (void)xinput_devices_register();
    (void)xvoice_media_register();
    (void)mu_register();
}

uint64_t tsfp_input_host_poll_count(unsigned port)
{
    return xinput_source_port_poll_count(port);
}

void tsfp_input_host_timing_enable(bool enabled)
{
    xinput_source_t3615_timing_enable(enabled);
}

void tsfp_input_host_timing_print(FILE *out)
{
    xinput_source_t3615_timing_print(out);
}

typedef struct {
    xinput_hotkeys *hotkeys;
    bool ticked;
    uint64_t ticked_poll;
} tsfp_input_host_hotkey_feed;

void *tsfp_input_host_hotkey_feed_create(void *hotkeys)
{
    if (hotkeys == NULL) return NULL;
    tsfp_input_host_hotkey_feed *feed = calloc(1u, sizeof(*feed));
    if (feed != NULL) feed->hotkeys = hotkeys;
    return feed;
}

void tsfp_input_host_hotkey_feed_free(void *opaque)
{
    free(opaque);
}

void tsfp_input_host_hotkey_feed_tick(void *opaque, uint64_t now_ms, uint64_t poll)
{
    tsfp_input_host_hotkey_feed *feed = opaque;
    if (feed == NULL || (feed->ticked && feed->ticked_poll == poll)) return;
    feed->ticked = true;
    feed->ticked_poll = poll;
    xinput_hotkeys_tick(feed->hotkeys, now_ms, poll);
}

bool tsfp_input_host_hotkey_feed_event(void *opaque, uint32_t kind, const char *name, uint64_t now_ms, uint64_t poll)
{
    tsfp_input_host_hotkey_feed *feed = opaque;
    if (feed == NULL) return true;
    xinput_hotkey_device device;
    bool down;
    switch (kind) {
    case 1u: device = XINPUT_HOTKEY_DEVICE_KEYBOARD; down = true; break;
    case 2u: device = XINPUT_HOTKEY_DEVICE_KEYBOARD; down = false; break;
    case 3u: device = XINPUT_HOTKEY_DEVICE_PAD; down = true; break;
    case 4u: device = XINPUT_HOTKEY_DEVICE_PAD; down = false; break;
    default: return true;
    }
    return xinput_hotkeys_event(feed->hotkeys, device, name, down, now_ms, poll);
}

bool tsfp_input_host_hotkey_needs_device(const void *hotkeys, uint32_t device)
{
    return hotkeys != NULL && xinput_hotkeys_needs_device(hotkeys, (xinput_hotkey_device)device);
}
