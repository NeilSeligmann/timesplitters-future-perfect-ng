/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "xinput_source.h"
#ifdef TSFP_INPUT_HLE_LINKED
#include "tsfp_input_host_wiring.h"
#else
static xinput_poll_observer_fn g_poll_observer;
static void *g_poll_observer_user;
void xinput_source_set_poll_observer(xinput_poll_observer_fn observer, void *user)
{
    g_poll_observer_user = user;
    g_poll_observer = observer;
}
#endif
#include "xinput_devices.h"
#include "xinput_devices_rust.h"
#include "xinput_record.h"
#include <ctype.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <pthread.h>
#include <stdatomic.h>
#include <time.h>
#include <fcntl.h>
#include <unistd.h>

static pthread_mutex_t source_mutex = PTHREAD_MUTEX_INITIALIZER;
/* Provider lifetimes must span in-flight sampling; callbacks run without this
 * lock so hotplug can safely reenter the serialized device adapter. */
static xinput_source_fn source_fn;
static xinput_ports_source_fn ports_source_fn;
static void *source_user;
static void *ports_source_user;
static uint64_t polls;
static uint64_t port_polls[XINPUT_PORT_COUNT];
/* T3926: with the Rust bridge linked the poll counters live in crates/tsfp-input-devices (poll.rs); the statics above are
 * the TSFP_RUST_INPUT_HLE=0 baseline. */
#if defined(TSFP_INPUT_HLE_LINKED)
#define COUNTERS_IN_RUST() xinput_devices_rust_enabled()
#else
#define COUNTERS_IN_RUST() false
/* Hand-linked test closures have no Rust archive; these branches are dead there but must still link. */
#define tsfp_input_script_parse(t, l, e, n) ((void)(t), (void)(l), (void)(e), (void)(n), (tsfp_input_script *)NULL)
#define tsfp_input_script_from_entries(e, n) ((void)(e), (void)(n), (tsfp_input_script *)NULL)
#define tsfp_input_script_free(s) ((void)(s))
#define tsfp_input_script_entry_count(s) ((void)(s), (size_t)0)
#define tsfp_input_script_total_frames(s) ((void)(s), (uint64_t)0)
#define tsfp_input_script_state_at(s, i, o) ((void)(s), (void)(i), (void)memset((o), 0, sizeof(tsfp_pad_state)))
#define tsfp_input_live_parse_line(t, l, o) ((void)(t), (void)(l), (void)memset((o), 0, sizeof(tsfp_pad_state)), false)
#define tsfp_input_live_create() ((tsfp_input_live *)NULL)
#define tsfp_input_live_free(l) ((void)(l))
#define tsfp_input_live_poll(l, b, g, o) ((void)(l), (void)(b), (void)(g), (void)memset((o), 0, sizeof(tsfp_pad_state)))
#define tsfp_input_live_refused(l) ((void)(l), (uint64_t)0)
#endif
static xinput_feedback_fn feedback_fn;
static void *feedback_user;
#if !defined(TSFP_INPUT_HLE_LINKED)
static xinput_pre_install_hook_fn pre_install_hook;
static void *pre_install_user;
#endif
static bool t3615_timing_enabled;
static _Atomic uint64_t t3615_timing_calls;
static _Atomic uint64_t t3615_timing_sync_ns;
static _Atomic uint64_t t3615_timing_poll_ns;
static _Atomic uint64_t t3615_timing_total_ns;
static _Atomic uint64_t t3615_timing_sync_max_ns;
static _Atomic uint64_t t3615_timing_poll_max_ns;
static _Atomic uint64_t t3615_timing_total_max_ns;

static uint64_t t3615_monotonic_ns(void)
{
    struct timespec now;
    if (clock_gettime(CLOCK_MONOTONIC, &now) != 0) return 0u;
    return (uint64_t)now.tv_sec * UINT64_C(1000000000) + (uint64_t)now.tv_nsec;
}

static void t3615_atomic_max(_Atomic uint64_t *target, uint64_t value)
{
    uint64_t previous = atomic_load_explicit(target, memory_order_relaxed);
    while (previous < value && !atomic_compare_exchange_weak_explicit(
               target, &previous, value, memory_order_relaxed, memory_order_relaxed)) {}
}

