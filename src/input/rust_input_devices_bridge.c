/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "xinput_devices_rust.h"

#include <stddef.h>
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>
#include <pthread.h>

#include "kernel_call.h"
#include "xinput_hle.h"
#include "tsfp_input_dispatch.h"
#include "../host/t2_capture.h"

extern void t2cap_input_begin(const t2_input_annotation *) __attribute__((weak));

typedef bool (*tsfp_guest_read_fn)(void *user, uint32_t address, uint8_t *output, size_t length);
typedef bool (*tsfp_guest_write_fn)(void *user, uint32_t address, const uint8_t *bytes, size_t length);

static tsfp_input_dispatch *hle_dispatch;
static pthread_once_t hle_dispatch_once = PTHREAD_ONCE_INIT;
static atomic_uint_fast64_t hle_dispatch_calls;

static void init_dispatch(void)
{
    hle_dispatch = tsfp_input_dispatch_new();
}

static tsfp_input_dispatch *dispatch_handle(void)
{
    if (pthread_once(&hle_dispatch_once, init_dispatch) != 0) abort();
    return hle_dispatch;
}

bool xinput_hle_rust_dispatch_register(uint32_t address, xinput_fn handler, uint32_t identity)
{
    tsfp_input_dispatch *dispatch = dispatch_handle();
    return handler != NULL && dispatch != NULL &&
           tsfp_input_dispatch_register_identified(dispatch, address, handler, identity);
}

bool xinput_hle_rust_dispatch_reset(void)
{ return tsfp_input_dispatch_reset(dispatch_handle()); }

bool xinput_hle_rust_dispatch_default(uint32_t address, uint32_t value)
{ return tsfp_input_dispatch_default(dispatch_handle(), address, value); }

static void capture_snapshot(const tsfp_input_capture_selection *snapshot)
{
    t2_input_annotation annotation = {
        .generation = snapshot->generation, .lifecycle_sequence = snapshot->lifecycle_sequence,
        .selection_sequence = snapshot->selection_sequence, .address = snapshot->address,
        .route = snapshot->route, .handler = snapshot->handler,
        .default_return = snapshot->default_return, .complete = snapshot->complete,
        .entry_count = snapshot->entry_count,
    };
    _Static_assert(T2_INPUT_MAX_ENTRIES == TSFP_INPUT_CAPTURE_MAX_ENTRIES, "capture capacity mismatch");
    for (uint32_t i = 0; i < snapshot->entry_count && i < T2_INPUT_MAX_ENTRIES; i++) {
        annotation.entries[i] = (t2_input_entry){snapshot->entries[i].address,
            snapshot->entries[i].handler, snapshot->entries[i].default_return};
    }
    t2cap_input_begin(&annotation);
}

void xinput_hle_rust_dispatch_observe_c(uint32_t address, xinput_fn actual, uint32_t c_default)
{
    if (t2cap_input_begin == NULL) return;
    tsfp_input_capture_selection snapshot;
    if (tsfp_input_dispatch_observe_c(dispatch_handle(), address, actual, c_default, &snapshot))
        capture_snapshot(&snapshot);
}

bool xinput_hle_rust_dispatch_select(uint32_t address, xinput_fn *handler_out)
{
    tsfp_input_dispatch *dispatch = dispatch_handle();
    if (handler_out == NULL || dispatch == NULL) return false;
    if (t2cap_input_begin != NULL) {
        tsfp_input_capture_selection snapshot;
        *handler_out = tsfp_input_dispatch_select_capture(dispatch, address, &snapshot);
        capture_snapshot(&snapshot);
    } else {
        *handler_out = tsfp_input_dispatch_select(dispatch, address);
    }
    if (*handler_out == NULL) return false;
    atomic_fetch_add_explicit(&hle_dispatch_calls, 1u, memory_order_relaxed);
    return true;
}

uint64_t xinput_hle_rust_dispatch_calls(void)
{
    return atomic_load_explicit(&hle_dispatch_calls, memory_order_relaxed);
}

extern uint32_t tsfp_input_device_hle_reset(void);
extern uint32_t tsfp_input_device_hle_set_mu_mask(uint32_t mask, void *user,
                                                   tsfp_guest_read_fn read, tsfp_guest_write_fn write);
extern uint32_t tsfp_input_device_hle_initialize(uint32_t count, uint32_t declarations,
                                                  uint32_t connected_ports, bool synthetic,
                                                  bool multiport, bool removed, void *user,
                                                  tsfp_guest_read_fn read, tsfp_guest_write_fn write,
                                                  uint32_t *insertions_out);
