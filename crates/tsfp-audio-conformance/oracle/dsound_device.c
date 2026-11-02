/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "dsound_device.h"
#include <pthread.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#include "dsound_hle.h"
#include "guest_mem.h"
#include "dsound_audio_guest_heap.h"
#include "kernel_call.h"

#define CREATE_ENTRY 0x00409635u
#define CREATE_CALLER 0x000279D5u
/* T421: the XMV GetNextFrame returns here from its DirectSoundCreate and device Release calls. */
#define RELEASE_ENTRY 0x00406A8Au
#define MOVIE_CREATE_CALLER 0x0044570Bu
#define MOVIE_RELEASE_CALLER 0x00445726u
#ifdef TSFP_RUST_AUDIO_HOST
extern size_t tsfp_audio_movie_device_register_release_rust(void);
extern bool tsfp_audio_movie_release_frame_snapshot(void *context, int32_t *host_pid_out,
                                                    uint32_t *stack_address_out,
                                                    uint32_t *stack_limit_out);
extern void tsfp_audio_movie_release_refuse(uint32_t reason) __attribute__((noreturn));
#endif
/* The original device Release that would leave this many references destroys the device: five belong
 * to its own children and the sixth to the title. Measured in tests/test_dsound_device_calls_oracle.py. */
#define DESTROY_REFERENCES 6u
#define SINGLETON 0x00412B30u
#define DEVICE_VTABLE 0x004A1CB0u
#define DEVICE_BYTES 44u
#define E_OUTOFMEMORY 0x8007000Eu
#define DSERR_NODRIVER 0x88780078u
#ifndef TSFP_RUST_AUDIO_HOST
static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
static uint32_t owned_heap, owned_address, shadow_reference;
typedef struct lease_record {
    dsound_device_lease token;
    dsound_device_lease_child child;
    struct lease_record *next;
} lease_record;
static lease_record *leases;
static size_t lease_count;
/* Survives reset; refuses exhaustion rather than reissuing an identity. */
static uint64_t lease_serial;
static bool announced;
static dsound_device_fatal_fn fatal_handler;
static dsound_device_create_note_fn create_note;
void dsound_device_set_create_note(dsound_device_create_note_fn note)
{pthread_mutex_lock(&lock);create_note=note;pthread_mutex_unlock(&lock);}

/* T421: references the XMV movie calls hold (second DirectSoundCreate, not yet Released). */
static bool movie_calls_enabled;
static uint32_t movie_references;