void xinput_source_t3615_timing_enable(bool enabled)
{
    t3615_timing_enabled = enabled;
    atomic_store(&t3615_timing_calls, 0u);
    atomic_store(&t3615_timing_sync_ns, 0u);
    atomic_store(&t3615_timing_poll_ns, 0u);
    atomic_store(&t3615_timing_total_ns, 0u);
    atomic_store(&t3615_timing_sync_max_ns, 0u);
    atomic_store(&t3615_timing_poll_max_ns, 0u);
    atomic_store(&t3615_timing_total_max_ns, 0u);
}

void xinput_source_t3615_timing_print(FILE *out)
{
    if (!t3615_timing_enabled || out == NULL) return;
    const uint64_t calls = atomic_load(&t3615_timing_calls);
    if (calls == 0u) return;
    const uint64_t sync = atomic_load(&t3615_timing_sync_ns);
    const uint64_t poll = atomic_load(&t3615_timing_poll_ns);
    const uint64_t total = atomic_load(&t3615_timing_total_ns);
    fprintf(out, "T3615 input poll timing: %llu calls; replay sync+mutation %.3f ms total %.3f us/call max %.3f ms; device poll %.3f ms total %.3f us/call max %.3f ms; whole source poll %.3f ms total %.3f us/call max %.3f ms (wall, diagnostic)\n",
            (unsigned long long)calls, (double)sync / 1e6, (double)sync / (double)calls / 1e3,
            (double)atomic_load(&t3615_timing_sync_max_ns) / 1e6,
            (double)poll / 1e6, (double)poll / (double)calls / 1e3,
            (double)atomic_load(&t3615_timing_poll_max_ns) / 1e6,
            (double)total / 1e6, (double)total / (double)calls / 1e3,
            (double)atomic_load(&t3615_timing_total_max_ns) / 1e6);
}

struct xinput_script {
    xinput_script_entry *entries;
    size_t count;
    uint64_t total;
    tsfp_input_script *rust; /* non-NULL: Rust owns the entries (script.rs), the C fields above are unused */
};

