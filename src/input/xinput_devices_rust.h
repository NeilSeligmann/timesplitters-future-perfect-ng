/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_INPUT_XINPUT_DEVICES_RUST_H
#define TSFP_INPUT_XINPUT_DEVICES_RUST_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

bool xinput_devices_rust_enabled(void);
uint64_t xinput_hle_rust_dispatch_calls(void);
uint32_t xinput_devices_rust_reset(uint32_t mu_mask);
uint32_t xinput_devices_rust_set_mu_mask(uint32_t mask);
uint32_t xinput_devices_rust_initialize(uint32_t count, uint32_t declarations, bool synthetic,
                                        bool multiport, bool removed, uint32_t *insertions_out);
uint32_t xinput_devices_rust_get(uint32_t type, bool synthetic, bool multiport, bool removed,
                                 uint32_t *value_out);
uint32_t xinput_devices_rust_peek(uint32_t type, uint32_t previous, uint32_t reconnected,
                                  bool synthetic, bool multiport, bool removed, uint32_t *value_out);
uint32_t xinput_devices_rust_changes(uint32_t type, uint32_t insertions, uint32_t removals,
                                     bool synthetic, bool multiport, bool removed, bool *changed_out);
uint32_t xinput_devices_rust_enumeration_status(bool synthetic, bool multiport, bool removed,
                                                bool *status_out);

/* T3926: Rust input record reader (crates/tsfp-input-record via tsfp-input-capi). */
typedef struct tsfp_input_record tsfp_input_record;
tsfp_input_record *tsfp_input_record_load(const uint8_t *bytes, size_t len, const char *xbe_sha256, const char *flags_sha256,
                                          const char *flags, char *error, size_t error_capacity);
void tsfp_input_record_free(tsfp_input_record *loaded);
uint64_t tsfp_input_record_total(const tsfp_input_record *loaded);
size_t tsfp_input_record_entry_count(const tsfp_input_record *loaded);
bool tsfp_input_record_entry(const tsfp_input_record *loaded, size_t index, uint32_t *frames, uint16_t *digital, uint8_t *analog,
                             int16_t *thumbs);
bool tsfp_input_record_budgets(const tsfp_input_record *loaded, uint32_t *out);
size_t tsfp_input_record_mark_count(const tsfp_input_record *loaded);
uint64_t tsfp_input_record_mark(const tsfp_input_record *loaded, size_t index);
size_t tsfp_input_record_wait_count(const tsfp_input_record *loaded);
const char *tsfp_input_record_wait(const tsfp_input_record *loaded, size_t index);
size_t tsfp_input_record_nav_count(const tsfp_input_record *loaded);
const char *tsfp_input_record_nav(const tsfp_input_record *loaded, size_t index);

/* T3926: Rust-owned pad source poll counters. begin returns the zero based poll index (UINT64_MAX for a bad port). */
uint64_t tsfp_input_poll_begin(uint32_t port);
void tsfp_input_poll_reset(void);
uint64_t tsfp_input_poll_total(void);
uint64_t tsfp_input_poll_port(uint32_t port);

/* T3926: Rust-owned device state (crates/tsfp-input-devices via tsfp-input-capi). */
void tsfp_input_devices_lock(void);
bool tsfp_input_devices_unlock(void);
uint32_t tsfp_input_devices_depth(void);
void tsfp_input_devices_state_reset(void);
void tsfp_input_pad_set_remove_after_polls(uint32_t polls);
bool tsfp_input_pad_open_allowed(uint32_t type, uint32_t port, uint32_t slot);
void tsfp_input_pad_open(uint32_t report_changes);
void tsfp_input_pad_close(void);
bool tsfp_input_pad_is_open(void);
bool tsfp_input_pad_is_removed(void);
uint32_t tsfp_input_pad_remove_refusal(void);
uint32_t tsfp_input_pad_insert_refusal(void);
void tsfp_input_pad_set_removed(bool removed);
uint32_t tsfp_input_pad_sample(uint32_t report_changes);
uint32_t tsfp_input_pad_take_pending(void);
void tsfp_input_pad_set_pending(uint32_t address);
void tsfp_input_pad_record_output(uint32_t left_right);
bool tsfp_input_pad_last_output(uint8_t out[6]);
uint64_t tsfp_input_pad_output_count(void);
bool tsfp_input_pad_poll_fires_removal(void);
bool tsfp_input_host_open_allowed(uint32_t type, uint32_t port, uint32_t slot, bool port_is_synthetic);
uint32_t tsfp_input_host_open(uint32_t port, uint32_t report_changes, uint32_t *token_out);
bool tsfp_input_host_contains(uint32_t token);
bool tsfp_input_host_is_removed(uint32_t token);
uint32_t tsfp_input_host_port_of(uint32_t token);
uint32_t tsfp_input_host_packet(uint32_t token, uint32_t report_changes);
uint32_t tsfp_input_host_take_pending(uint32_t token);
void tsfp_input_host_set_pending(uint32_t token, uint32_t address);
bool tsfp_input_host_close(uint32_t token, uint32_t *pending, uint32_t *silence_port);
uint32_t tsfp_input_host_disconnect(uint32_t port, uint32_t *pending /* 64 entries */);