static void refuse_at(uint32_t entry, const char *message) __attribute__((noreturn));
static void refuse_at(uint32_t entry, const char *message)
{
    dsound_hle_log()("dsound SILENT facade: %#x refused: %s\n", entry, message);
    if (fatal_handler != NULL) fatal_handler(entry, message);
    abort();
}
static void refuse(const char *message) __attribute__((noreturn));
static void refuse(const char *message) { refuse_at(CREATE_ENTRY, message); }
static void refuse_locked(const char *message) __attribute__((noreturn));
static void refuse_locked(const char *message)
{ pthread_mutex_unlock(&lock); refuse(message); }
static void refuse_locked_at(uint32_t entry, const char *message) __attribute__((noreturn));
static void refuse_locked_at(uint32_t entry, const char *message)
{ pthread_mutex_unlock(&lock); refuse_at(entry, message); }
void dsound_device_set_fatal(dsound_device_fatal_fn handler) { fatal_handler = handler; }
static bool overlaps(uint32_t a, uint32_t size_a, uint32_t b, uint32_t size_b)
{
    return (uint64_t)a < (uint64_t)b + size_b && (uint64_t)b < (uint64_t)a + size_a;
}
static void header_words(uint32_t address, uint32_t reference, uint32_t words[11])
{
    memset(words, 0, DEVICE_BYTES);
    words[0] = DEVICE_VTABLE;
    words[1] = reference;
    words[4] = address + 16u;
    words[5] = address + 16u;
}
static bool validate_owned(uint32_t interface)
{
    uint32_t singleton, requested, expected[11], actual[11];
    if (owned_address == 0u || interface != owned_address + 8u || shadow_reference == 0u ||
        !kernel_guest_read_bytes(owned_address, actual, sizeof(actual)) ||
        !kernel_guest_read_u32(SINGLETON, &singleton) || singleton != owned_address ||
        !dsound_audio_heap_valid(owned_heap) ||
        !dsound_audio_heap_block_size(owned_heap, owned_address, &requested) ||
        requested != DEVICE_BYTES) return false;
    header_words(owned_address, shadow_reference, expected);
    return memcmp(actual, expected, sizeof(actual)) == 0;
}
bool dsound_device_with_owned_interface(uint32_t interface, dsound_owned_interface_fn callback,
                                        void *userdata, uint32_t *result)
{
    pthread_mutex_lock(&lock);
    const bool valid = callback != NULL && result != NULL &&
        shadow_reference != UINT32_MAX && validate_owned(interface);
    if (valid) callback(owned_address, userdata, result);
    pthread_mutex_unlock(&lock);
    return valid;
}
bool dsound_device_with_owned_identity(uint32_t interface, dsound_owned_identity_fn callback,
                                      void *userdata, uint32_t *result)
{
    pthread_mutex_lock(&lock);
    const bool valid = callback != NULL && result != NULL &&
        shadow_reference != UINT32_MAX && validate_owned(interface);
    if (valid) {
        const dsound_device_identity identity = {owned_heap, owned_address};
        callback(&identity, userdata, result);
    }
    pthread_mutex_unlock(&lock);
    return valid;
}
static bool valid_ops(const dsound_device_lease_ops *ops)
{ return ops != NULL && ops->prepare != NULL && ops->abort != NULL && ops->finalize != NULL; }
static bool valid_child(const dsound_device_lease_child *child)
{
    uint32_t bytes;
    return child->heap != 0u && child->heap != owned_heap && child->address != 0u &&
        child->bytes != 0u && (uint64_t)child->address + child->bytes <= UINT64_C(0x100000000) &&
        dsound_audio_heap_valid(child->heap) &&
        dsound_audio_heap_block_size(child->heap, child->address, &bytes) && bytes == child->bytes &&
        !overlaps(child->address, child->bytes, owned_address, DEVICE_BYTES);
}
static bool same_token(const dsound_device_lease *a, const dsound_device_lease *b)
{ return a->device_heap == b->device_heap && a->internal_address == b->internal_address &&
         a->serial == b->serial; }