static void inner_xinput_source_install(xinput_source_fn source, void *user)
{
    (void)pthread_mutex_lock(&source_mutex);
    source_fn = source; source_user = user;
    (void)pthread_mutex_unlock(&source_mutex);
}
static void inner_xinput_source_install_ports(xinput_ports_source_fn source, void *user)
{
    (void)pthread_mutex_lock(&source_mutex);
    ports_source_fn = source; ports_source_user = user;
    (void)pthread_mutex_unlock(&source_mutex);
}
static bool inner_xinput_feedback_send(unsigned port, uint16_t left, uint16_t right)
{
    if (port >= XINPUT_PORT_COUNT) return false;
    (void)pthread_mutex_lock(&source_mutex);
    xinput_feedback_fn fn = feedback_fn; void *user = feedback_user;
    (void)pthread_mutex_unlock(&source_mutex);
    return fn != NULL && fn(port, left, right, user);
}
static void inner_xinput_set_pre_install_hook(xinput_pre_install_hook_fn hook, void *user)
{
#if defined(TSFP_INPUT_HLE_LINKED)
    tsfp_input_host_store_pre_install_hook(hook, user);
#else
    (void)pthread_mutex_lock(&source_mutex);
    pre_install_hook = hook; pre_install_user = user;
    (void)pthread_mutex_unlock(&source_mutex);
#endif
}
static void inner_xinput_feedback_install(xinput_feedback_fn feedback, void *user)
{
    (void)pthread_mutex_lock(&source_mutex);
    xinput_feedback_fn previous = feedback_fn; void *previous_user = feedback_user;
    feedback_fn = feedback; feedback_user = user;
    (void)pthread_mutex_unlock(&source_mutex);
    if (previous != NULL)
        for (unsigned port = 0; port < XINPUT_PORT_COUNT; port++) (void)previous(port, 0u, 0u, previous_user);
}
static void inner_xinput_source_reset(void)
{
    xinput_feedback_install(NULL, NULL);
    (void)pthread_mutex_lock(&source_mutex);
    source_fn = NULL; ports_source_fn = NULL; source_user = NULL; ports_source_user = NULL; polls = 0u;
#if defined(TSFP_INPUT_HLE_LINKED)
    tsfp_input_host_store_pre_install_hook(NULL, NULL);
#else
    pre_install_hook = NULL; pre_install_user = NULL;
#endif
    memset(port_polls, 0, sizeof(port_polls));
#if defined(TSFP_INPUT_HLE_LINKED)
    if (COUNTERS_IN_RUST()) tsfp_input_poll_reset();
#endif
    (void)pthread_mutex_unlock(&source_mutex);
}
uint64_t xinput_source_poll_count(void)
{
#if defined(TSFP_INPUT_HLE_LINKED)
    if (COUNTERS_IN_RUST()) return tsfp_input_poll_total();
#endif
    (void)pthread_mutex_lock(&source_mutex); uint64_t result = polls; (void)pthread_mutex_unlock(&source_mutex); return result;
}
uint64_t xinput_source_port_poll_count(unsigned port)
{
#if defined(TSFP_INPUT_HLE_LINKED)
    if (COUNTERS_IN_RUST()) return tsfp_input_poll_port(port);
#endif
    (void)pthread_mutex_lock(&source_mutex); uint64_t result = port < XINPUT_PORT_COUNT ? port_polls[port] : 0u;
    (void)pthread_mutex_unlock(&source_mutex); return result;
}
static void guest_boundary_read(void *user)
{
    *(uint64_t *)user = xinput_source_port_poll_count(0u);
}
uint64_t xinput_source_guest_boundary(void)
{
    uint64_t count = 0u;
    xinput_devices_run_locked(guest_boundary_read, &count);
    return count;
}
static bool inner_xinput_source_poll_port(unsigned port)
{
    if (port >= XINPUT_PORT_COUNT) return false;
    (void)pthread_mutex_lock(&source_mutex);
#if defined(TSFP_INPUT_HLE_LINKED)
    const uint64_t index = COUNTERS_IN_RUST() ? tsfp_input_poll_begin(port) : port_polls[port]++;
    if (!COUNTERS_IN_RUST()) polls++;
#else
    const uint64_t index = port_polls[port]++; polls++;
#endif
    xinput_ports_source_fn port_fn = ports_source_fn; void *port_user = ports_source_user;
    xinput_source_fn legacy_fn = source_fn; void *legacy_user = source_user;
#if !defined(TSFP_INPUT_HLE_LINKED)
    xinput_pre_install_hook_fn hook = port == 0u ? pre_install_hook : NULL; void *hook_user = pre_install_user;
#endif
    (void)pthread_mutex_unlock(&source_mutex);
    xinput_pad_state state = {0};
    bool supplied = port_fn != NULL && port_fn(index, port, &state, port_user);
    if (!supplied) {
        memset(&state, 0, sizeof(state));
        if (port != 0u || legacy_fn == NULL || !legacy_fn(index, &state, legacy_user)) {
            xinput_record_observe(index, port, NULL);
            return false;
        }
    }
#if defined(TSFP_INPUT_HLE_LINKED)
    if (port == 0u) tsfp_input_host_pre_install(index, &state);
#else
    if (hook != NULL) hook(index, &state, hook_user); /* T1629: observer, see the header for the rules */
#endif
    bool installed = xinput_hle_set_synthetic_pad_state(port, state);
    xinput_record_observe(index, port, installed ? &state : NULL);
    return installed;
}
bool xinput_source_poll(void) { return xinput_source_poll_port(0u); }

/* Share device serialization for provider installation, cancellation and calls.
 * Once uninstall returns, no earlier sample/feedback callback remains in flight. */
