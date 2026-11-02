#ifndef TSFP_INPUT_HOST_WIRING_H
#define TSFP_INPUT_HOST_WIRING_H
/* T3975: the input host duties (reset, log sink, pad policy, handler registration, poll counters, timing report)
 * behind tsfp_input_host_* entry points, the boundary the Rust input host must export at retirement (T3879).
 * Calls, order and messages are those src/host/main.c made inline. Later slices move record/replay, route,
 * hotkeys and pad sources behind the same contract. */
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include "xinput_hle.h"
#include "xinput_host_source.h"
#include "xinput_source.h"
#include "xinput_record.h"

typedef void (*tsfp_input_poll_observer_fn)(uint32_t port, uint64_t port_polls, void *user);

typedef struct tsfp_input_host_pad_config {
    bool synthetic_pad, controllers, pad_ports;
    bool pad_ports_router_present; /* the Bevy shell's tsfp_pad_ports_* symbols resolved */
    uint32_t synthetic_pad_remove_after_polls;
    const char *pad_script;
    bool pad_script_live;
} tsfp_input_host_pad_config;

typedef struct tsfp_input_host_register_config {
    bool synthetic_pad, controllers, pad_ports;
    void (*device_fatal)(uint32_t address, const char *reason);
    bool (*ports_sample)(uint64_t index, unsigned port, xinput_pad_state *out, void *user);
    bool (*ports_bind)(bool (*connect)(unsigned), bool (*disconnect)(unsigned));
} tsfp_input_host_register_config;

/* Put every input HLE module back to its start state (run start). */
void tsfp_input_host_reset_all(void);
/* Diagnostics sink of the input HLE. */
void tsfp_input_host_set_log(xinput_log_fn printer);
/* Synthetic pad, multiport, pad-ports, remove-after-polls and script/live script policy. false after a stderr refusal (exit 2). */
bool tsfp_input_host_configure_pads(const tsfp_input_host_pad_config *config);
/* Register the input, voice and MU handlers and the pad ports adapter, print the handler count. */
void tsfp_input_host_register_all(const tsfp_input_host_register_config *config);
/* Register the input, voice and MU handler surfaces for the standalone XDK report tool. */
void tsfp_input_host_register_report_surface(void);
/* Polls of XInputGetState consumed on a port. */
uint64_t tsfp_input_host_poll_count(unsigned port);
/* Host-facing query; the record reader remains the C baseline pending equal Rust reader evidence. */
uint64_t tsfp_input_host_replay_total_polls(void);
/* Host-facing replay budget metadata; the record reader remains the C baseline. */
bool tsfp_input_host_replay_budgets(xinput_record_budgets *out);
/* Reserve the legacy queue at port zero for an explicit host keyboard/script source. */
void tsfp_input_host_reserve_port_zero(void);
/* Parse the host's script/keyboard/gamepad source selection; writes stable source tag 0/1/2. */
bool tsfp_input_host_pad_source_kind_parse(const char *text, uint32_t *kind);
void tsfp_input_host_set_poll_observer(tsfp_input_poll_observer_fn observer, void *user);
void tsfp_input_host_notify_poll_observer(unsigned port, uint64_t port_polls);
void tsfp_input_host_set_pre_install_hook(xinput_pre_install_hook_fn hook, void *user);
void tsfp_input_host_store_pre_install_hook(xinput_pre_install_hook_fn hook, void *user);
void tsfp_input_host_pre_install(uint64_t poll, const xinput_pad_state *state);
void tsfp_input_host_translate_sdl_event(uint32_t kind, const char *name, int32_t raw, int32_t value,
                                         uint64_t poll, xinput_host_event *out);
void tsfp_input_host_timing_enable(bool enabled);
void tsfp_input_host_timing_print(FILE *out);
/* SDL pad-feed hotkey scheduling and dispatch. The opaque feed owns its per-poll tick state in Rust. */
void *tsfp_input_host_hotkey_feed_create(void *hotkeys);
void tsfp_input_host_hotkey_feed_free(void *feed);
void tsfp_input_host_hotkey_feed_tick(void *feed, uint64_t now_ms, uint64_t poll);
bool tsfp_input_host_hotkey_feed_event(void *feed, uint32_t kind, const char *name, uint64_t now_ms, uint64_t poll);
bool tsfp_input_host_hotkey_needs_device(const void *hotkeys, uint32_t device);
#endif