extern uint32_t tsfp_input_device_hle_get(uint32_t device_type, uint32_t connected_ports,
                                          bool synthetic, bool multiport, bool removed, void *user,
                                          tsfp_guest_read_fn read, tsfp_guest_write_fn write,
                                          uint32_t *value_out);
extern uint32_t tsfp_input_device_hle_peek(uint32_t device_type, uint32_t previous,
                                            uint32_t reconnected, uint32_t connected_ports,
                                            bool synthetic, bool multiport, bool removed, void *user,
                                            tsfp_guest_read_fn read, tsfp_guest_write_fn write,
                                            uint32_t *value_out);
extern uint32_t tsfp_input_device_hle_changes(uint32_t device_type, uint32_t insertions,
                                              uint32_t removals, uint32_t connected_ports,
                                              bool synthetic, bool multiport, bool removed, void *user,
                                              tsfp_guest_read_fn read, tsfp_guest_write_fn write,
                                              bool *changed_out);
extern uint32_t tsfp_input_device_hle_enumeration_status(uint32_t connected_ports,
                                                         bool synthetic, bool multiport, bool removed,
                                                         void *user, tsfp_guest_read_fn read,
                                                         tsfp_guest_write_fn write, bool *status_out);

static bool guest_read(void *user, uint32_t address, uint8_t *output, size_t length)
{
    (void)user;
    return kernel_guest_read_bytes((kernel_guest_ptr)address, output, length);
}

static bool guest_write(void *user, uint32_t address, const uint8_t *bytes, size_t length)
{
    (void)user;
    return kernel_guest_write_bytes((kernel_guest_ptr)address, bytes, length);
}

static uint32_t connected_ports(void)
{
    uint32_t mask = 0u;
    for (unsigned port = 0u; port < XINPUT_PORT_COUNT; port++)
        if (xinput_hle_port_state(port) == XINPUT_PORT_SYNTHETIC) mask |= 1u << port;
    return mask;
}

bool xinput_devices_rust_enabled(void)
{
    const char *value = getenv("TSFP_RUST_INPUT_HLE");
    return value == NULL || strcmp(value, "0") != 0; /* IN-5: Rust default, =0 is the C fallback */
}

uint32_t xinput_devices_rust_reset(uint32_t mu_mask)
{
    const uint32_t reset = tsfp_input_device_hle_reset();
    if (reset != 0u) return reset;
    return xinput_devices_rust_set_mu_mask(mu_mask);
}

uint32_t xinput_devices_rust_set_mu_mask(uint32_t mask)
{
    return tsfp_input_device_hle_set_mu_mask(mask, NULL, guest_read, guest_write);
}

uint32_t xinput_devices_rust_initialize(uint32_t count, uint32_t declarations, bool synthetic,
                                        bool multiport, bool removed, uint32_t *insertions_out)
{
    return tsfp_input_device_hle_initialize(count, declarations, connected_ports(), synthetic, multiport,
                                            removed, NULL, guest_read, guest_write, insertions_out);
}

uint32_t xinput_devices_rust_get(uint32_t type, bool synthetic, bool multiport, bool removed,
                                 uint32_t *value_out)
{
    return tsfp_input_device_hle_get(type, connected_ports(), synthetic, multiport, removed,
                                     NULL, guest_read, guest_write, value_out);
}

uint32_t xinput_devices_rust_peek(uint32_t type, uint32_t previous, uint32_t reconnected,
                                  bool synthetic, bool multiport, bool removed, uint32_t *value_out)
{
    return tsfp_input_device_hle_peek(type, previous, reconnected, connected_ports(), synthetic,
                                      multiport, removed, NULL, guest_read, guest_write, value_out);
}

uint32_t xinput_devices_rust_changes(uint32_t type, uint32_t insertions, uint32_t removals,
                                     bool synthetic, bool multiport, bool removed, bool *changed_out)
{
    return tsfp_input_device_hle_changes(type, insertions, removals, connected_ports(), synthetic,
                                         multiport, removed, NULL, guest_read, guest_write, changed_out);
}

uint32_t xinput_devices_rust_enumeration_status(bool synthetic, bool multiport, bool removed,
                                                bool *status_out)
{
    return tsfp_input_device_hle_enumeration_status(connected_ports(), synthetic, multiport, removed,
                                                    NULL, guest_read, guest_write, status_out);
}