typedef struct {
    unsigned operation, port;
    xinput_source_fn legacy;
    xinput_ports_source_fn ports;
    xinput_feedback_fn feedback;
    xinput_pre_install_hook_fn hook;
    void *user;
    uint16_t left, right;
    bool result;
} source_operation;
static void source_job(void *user)
{
    source_operation *op = user;
    switch (op->operation) {
    case 0u: inner_xinput_source_install(op->legacy, op->user); break;
    case 1u: inner_xinput_source_install_ports(op->ports, op->user); break;
    case 2u: inner_xinput_feedback_install(op->feedback, op->user); break;
    case 3u: inner_xinput_source_reset(); break;
    case 4u: op->result = inner_xinput_source_poll_port(op->port); break;
    case 6u: inner_xinput_set_pre_install_hook(op->hook, op->user); break;
    default: op->result = inner_xinput_feedback_send(op->port, op->left, op->right); break;
    }
}
void xinput_source_install(xinput_source_fn source, void *user)
{
    source_operation op = {.operation = 0u, .legacy = source, .user = user};
    xinput_devices_run_locked(source_job, &op);
}
void xinput_source_install_ports(xinput_ports_source_fn source, void *user)
{
    source_operation op = {.operation = 1u, .ports = source, .user = user};
    xinput_devices_run_locked(source_job, &op);
}
void xinput_source_set_pre_install_hook(xinput_pre_install_hook_fn hook, void *user)
{
    source_operation op = {.operation = 6u, .hook = hook, .user = user};
    xinput_devices_run_locked(source_job, &op);
}
void xinput_feedback_install(xinput_feedback_fn feedback, void *user)
{
    source_operation op = {.operation = 2u, .feedback = feedback, .user = user};
    xinput_devices_run_locked(source_job, &op);
}
void xinput_source_reset(void)
{
    source_operation op = {.operation = 3u}; xinput_devices_run_locked(source_job, &op);
}
/* T2121: the lane mutation build (src/host/rust_lane_mutation.c) checks its flipped bytes at every poll. */
void tsfp_lane_mutation_poll(void) __attribute__((weak));
/* T3437: the Bevy shell's replay input lockstep on the guest thread (crates/tsfp-bevy/src/wi.rs). A replayed
 * tick's rows must be queued before this poll drains the queue, else the poll saw them a frame late whenever
 * Bevy ran behind the guest (wall-clock race, MEASURED). Called with no lock held; absent outside the Bevy host. */
void tsfp_replay_guest_poll_sync(void) __attribute__((weak));
bool xinput_source_poll_port(unsigned port)
{
    const bool time_poll = t3615_timing_enabled;
    uint64_t started = time_poll ? t3615_monotonic_ns() : 0u;
    if (tsfp_replay_guest_poll_sync != NULL) tsfp_replay_guest_poll_sync();
    if (tsfp_lane_mutation_poll != NULL && port == 0u) tsfp_lane_mutation_poll();
    const uint64_t sync_done = time_poll ? t3615_monotonic_ns() : 0u;
    source_operation op = {.operation = 4u, .port = port}; xinput_devices_run_locked(source_job, &op);
    const uint64_t poll_done = time_poll ? t3615_monotonic_ns() : 0u;
    if (port < XINPUT_PORT_COUNT) {
#ifdef TSFP_INPUT_HLE_LINKED
        tsfp_input_host_notify_poll_observer(port, tsfp_input_host_poll_count(port));
#else
        xinput_poll_observer_fn observer = g_poll_observer;
        if (observer != NULL) observer(port, xinput_source_port_poll_count(port), g_poll_observer_user);
#endif
    }
    if (time_poll) {
        const uint64_t ended = t3615_monotonic_ns();
        const uint64_t sync_ns = sync_done >= started ? sync_done - started : 0u;
        const uint64_t poll_ns = poll_done >= sync_done ? poll_done - sync_done : 0u;
        const uint64_t total_ns = ended >= started ? ended - started : 0u;
        atomic_fetch_add_explicit(&t3615_timing_calls, 1u, memory_order_relaxed);
        atomic_fetch_add_explicit(&t3615_timing_sync_ns, sync_ns, memory_order_relaxed);
        atomic_fetch_add_explicit(&t3615_timing_poll_ns, poll_ns, memory_order_relaxed);
        atomic_fetch_add_explicit(&t3615_timing_total_ns, total_ns, memory_order_relaxed);
        t3615_atomic_max(&t3615_timing_sync_max_ns, sync_ns);
        t3615_atomic_max(&t3615_timing_poll_max_ns, poll_ns);
        t3615_atomic_max(&t3615_timing_total_max_ns, total_ns);
    }
    return op.result;
}
bool xinput_feedback_send(unsigned port, uint16_t left, uint16_t right)
{
    source_operation op = {.operation = 5u, .port = port, .left = left, .right = right};
    xinput_devices_run_locked(source_job, &op); return op.result;
}