static bool same_child(const dsound_device_lease_child *a, const dsound_device_lease_child *b)
{ return a->heap == b->heap && a->address == b->address && a->bytes == b->bytes; }
static bool probe_reference(void)
{ return kernel_guest_write_u32(owned_address + 4u, shadow_reference); }
dsound_device_lease_status dsound_device_acquire_lease(uint32_t interface,
    const dsound_device_lease_ops *ops, void *userdata, dsound_device_lease *output)
{
    dsound_device_lease_status status = DSOUND_LEASE_INVALID;
    pthread_mutex_lock(&lock);
    if (!valid_ops(ops) || output == NULL || !validate_owned(interface)) goto done;
    if (shadow_reference == UINT32_MAX) { status = DSOUND_LEASE_REFERENCE_LIMIT; goto done; }
    if (lease_serial == UINT64_MAX) { status = DSOUND_LEASE_SERIAL_LIMIT; goto done; }
    if (!probe_reference()) { status = DSOUND_LEASE_WRITE_REFUSED; goto done; }
    lease_record *record = malloc(sizeof(*record));
    if (record == NULL) { status = DSOUND_LEASE_OUT_OF_MEMORY; goto done; }
    record->child = (dsound_device_lease_child){0};
    record->token = (dsound_device_lease){owned_heap, owned_address, lease_serial + 1u};
    if (!ops->prepare(&record->token, userdata, &record->child)) {
        status = DSOUND_LEASE_PREPARE_FAILED; goto abort_prepare;
    }
    if (!valid_child(&record->child)) goto abort_prepare;
    for (lease_record *other = leases; other != NULL; other = other->next)
        if (same_child(&other->child, &record->child)) goto abort_prepare;
    /* No callback may change the device or its heap. Validate again before commit. */
    if (!validate_owned(interface)) goto abort_prepare;
    if (!kernel_guest_write_u32(owned_address + 4u, shadow_reference + 1u)) {
        status = DSOUND_LEASE_WRITE_REFUSED; goto abort_prepare;
    }
    shadow_reference++;
    lease_serial = record->token.serial;
    record->next = leases; leases = record; lease_count++;
    ops->finalize(&record->token, userdata);
    *output = record->token;
    status = DSOUND_LEASE_OK;
    goto done;
abort_prepare:
    ops->abort(userdata);
    free(record);
done:
    pthread_mutex_unlock(&lock);
    return status;
}
dsound_device_lease_status dsound_device_release_lease(const dsound_device_lease *lease,
    const dsound_device_lease_ops *ops, void *userdata)
{
    dsound_device_lease_status status = DSOUND_LEASE_INVALID;
    pthread_mutex_lock(&lock);
    if (lease == NULL || !valid_ops(ops)) goto done;
    lease_record **link = &leases;
    while (*link != NULL && !same_token(lease, &(*link)->token)) link = &(*link)->next;
    lease_record *record = *link;
    if (record == NULL || record->token.device_heap != owned_heap ||
        record->token.internal_address != owned_address ||
        !validate_owned(owned_address + 8u) || !valid_child(&record->child) ||
        shadow_reference <= lease_count) goto done;
    if (!probe_reference()) { status = DSOUND_LEASE_WRITE_REFUSED; goto done; }
    dsound_device_lease_child child = {0};
    if (!ops->prepare(&record->token, userdata, &child)) {
        status = DSOUND_LEASE_PREPARE_FAILED; goto abort_prepare;
    }
    if (!same_child(&child, &record->child) || !valid_child(&child) ||
        !validate_owned(owned_address + 8u)) goto abort_prepare;
    if (!kernel_guest_write_u32(owned_address + 4u, shadow_reference - 1u)) {
        status = DSOUND_LEASE_WRITE_REFUSED; goto abort_prepare;
    }
    shadow_reference--; *link = record->next; lease_count--;
    ops->finalize(&record->token, userdata);
    free(record);
    status = DSOUND_LEASE_OK;
    goto done;
abort_prepare:
    ops->abort(userdata);
done:
    pthread_mutex_unlock(&lock);
    return status;
}
bool dsound_device_reset_checked(void)
{
    pthread_mutex_lock(&lock);
    if (lease_count != 0u) { pthread_mutex_unlock(&lock); return false; }
    if (owned_address != 0u && dsound_audio_heap_valid(owned_heap)) {
        /* A live but changed allocation/header/singleton may be foreign state.
         * Keep both guest state and sidecar intact so reset is reviewable/retryable. */
        if (!validate_owned(owned_address + 8u) ||
            !kernel_guest_write_u32(SINGLETON, 0u)) {
            pthread_mutex_unlock(&lock); return false;
        }
        /* Quiescent heap ownership makes this destruction deterministic. */
        if (!dsound_audio_heap_destroy(owned_heap)) {
            (void)kernel_guest_write_u32(SINGLETON, owned_address);
            pthread_mutex_unlock(&lock); return false;
        }
    }
    owned_heap = owned_address = shadow_reference = 0u;
    movie_references = 0u;
    announced = false;
    pthread_mutex_unlock(&lock);
    return true;
}
void dsound_device_reset(void) { (void)dsound_device_reset_checked(); }
static uint32_t create_device(uint32_t guid, uint32_t output, uint32_t outer, bool movie)
{
    pthread_mutex_lock(&lock);
    if (guid != 0u || outer != 0u) refuse_locked("only NULL GUID/outer are recovered");
    if (kernel_guest_at_ro(output, 4u) == NULL || kernel_guest_at_ro(SINGLETON, 4u) == NULL ||
        kernel_guest_at_ro(DEVICE_VTABLE, 12u) == NULL)
        refuse_locked("output, singleton or original vtable is not mapped");
    if (overlaps(output, 4u, SINGLETON, 4u) || overlaps(output, 4u, DEVICE_VTABLE, 12u) ||
        (owned_address != 0u && overlaps(output, 4u, owned_address, DEVICE_BYTES)))
        refuse_locked("output alias with device state is not recovered");
    uint32_t singleton;
    if (!kernel_guest_read_u32(SINGLETON, &singleton)) refuse_locked("singleton is unreadable");
    uint32_t status = 0u;
    if (movie && owned_address == 0u)
        refuse_locked("a movie DirectSoundCreate needs the device the startup already created");
    if (owned_address != 0u) {
        if (!validate_owned(owned_address + 8u) || shadow_reference == UINT32_MAX)
            refuse_locked("cached singleton/header/heap generation/reference changed");
        if (movie && movie_references == UINT32_MAX)
            refuse_locked("movie reference count would overflow");
        /* Cached success remains independent of codec readiness. Use a guarded
         * ref write and update its shadow only after the guest write succeeds. */
        if (!kernel_guest_write_u32(owned_address + 4u, shadow_reference + 1u))
            refuse_locked("cached reference is not writable");
        shadow_reference++;
        if (movie) movie_references++;
    } else {
        if (singleton != 0u) refuse_locked("existing device is not owned by this facade");
        const uint32_t heap = dsound_audio_heap_create(0u, GUEST_HEAP_CHUNK_MIN, 0u);
        if (heap == 0u) { pthread_mutex_unlock(&lock); return E_OUTOFMEMORY; }
        const uint32_t address = dsound_audio_heap_alloc(heap, DEVICE_BYTES);
        if (address == 0u) {
            (void)dsound_audio_heap_destroy(heap);
            pthread_mutex_unlock(&lock);
            return E_OUTOFMEMORY;
        }
        if (kernel_guest_at_ro(address, DEVICE_BYTES) == NULL) {
            (void)dsound_audio_heap_destroy(heap);
            refuse_locked("new owned device is not mapped");
        }
        if(create_note!=NULL && !create_note(heap,address)) {
            const bool freed=dsound_audio_heap_destroy(heap);
            refuse_locked(freed?"PCM epoch rejected retained audio or inactive renderer":
                                  "PCM epoch rejected and unpublished allocation rollback failed");
        }
        /* These reference values are original observations. The omitted internal
         * references/buffers are NOT constructed by this SILENT policy. */
        const bool codec_ready = dsound_hle_codec_ready();
        owned_heap = heap;
        owned_address = address;
        shadow_reference = codec_ready ? 6u : 1u;
        uint32_t words[11];
        header_words(address, shadow_reference, words);
        if (!kernel_guest_write_bytes(address, words, DEVICE_BYTES) || !kernel_guest_write_u32(SINGLETON, address))
            refuse_locked("new owned device or singleton is not writable");
        if (!codec_ready) status = DSERR_NODRIVER;
    }
    if (status == 0u) {
        const uint32_t interface = owned_address + 8u;
        (void)kernel_guest_write_u32(output, interface);
    }
    const bool announce = !announced;
    announced = true;
    pthread_mutex_unlock(&lock);
    if (announce)
        dsound_hle_log()("dsound: explicit SILENT public-device facade; omitted B4/540/APU, "
            "IRQ/DPC/timer, silent-buffer and guest critical-section internals; "
            "no audio output or DSP acknowledgement; nested fields remain unsupported\n");
    return status;
}
uint32_t dsound_device_create(uint32_t guid, uint32_t output, uint32_t outer)
{ return create_device(guid, output, outer, false); }
void dsound_device_set_movie_calls(bool enabled)
{ pthread_mutex_lock(&lock); movie_calls_enabled = enabled; pthread_mutex_unlock(&lock); }
bool dsound_device_movie_calls_enabled(void)
{
    pthread_mutex_lock(&lock);
    const bool value = movie_calls_enabled;
    pthread_mutex_unlock(&lock);
    return value;
}
uint32_t dsound_device_movie_references(void)
{
    pthread_mutex_lock(&lock);
    const uint32_t value = movie_references;
    pthread_mutex_unlock(&lock);
    return value;
}
uint32_t dsound_device_release_movie(uint32_t interface)
{
    pthread_mutex_lock(&lock);
    if (!movie_calls_enabled) refuse_locked_at(RELEASE_ENTRY, "movie device calls are disabled");
    if (shadow_reference == UINT32_MAX || !validate_owned(interface))
        refuse_locked_at(RELEASE_ENTRY, "Release argument is not the owned device interface");
    if (movie_references == 0u)
        refuse_locked_at(RELEASE_ENTRY, "no movie reference is outstanding, releasing the startup "
                                        "reference would run the original device destructor");
    /* The original destroys the device when the count would fall to DESTROY_REFERENCES - 1. Every stream
     * lease and movie reference sits above the base count, so this also keeps them. */
    if (shadow_reference - 1u < DESTROY_REFERENCES)
        refuse_locked_at(RELEASE_ENTRY, "Release would reach the original device destructor");
    if (!kernel_guest_write_u32(owned_address + 4u, shadow_reference - 1u))
        refuse_locked_at(RELEASE_ENTRY, "device reference is not writable");
    shadow_reference--;
    movie_references--;
    const uint32_t result = shadow_reference;
    pthread_mutex_unlock(&lock);
    return result;
}
#else
/* Rust audio host (T2046): the device record, reference count, lease ledger and movie references live in
 * tsfp_audio::lifecycle::Device (crates/tsfp-audio-capi/src/device_ffi.rs). C keeps the fatal handler, the log
 * and the frame decoding. */