/* T3926 slice 2: Rust-owned memory-unit slot table (crates/tsfp-input-devices mu.rs). */
int32_t tsfp_input_mu_index(uint32_t port, uint32_t slot);
void tsfp_input_mu_reset(void);
uint32_t tsfp_input_mu_mounted_mask(void);
bool tsfp_input_mu_is_mounted(uint32_t index);
int32_t tsfp_input_mu_peek_free_id(void);
void tsfp_input_mu_mark_mounted(uint32_t index, bool guest_backed);
bool tsfp_input_mu_mark_unmounted(uint32_t index);
void tsfp_input_mu_set_eject_pending(uint32_t index, bool pending);
bool tsfp_input_mu_eject_pending(uint32_t index);
bool tsfp_input_mu_mounted_flag(uint32_t index);
bool tsfp_input_mu_is_guest_backed(uint32_t index);
uint32_t tsfp_input_mu_device_id(uint32_t index);

/* T3926 slice 7: Rust host hotkey chord engine (crates/tsfp-input-devices hotkey.rs via tsfp-input-capi). */
typedef struct tsfp_hotkey_spec {
    uint32_t device, hold_ms;
    char label[32];
    uint32_t key_count, accept_count[5];
    char accept[5][2][16];
} tsfp_hotkey_spec;
typedef struct tsfp_input_hotkeys tsfp_input_hotkeys;
typedef void (*tsfp_hotkey_fire_cb)(uint32_t number, const char *label, uint64_t poll, void *user);
typedef void (*tsfp_hotkey_leak_cb)(uint32_t device, const char *name, uint64_t poll, bool begin, void *user);
tsfp_input_hotkeys *tsfp_input_hotkeys_create(const tsfp_hotkey_spec *specs, uint32_t count, const char *dir);
void tsfp_input_hotkeys_free(tsfp_input_hotkeys *hotkeys);
bool tsfp_input_hotkeys_event(tsfp_input_hotkeys *hotkeys, uint32_t device, const char *name, bool down, uint64_t now_ms,
                              uint64_t poll, tsfp_hotkey_fire_cb fire, tsfp_hotkey_leak_cb leak, void *user);
void tsfp_input_hotkeys_tick(tsfp_input_hotkeys *hotkeys, uint64_t now_ms, uint64_t poll, tsfp_hotkey_fire_cb fire,
                             tsfp_hotkey_leak_cb leak, void *user);
uint32_t tsfp_input_hotkeys_counter(const tsfp_input_hotkeys *hotkeys, uint32_t which);
bool tsfp_input_hotkeys_needs_device(const tsfp_input_hotkeys *hotkeys, uint32_t device);

/* T3926 slice 8: Rust held-key model of the host keyboard/gamepad sources (hostsource in tsfp-input-devices). */
typedef struct tsfp_input_host_source tsfp_input_host_source;
tsfp_input_host_source *tsfp_input_host_source_create(uint32_t device);
void tsfp_input_host_source_free(tsfp_input_host_source *source);
bool tsfp_input_host_source_apply(tsfp_input_host_source *source, uint32_t device, uint32_t action, uint32_t code, int32_t value);
void tsfp_input_host_source_compose(const tsfp_input_host_source *source, uint16_t *digital, uint8_t *analog, int16_t *thumbs);
uint64_t tsfp_input_host_source_counter(const tsfp_input_host_source *source, uint32_t which);
void tsfp_input_host_source_count_applied(tsfp_input_host_source *source);
bool tsfp_input_host_row(uint32_t device, uint32_t code, uint32_t *kind, uint16_t *digital, uint8_t *analog, uint8_t *axis,
                         int8_t *direction);

/* T3926 slice 9: Rust scripted/live pad sources (script.rs); C keeps the file reads. */
typedef struct tsfp_input_script tsfp_input_script;
typedef struct tsfp_input_live tsfp_input_live;
typedef struct {
    uint32_t packet_number;
    uint16_t digital_buttons;
    uint8_t analog[8];
    int16_t thumbs[4];
} tsfp_pad_state;
tsfp_input_script *tsfp_input_script_parse(const uint8_t *text, size_t len, char *error, size_t error_size);
tsfp_input_script *tsfp_input_script_from_entries(const void *entries, size_t count);
void tsfp_input_script_free(tsfp_input_script *script);
size_t tsfp_input_script_entry_count(const tsfp_input_script *script);
uint64_t tsfp_input_script_total_frames(const tsfp_input_script *script);
void tsfp_input_script_state_at(const tsfp_input_script *script, uint64_t poll_index, tsfp_pad_state *out);
bool tsfp_input_live_parse_line(const uint8_t *text, size_t len, tsfp_pad_state *out);
tsfp_input_live *tsfp_input_live_create(void);
void tsfp_input_live_free(tsfp_input_live *live);
void tsfp_input_live_poll(tsfp_input_live *live, const uint8_t *buffer, ptrdiff_t got, tsfp_pad_state *out);
uint64_t tsfp_input_live_refused(const tsfp_input_live *live);

#endif