static void fail(char *error, size_t size, unsigned line, const char *what, const char *token)
{
    if (error != NULL && size != 0u) snprintf(error, size, "line %u: %s '%s'", line, what, token);
}
static bool parse_number(const char *text, long min, long max, long *out)
{
    char *end = NULL;
    errno = 0;
    const long value = strtol(text, &end, 10);
    if (errno != 0 || end == text || *end != '\0' || value < min || value > max) return false;
    *out = value;
    return true;
}
static bool apply_token(xinput_pad_state *state, char *token, unsigned line, char *error, size_t size)
{
    static const struct { const char *name; uint16_t bit; } digital[] = {
        {"UP", XINPUT_BUTTON_DPAD_UP}, {"DOWN", XINPUT_BUTTON_DPAD_DOWN}, {"LEFT", XINPUT_BUTTON_DPAD_LEFT},
        {"RIGHT", XINPUT_BUTTON_DPAD_RIGHT}, {"START", XINPUT_BUTTON_START}, {"BACK", XINPUT_BUTTON_BACK},
        {"LTHUMB", XINPUT_BUTTON_LEFT_THUMB}, {"RTHUMB", XINPUT_BUTTON_RIGHT_THUMB},
    };
    static const char *const analog[XINPUT_ANALOG_COUNT] = {"A", "B", "X", "Y", "BLACK", "WHITE", "LT", "RT"};
    static const char *const axes[4] = {"LX", "LY", "RX", "RY"};
    char *equals = strchr(token, '=');
    const char *value_text = NULL;
    if (equals != NULL) { *equals = '\0'; value_text = equals + 1; }
    for (char *c = token; *c != '\0'; c++) *c = (char)toupper((unsigned char)*c);
    if (value_text == NULL) {
        for (size_t i = 0u; i < sizeof(digital) / sizeof(digital[0]); i++)
            if (strcmp(token, digital[i].name) == 0) { state->digital_buttons |= digital[i].bit; return true; }
        for (unsigned i = 0u; i < 6u; i++)
            if (strcmp(token, analog[i]) == 0) { state->analog[i] = 255u; return true; }
        fail(error, size, line, "unknown token", token);
        return false;
    }
    long value = 0;
    for (unsigned i = 0u; i < XINPUT_ANALOG_COUNT; i++)
        if (strcmp(token, analog[i]) == 0) {
            if (!parse_number(value_text, 0, 255, &value)) { fail(error, size, line, "analog value not 0..255", value_text); return false; }
            state->analog[i] = (uint8_t)value;
            return true;
        }
    int16_t *const targets[4] = {&state->thumb_left_x, &state->thumb_left_y, &state->thumb_right_x, &state->thumb_right_y};
    for (unsigned i = 0u; i < 4u; i++)
        if (strcmp(token, axes[i]) == 0) {
            if (!parse_number(value_text, -32768, 32767, &value)) { fail(error, size, line, "axis value not -32768..32767", value_text); return false; }
            *targets[i] = (int16_t)value;
            return true;
        }
    fail(error, size, line, "unknown token", token);
    return false;
}

/* T3926 slice 9: the script and live parsers live in Rust (script.rs); TSFP_RUST_INPUT_HLE=0 keeps the C ones below. */
/* tsfp_input_script_from_entries reads the C entry array as the Rust mirror struct. */
static void tsfp_script_entry_layout_check(void)
{
    _Static_assert(sizeof(xinput_pad_state) == sizeof(tsfp_pad_state), "pad state layout");
    _Static_assert(sizeof(xinput_script_entry) == 4u + sizeof(tsfp_pad_state), "script entry layout");
}
static void pad_from_rust(xinput_pad_state *out, const tsfp_pad_state *in)
{
    memset(out, 0, sizeof(*out));
    out->digital_buttons = in->digital_buttons;
    memcpy(out->analog, in->analog, sizeof(out->analog));
    out->thumb_left_x = in->thumbs[0]; out->thumb_left_y = in->thumbs[1];
    out->thumb_right_x = in->thumbs[2]; out->thumb_right_y = in->thumbs[3];
}