typedef struct rust_reason { const uint8_t *text; size_t len; } rust_reason;
typedef struct rust_create_outcome { uint32_t status; bool announce; rust_reason reason; } rust_create_outcome;
extern bool tsfp_audio_device_create(uint32_t guid, uint32_t output, uint32_t outer, bool movie, bool codec_ready,
                                     dsound_device_create_note_fn note, rust_create_outcome *outcome);
extern bool tsfp_audio_device_release_movie(uint32_t interface, uint32_t *result, rust_reason *reason);
typedef void (*rust_owned_fn)(const uint32_t *words, void *userdata, uint32_t *result);
extern bool tsfp_audio_device_with_owned_interface(uint32_t interface, rust_owned_fn callback, void *userdata,
                                                   uint32_t *result);
extern bool tsfp_audio_device_with_owned_identity(uint32_t interface, rust_owned_fn callback, void *userdata,
                                                  uint32_t *result);
extern int32_t tsfp_audio_device_acquire_lease(uint32_t interface, const dsound_device_lease_ops *ops,
                                               void *userdata, dsound_device_lease *output);
extern int32_t tsfp_audio_device_release_lease(const dsound_device_lease *lease,
                                               const dsound_device_lease_ops *ops, void *userdata);
extern bool tsfp_audio_device_reset_checked(void);
extern void tsfp_audio_device_set_movie_calls(bool enabled);
extern bool tsfp_audio_device_movie_calls_enabled(void);
extern uint32_t tsfp_audio_device_movie_references(void);
static dsound_device_fatal_fn fatal_handler;
static dsound_device_create_note_fn create_note;
void dsound_device_set_create_note(dsound_device_create_note_fn note) { create_note = note; }
void dsound_device_set_fatal(dsound_device_fatal_fn handler) { fatal_handler = handler; }
static void refuse_at(uint32_t entry, const char *message) __attribute__((noreturn));
static void refuse_at(uint32_t entry, const char *message)
{
    dsound_hle_log()("dsound SILENT facade: %#x refused: %s\n", entry, message);
    if (fatal_handler != NULL) fatal_handler(entry, message);
    abort();
}
static void refuse(const char *message) __attribute__((noreturn));
static void refuse(const char *message) { refuse_at(CREATE_ENTRY, message); }
static void refuse_reason_at(uint32_t entry, const rust_reason *reason) __attribute__((noreturn));
static void refuse_reason_at(uint32_t entry, const rust_reason *reason)
{
    char message[256];
    const size_t length = reason->len < sizeof message - 1u ? reason->len : sizeof message - 1u;
    if (reason->text != NULL) memcpy(message, reason->text, length);
    message[reason->text != NULL ? length : 0u] = '\0';
    refuse_at(entry, message);
}
typedef struct owned_trampoline {
    dsound_owned_interface_fn interface_fn;
    dsound_owned_identity_fn identity_fn;
    void *userdata;
} owned_trampoline;
static void interface_trampoline(const uint32_t *words, void *context, uint32_t *result)
{ const owned_trampoline *t = context; t->interface_fn(words[0], t->userdata, result); }
static void identity_trampoline(const uint32_t *words, void *context, uint32_t *result)
{
    const owned_trampoline *t = context;
    const dsound_device_identity identity = {words[0], words[1]};
    t->identity_fn(&identity, t->userdata, result);
}
bool dsound_device_with_owned_interface(uint32_t interface, dsound_owned_interface_fn callback,
                                        void *userdata, uint32_t *result)
{
    if (callback == NULL) return false;
    owned_trampoline t = {callback, NULL, userdata};
    return tsfp_audio_device_with_owned_interface(interface, interface_trampoline, &t, result);
}
bool dsound_device_with_owned_identity(uint32_t interface, dsound_owned_identity_fn callback,
                                      void *userdata, uint32_t *result)
{
    if (callback == NULL) return false;
    owned_trampoline t = {NULL, callback, userdata};
    return tsfp_audio_device_with_owned_identity(interface, identity_trampoline, &t, result);
}
dsound_device_lease_status dsound_device_acquire_lease(uint32_t interface,
    const dsound_device_lease_ops *ops, void *userdata, dsound_device_lease *output)
{ return (dsound_device_lease_status)tsfp_audio_device_acquire_lease(interface, ops, userdata, output); }
dsound_device_lease_status dsound_device_release_lease(const dsound_device_lease *lease,
    const dsound_device_lease_ops *ops, void *userdata)
{ return (dsound_device_lease_status)tsfp_audio_device_release_lease(lease, ops, userdata); }
bool dsound_device_reset_checked(void) { return tsfp_audio_device_reset_checked(); }
void dsound_device_reset(void) { (void)dsound_device_reset_checked(); }
static uint32_t create_device(uint32_t guid, uint32_t output, uint32_t outer, bool movie)
{
    rust_create_outcome outcome;
    if (!tsfp_audio_device_create(guid, output, outer, movie, dsound_hle_codec_ready(), create_note, &outcome))
        refuse_reason_at(CREATE_ENTRY, &outcome.reason);
    if (outcome.announce)
        dsound_hle_log()("dsound: explicit SILENT public-device facade; omitted B4/540/APU, "
            "IRQ/DPC/timer, silent-buffer and guest critical-section internals; "
            "no audio output or DSP acknowledgement; nested fields remain unsupported\n");
    return outcome.status;
}
uint32_t dsound_device_create(uint32_t guid, uint32_t output, uint32_t outer)
{ return create_device(guid, output, outer, false); }
void dsound_device_set_movie_calls(bool enabled) { tsfp_audio_device_set_movie_calls(enabled); }
bool dsound_device_movie_calls_enabled(void) { return tsfp_audio_device_movie_calls_enabled(); }
uint32_t dsound_device_movie_references(void) { return tsfp_audio_device_movie_references(); }
uint32_t dsound_device_release_movie(uint32_t interface)
{
    uint32_t result;
    rust_reason reason;
    if (!tsfp_audio_device_release_movie(interface, &result, &reason))
        refuse_reason_at(RELEASE_ENTRY, &reason);
    return result;
}
/* Host services for the Rust device facade: guest memory and the guest heap. */
bool tsfp_audio_dev_mapped(uint32_t address, uint32_t length)
{ return kernel_guest_at_ro(address, length) != NULL; }
bool tsfp_audio_dev_readable(uint32_t address, uint32_t length)
{ return kernel_guest_range_readable(address, length); }
uint32_t tsfp_audio_dev_heap_create(void)
{ return dsound_audio_heap_create(0u, GUEST_HEAP_CHUNK_MIN, 0u); }
uint32_t tsfp_audio_dev_heap_alloc(uint32_t heap, uint32_t bytes)
{ return dsound_audio_heap_alloc(heap, bytes); }
bool tsfp_audio_dev_heap_destroy(uint32_t heap) { return dsound_audio_heap_destroy(heap); }
bool tsfp_audio_dev_heap_valid(uint32_t heap) { return dsound_audio_heap_valid(heap); }
bool tsfp_audio_dev_heap_block_size(uint32_t heap, uint32_t address, uint32_t *size)
{ return dsound_audio_heap_block_size(heap, address, size); }
#endif
static uint32_t create_handler(void *context)
{
    const kernel_call_frame *frame = context;
    uint32_t caller, arguments[3];
    if (frame == NULL || !kernel_guest_read_u32(frame->stack_ptr, &caller))
        refuse("unreadable Create frame");
    const bool startup = caller == CREATE_CALLER;
    if (!startup && !(caller == MOVIE_CREATE_CALLER && dsound_device_movie_calls_enabled()))
        refuse("only measured startup caller return 0x000279D5 is recovered");
    for (unsigned i = 0u; i < 3u; i++)
        if (!kernel_frame_arg(frame, i, &arguments[i])) refuse("unreadable Create argument");
    return create_device(arguments[0], arguments[1], arguments[2], !startup);
}
#ifndef TSFP_RUST_AUDIO_HOST
static uint32_t release_handler(void *context)
{
    const kernel_call_frame *frame = context;
    uint32_t interface;
    uint32_t caller;
    if (frame == NULL || !kernel_guest_read_u32(frame->stack_ptr, &caller))
        refuse_at(RELEASE_ENTRY, "only the measured movie caller return 0x00445726 of device Release "
                                 "is recovered");
    if (caller != MOVIE_RELEASE_CALLER)
        refuse_at(RELEASE_ENTRY, "only the measured movie caller return 0x00445726 of device Release "
                                 "is recovered");
    if (!kernel_frame_arg(frame, 0u, &interface))
        refuse_at(RELEASE_ENTRY, "unreadable Release argument");
    return dsound_device_release_movie(interface);
}
#endif
size_t dsound_device_register(void)
{ return dsound_hle_register(CREATE_ENTRY, create_handler) ? 1u : 0u; }
size_t dsound_device_register_movie(void)
{
#ifdef TSFP_RUST_AUDIO_HOST
    return tsfp_audio_movie_device_register_release_rust();
#else
    return dsound_hle_register(RELEASE_ENTRY, release_handler) ? 1u : 0u;
#endif
}

#ifdef TSFP_RUST_AUDIO_HOST
bool tsfp_audio_movie_release_frame_snapshot(void *context, int32_t *host_pid_out,
                                             uint32_t *stack_address_out,
                                             uint32_t *stack_limit_out)
{
    const kernel_call_frame *frame = context;
    if (frame == NULL || host_pid_out == NULL || stack_address_out == NULL ||
        stack_limit_out == NULL) return false;
    *host_pid_out = (int32_t)kernel_host_pid();
    *stack_address_out = frame->stack_ptr;
    *stack_limit_out = frame->stack_limit;
    return true;
}

void tsfp_audio_movie_release_refuse(uint32_t reason)
{
    if (reason == 0u)
        refuse_at(RELEASE_ENTRY, "only the measured movie caller return 0x00445726 of device Release "
                                 "is recovered");
    refuse_at(RELEASE_ENTRY, "unreadable Release argument");
}
#endif