xinput_script *xinput_script_parse(const char *text, size_t len, char *error, size_t error_size)
{
    if (error != NULL && error_size != 0u) error[0] = '\0';
    xinput_script *script = calloc(1u, sizeof(*script));
    if (script == NULL) return NULL;
    if (COUNTERS_IN_RUST()) {
        script->rust = tsfp_input_script_parse((const uint8_t *)text, len, error, error_size);
        if (script->rust == NULL) { free(script); return NULL; }
        return script;
    }
    unsigned line_number = 0u;
    size_t at = 0u;
    while (at < len) {
        size_t end = at;
        while (end < len && text[end] != '\n') end++;
        char line[512];
        const size_t n = end - at;
        line_number++;
        if (n >= sizeof(line)) { fail(error, error_size, line_number, "line too long", ""); goto bad; }
        memcpy(line, text + at, n);
        line[n] = '\0';
        at = end + 1u;
        char *hash = strchr(line, '#');
        if (hash != NULL) *hash = '\0';
        char *save = NULL;
        char *first = strtok_r(line, " \t\r", &save);
        if (first == NULL) continue;
        long frames = 0;
        if (!parse_number(first, 1, 1000000, &frames)) { fail(error, error_size, line_number, "frame count not 1..1000000", first); goto bad; }
        xinput_pad_state state;
        memset(&state, 0, sizeof(state));
        for (char *token = strtok_r(NULL, " \t\r", &save); token != NULL; token = strtok_r(NULL, " \t\r", &save))
            if (!apply_token(&state, token, line_number, error, error_size)) goto bad;
        xinput_script_entry *grown = realloc(script->entries, (script->count + 1u) * sizeof(*grown));
        if (grown == NULL) goto bad;
        script->entries = grown;
        script->entries[script->count].frames = (uint32_t)frames;
        script->entries[script->count].state = state;
        script->count++;
        script->total += (uint64_t)frames;
    }
    return script;
bad:
    xinput_script_free(script);
    return NULL;
}

xinput_script *xinput_script_from_entries(const xinput_script_entry *entries, size_t count)
{
    xinput_script *script = calloc(1u, sizeof(*script));
    if (script == NULL) return NULL;
    if (COUNTERS_IN_RUST()) {
        tsfp_script_entry_layout_check();
        script->rust = tsfp_input_script_from_entries(entries, count);
        if (script->rust == NULL) { free(script); return NULL; }
        return script;
    }
    script->entries = calloc(count ? count : 1u, sizeof(*script->entries));
    if (script->entries == NULL) { free(script); return NULL; }
    for (size_t i = 0u; i < count; i++) { script->entries[i] = entries[i]; script->total += entries[i].frames; }
    script->count = count;
    return script;
}

xinput_script *xinput_script_load(const char *path, char *error, size_t error_size)
{
    FILE *file = fopen(path, "rb");
    if (file == NULL) {
        if (error != NULL && error_size != 0u) snprintf(error, error_size, "cannot open %s", path);
        return NULL;
    }
    char *buffer = NULL;
    size_t size = 0u, used = 0u;
    for (;;) {
        if (used == size) {
            size = size ? size * 2u : 4096u;
            char *grown = realloc(buffer, size);
            if (grown == NULL) { free(buffer); fclose(file); return NULL; }
            buffer = grown;
        }
        const size_t got = fread(buffer + used, 1u, size - used, file);
        used += got;
        if (got == 0u) break;
    }
    fclose(file);
    xinput_script *script = xinput_script_parse(buffer, used, error, error_size);
    free(buffer);
    return script;
}

void xinput_script_free(xinput_script *script)
{
    if (script == NULL) return;
    if (script->rust != NULL) tsfp_input_script_free(script->rust);
    free(script->entries);
    free(script);
}
size_t xinput_script_entry_count(const xinput_script *script)
{
    if (script == NULL) return 0u;
    return script->rust != NULL ? tsfp_input_script_entry_count(script->rust) : script->count;
}
uint64_t xinput_script_total_frames(const xinput_script *script)
{
    if (script == NULL) return 0u;
    return script->rust != NULL ? tsfp_input_script_total_frames(script->rust) : script->total;
}
xinput_pad_state xinput_script_state_at(const xinput_script *script, uint64_t poll_index)
{
    xinput_pad_state rest;
    memset(&rest, 0, sizeof(rest));
    if (script == NULL) return rest;
    if (script->rust != NULL) {
        tsfp_pad_state state;
        tsfp_input_script_state_at(script->rust, poll_index, &state);
        pad_from_rust(&rest, &state);
        return rest;
    }
    uint64_t base = 0u;
    for (size_t i = 0u; i < script->count; i++) {
        if (poll_index < base + script->entries[i].frames) return script->entries[i].state;
        base += script->entries[i].frames;
    }
    return rest;
}
static bool script_source(uint64_t poll_index, xinput_pad_state *out, void *user)
{
    *out = xinput_script_state_at((const xinput_script *)user, poll_index);
    return true;
}
void xinput_script_install(const xinput_script *script) { xinput_source_install(script_source, (void *)script); }

/* T1222: live pad. The file holds ONE line of tokens (the script syntax without the frame count), re-read at every poll,
 * so a controller process can change the pad while the title runs. A line that does not parse leaves the previous state
 * (counted in `refused`), an empty file is the pad at rest. FABRICATED input like the script. */
struct xinput_live {
    int fd;
    xinput_pad_state last;
    uint64_t refused;
    tsfp_input_live *rust; /* non-NULL: Rust owns `last` and `refused` */
};
bool xinput_live_parse_line(const char *text, size_t len, xinput_pad_state *out)
{
    if (COUNTERS_IN_RUST()) {
        tsfp_pad_state state;
        if (!tsfp_input_live_parse_line((const uint8_t *)text, len, &state)) return false;
        pad_from_rust(out, &state);
        return true;
    }
    char line[512];
    size_t n = 0u;
    while (n < len && n < sizeof(line) - 1u && text[n] != '\n' && text[n] != '\0') n++;
    memcpy(line, text, n);
    line[n] = '\0';
    char *hash = strchr(line, '#');
    if (hash != NULL) *hash = '\0';
    xinput_pad_state state;
    memset(&state, 0, sizeof(state));
    char *save = NULL;
    for (char *token = strtok_r(line, " \t\r", &save); token != NULL; token = strtok_r(NULL, " \t\r", &save))
        if (!apply_token(&state, token, 1u, NULL, 0u)) return false;
    *out = state;
    return true;
}
xinput_live *xinput_live_open(const char *path, char *error, size_t error_size)
{
    const int fd = open(path, O_RDONLY | O_CLOEXEC);
    if (fd < 0) {
        if (error != NULL && error_size != 0u) snprintf(error, error_size, "cannot open %s", path);
        return NULL;
    }
    xinput_live *live = calloc(1u, sizeof(*live));
    if (live == NULL) { close(fd); return NULL; }
    live->fd = fd;
    if (COUNTERS_IN_RUST()) live->rust = tsfp_input_live_create();
    return live;
}
void xinput_live_free(xinput_live *live)
{
    if (live == NULL) return;
    close(live->fd);
    if (live->rust != NULL) tsfp_input_live_free(live->rust);
    free(live);
}
uint64_t xinput_live_refused(const xinput_live *live)
{
    if (live == NULL) return 0u;
    return live->rust != NULL ? tsfp_input_live_refused(live->rust) : live->refused;
}
static bool live_source(uint64_t poll_index, xinput_pad_state *out, void *user)
{
    (void)poll_index;
    xinput_live *live = user;
    char buffer[512];
    const ssize_t got = pread(live->fd, buffer, sizeof(buffer), 0);
    if (live->rust != NULL) {
        tsfp_pad_state state;
        tsfp_input_live_poll(live->rust, (const uint8_t *)buffer, (ptrdiff_t)got, &state);
        pad_from_rust(out, &state);
        return true;
    }
    if (got < 0) { live->refused++; *out = live->last; return true; }
    xinput_pad_state state;
    if (xinput_live_parse_line(buffer, (size_t)got, &state)) live->last = state;
    else live->refused++;
    *out = live->last;
    return true;
}
void xinput_live_install(xinput_live *live) { xinput_source_install(live_source, live); }
