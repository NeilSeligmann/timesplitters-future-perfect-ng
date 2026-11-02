/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "native_scene_bridge.h"

#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>

static atomic_flag scene_lock = ATOMIC_FLAG_INIT;
static _Atomic int scene_enabled;
static uint64_t scene_sequence;
static uint64_t latest_frame_sequence;
static uint64_t draw_sequence;
static uint64_t draw_read_sequence;
static uint64_t active_frame_sequence;
static _Atomic int draw_capture_active;
/* Capture-only override (T3887): TSFP_NATIVE_SCENE_DRAW_SCOPE_PAYLOAD_LIMIT=<bytes>, capped at the
 * global payload limit, keeps whole frames for the offline preview. Default is unchanged. */
static uint32_t scope_payload_limit(void) {
    static uint32_t limit;
    if (limit == 0u) {
        limit = TSFP_NATIVE_SCENE_DRAW_SCOPE_PAYLOAD_LIMIT;
        const char *text = getenv("TSFP_NATIVE_SCENE_DRAW_SCOPE_PAYLOAD_LIMIT");
        if (text != NULL) {
            char *end = NULL;
            const unsigned long long value = strtoull(text, &end, 10);
            if (end != text && *end == '\0' && value != 0ull) {
                limit = value > TSFP_NATIVE_SCENE_DRAW_PAYLOAD_LIMIT
                            ? TSFP_NATIVE_SCENE_DRAW_PAYLOAD_LIMIT
                            : (uint32_t)value;
            }
        }
    }
    return limit;
}

static int draw_capture_scope_open;
static _Atomic uint32_t draw_capture_sample_interval = 1u;
static uint64_t draw_capture_scope_sequence;
static uint32_t draw_capture_scope_packets;
static uint32_t draw_capture_scope_payload_bytes;
static _Atomic uint64_t draw_capture_timed_calls;
static _Atomic uint64_t draw_capture_elapsed_ns;
static _Atomic uint64_t draw_capture_payload_bytes;
static uint64_t unscoped_draws;
static uint64_t dropped_draw_packets;
static uint64_t draw_capture_scope_errors;
static _Atomic uint64_t collector_guest_calls;
static _Atomic uint64_t collector_adapter_entries;
static _Atomic uint64_t collector_accepted;
static _Atomic uint64_t collector_bad_count;
static _Atomic uint64_t collector_bad_order;
static _Atomic int32_t collector_last_room_count;
static _Atomic int32_t collector_last_draw_depth;
static _Thread_local uint32_t active_draw_room = UINT32_MAX;
static _Thread_local uint32_t active_draw_scope_base;
static _Thread_local int active_draw_scope;
struct draw_record {
    uint64_t sequence;
    struct tsfp_native_scene_draw_packet packet;
    uint32_t payload_bytes;
    uint8_t *payload;
};
static struct draw_record draw_queue[TSFP_NATIVE_SCENE_DRAW_CAPACITY];
static uint32_t draw_payload_bytes;
struct scene_publication {
    uint64_t sequence;
    uint32_t kind;
    uint32_t room_count;
    uint32_t object_count;
    uint32_t view_count;
    int32_t draw_depth;
    uint8_t rooms[TSFP_NATIVE_SCENE_ROOM_CAPACITY];
    uint32_t object_records[TSFP_NATIVE_SCENE_OBJECT_CAPACITY];
    struct tsfp_native_scene_view view;
};
static struct scene_publication scene_queue[TSFP_NATIVE_SCENE_QUEUE_CAPACITY];

static void lock_scene(void) {
    while (atomic_flag_test_and_set_explicit(&scene_lock, memory_order_acquire)) {
    }
}

static void unlock_scene(void) {
    atomic_flag_clear_explicit(&scene_lock, memory_order_release);
}

void tsfp_native_scene_bridge_enable(int enabled) {
    atomic_store_explicit(&scene_enabled, enabled != 0, memory_order_release);
    if (!enabled) {
        lock_scene();
        active_frame_sequence = 0;
        latest_frame_sequence = 0;
        atomic_store_explicit(&draw_capture_active, 0, memory_order_release);
        draw_capture_scope_open = 0;
        draw_capture_scope_sequence = 0u;
        draw_capture_scope_packets = 0u;
        draw_capture_scope_payload_bytes = 0u;
        atomic_store_explicit(&draw_capture_timed_calls, 0u, memory_order_relaxed);
        atomic_store_explicit(&draw_capture_elapsed_ns, 0u, memory_order_relaxed);
        atomic_store_explicit(&draw_capture_payload_bytes, 0u, memory_order_relaxed);
        active_draw_room = UINT32_MAX;
        active_draw_scope_base = 0u;
        active_draw_scope = 0;
        for (uint32_t index = 0u; index < TSFP_NATIVE_SCENE_DRAW_CAPACITY; index++) {
            free(draw_queue[index].payload);
            memset(&draw_queue[index], 0, sizeof(draw_queue[index]));
        }
        draw_payload_bytes = 0u;
        draw_sequence = 0u;
        draw_read_sequence = 0u;
        unscoped_draws = 0u;
        dropped_draw_packets = 0u;
        draw_capture_scope_errors = 0u;
        atomic_store_explicit(&collector_guest_calls, 0u, memory_order_relaxed);
        atomic_store_explicit(&collector_adapter_entries, 0u, memory_order_relaxed);
        atomic_store_explicit(&collector_accepted, 0u, memory_order_relaxed);
        atomic_store_explicit(&collector_bad_count, 0u, memory_order_relaxed);
        atomic_store_explicit(&collector_bad_order, 0u, memory_order_relaxed);
        atomic_store_explicit(&collector_last_room_count, 0, memory_order_relaxed);
        atomic_store_explicit(&collector_last_draw_depth, 0, memory_order_relaxed);
        unlock_scene();
    }
}

void tsfp_native_scene_bridge_set_draw_capture_sample_interval(uint32_t interval) {
    atomic_store_explicit(&draw_capture_sample_interval, interval, memory_order_release);
}

int tsfp_native_scene_bridge_capture_enabled(void) {
    return atomic_load_explicit(&scene_enabled, memory_order_acquire) != 0;
}

int tsfp_native_scene_bridge_draw_capture_active(void) {
    return atomic_load_explicit(&scene_enabled, memory_order_acquire) &&
           atomic_load_explicit(&draw_capture_active, memory_order_acquire);
}

int tsfp_native_scene_bridge_draw_capture_budget(uint32_t *payload_remaining) {
    if (payload_remaining != NULL) *payload_remaining = 0u;
    /* This function is called from every D3D draw hook. The active flag is
     * already cleared on disable, so the common unsampled path needs one
     * acquire load and no scene lock. */
    if (!atomic_load_explicit(&draw_capture_active, memory_order_acquire)) return 0;
    if (!atomic_load_explicit(&scene_enabled, memory_order_acquire)) return 0;
    lock_scene();
    const int available = draw_capture_scope_packets < TSFP_NATIVE_SCENE_DRAW_SCOPE_PACKET_LIMIT;
    if (available && payload_remaining != NULL) {
        *payload_remaining = scope_payload_limit() -
                             draw_capture_scope_payload_bytes;
    } else if (!available) {
        ++dropped_draw_packets;
    }
    unlock_scene();
    return available;
}

void tsfp_native_scene_bridge_begin_draw_capture(void) {
    if (!atomic_load_explicit(&scene_enabled, memory_order_acquire)) {
        return;
    }
    lock_scene();
    if (draw_capture_scope_open) {
        ++draw_capture_scope_errors;
        active_frame_sequence = 0;
    } else {
        /* Retirement messages share scene_sequence but do not identify a room
         * draw plan. Keep packets tied to the most recently published frame. */
        active_frame_sequence = latest_frame_sequence;
    }
    ++draw_capture_scope_sequence;
    const uint32_t interval = atomic_load_explicit(&draw_capture_sample_interval, memory_order_acquire);
    /* Do not put the first full packet capture on the intro/movie frame. The
     * first sample occurs only after a complete interval of collector scopes. */
    const int sample = interval != 0u && draw_capture_scope_sequence % interval == 0u;
    atomic_store_explicit(&draw_capture_active, sample, memory_order_release);
    draw_capture_scope_open = 1;
    draw_capture_scope_packets = 0u;
    draw_capture_scope_payload_bytes = 0u;
    active_draw_room = 0u;
    active_draw_scope_base = 0u;
    active_draw_scope = 0;
    unlock_scene();
}

void tsfp_native_scene_bridge_end_draw_capture(void) {
    if (!atomic_load_explicit(&scene_enabled, memory_order_acquire)) {
        return;
    }
    lock_scene();
    if (!draw_capture_scope_open) {
        ++draw_capture_scope_errors;
    }
    active_frame_sequence = 0;
    atomic_store_explicit(&draw_capture_active, 0, memory_order_release);
    draw_capture_scope_open = 0;
    active_draw_room = UINT32_MAX;
    active_draw_scope_base = 0u;
    active_draw_scope = 0;
    unlock_scene();
}

uint32_t tsfp_native_scene_bridge_current_draw_room(void) {
    return active_draw_room;
}

/* RECOMP_ABI_CALL reaches lifted C directly. Its cooperative safepoint runs
 * immediately before each call, so use stack depth to keep the room key live
 * through descendants and retire it when control returns to the caller. */
void tsfp_native_scene_bridge_note_call(uint32_t callee, uint32_t stack_pointer,
                                        uint32_t room_argument) {
    /* The host invokes this observer at every lifted ABI safepoint (~30k per
     * frame). Keep the ordinary/off path to one atomic read and no stores. */
    if (!atomic_load_explicit(&draw_capture_active, memory_order_acquire)) {
        if (callee == 0x00086C40u &&
            atomic_load_explicit(&scene_enabled, memory_order_acquire)) {
            atomic_fetch_add_explicit(&collector_guest_calls, 1u, memory_order_relaxed);
        }
        return;
    }
    if (callee == 0x00086C40u)
        atomic_fetch_add_explicit(&collector_guest_calls, 1u, memory_order_relaxed);
    if (callee == 0x000A79B0u) {
        active_draw_room = room_argument <= 200u ? room_argument : 0u;
        active_draw_scope_base = stack_pointer;
        active_draw_scope = 1;
    } else if (active_draw_scope && stack_pointer >= active_draw_scope_base) {
        active_draw_room = 0u;
        active_draw_scope_base = 0u;
        active_draw_scope = 0;
    }
}

/* outcome: 0 accepted, 1 invalid room count, 2 rejected room order. */
void tsfp_native_scene_bridge_note_collector_result(uint32_t outcome,
                                                     int32_t room_count,
                                                     int32_t draw_depth) {
    if (!atomic_load_explicit(&scene_enabled, memory_order_acquire)) return;
    atomic_fetch_add_explicit(&collector_adapter_entries, 1u, memory_order_relaxed);
    atomic_store_explicit(&collector_last_room_count, room_count, memory_order_relaxed);
    atomic_store_explicit(&collector_last_draw_depth, draw_depth, memory_order_relaxed);
    switch (outcome) {
    case 0u: atomic_fetch_add_explicit(&collector_accepted, 1u, memory_order_relaxed); break;
    case 1u: atomic_fetch_add_explicit(&collector_bad_count, 1u, memory_order_relaxed); break;
    case 2u: atomic_fetch_add_explicit(&collector_bad_order, 1u, memory_order_relaxed); break;
    default: break;
    }
}

void tsfp_native_scene_bridge_collector_counters(uint64_t *guest_calls,
                                                  uint64_t *adapter_entries,
                                                  uint64_t *accepted,
                                                  uint64_t *bad_count,
                                                  uint64_t *bad_order,
                                                  int32_t *last_room_count,
                                                  int32_t *last_draw_depth) {
    if (guest_calls != NULL)
        *guest_calls = atomic_load_explicit(&collector_guest_calls, memory_order_relaxed);
    if (adapter_entries != NULL)
        *adapter_entries = atomic_load_explicit(&collector_adapter_entries, memory_order_relaxed);
    if (accepted != NULL)
        *accepted = atomic_load_explicit(&collector_accepted, memory_order_relaxed);
    if (bad_count != NULL)
        *bad_count = atomic_load_explicit(&collector_bad_count, memory_order_relaxed);
    if (bad_order != NULL)
        *bad_order = atomic_load_explicit(&collector_bad_order, memory_order_relaxed);
    if (last_room_count != NULL)
        *last_room_count = atomic_load_explicit(&collector_last_room_count, memory_order_relaxed);
    if (last_draw_depth != NULL)
        *last_draw_depth = atomic_load_explicit(&collector_last_draw_depth, memory_order_relaxed);
}

void tsfp_native_scene_bridge_record_draw(const struct tsfp_native_scene_draw_packet *packet,
                                          const uint8_t *payload, uint32_t payload_bytes) {
    if (!atomic_load_explicit(&scene_enabled, memory_order_acquire) || packet == NULL) {
        return;
    }
    lock_scene();
    if (draw_capture_active) {
        if (draw_capture_scope_packets >= TSFP_NATIVE_SCENE_DRAW_SCOPE_PACKET_LIMIT) {
            ++dropped_draw_packets;
            unlock_scene();
            return;
        }
        ++draw_capture_scope_packets;
        if (packet->room == 0u || packet->room > 200u) {
            ++unscoped_draws;
        }
        ++draw_sequence;
        if (draw_sequence == 0u) {
            ++draw_sequence;
        }
        const uint32_t slot = (uint32_t)(draw_sequence % TSFP_NATIVE_SCENE_DRAW_CAPACITY);
        struct draw_record *record = &draw_queue[slot];
        if (record->payload != NULL) {
            draw_payload_bytes -= record->payload_bytes;
            free(record->payload);
        }
        memset(record, 0, sizeof(*record));
        record->sequence = draw_sequence;
        record->packet = *packet;
        record->packet.frame_sequence = active_frame_sequence;
        const uint32_t scope_remaining = scope_payload_limit() -
                                         draw_capture_scope_payload_bytes;
        if (payload_bytes != 0u && payload != NULL && payload_bytes <= scope_remaining &&
            payload_bytes <= TSFP_NATIVE_SCENE_DRAW_PAYLOAD_LIMIT - draw_payload_bytes) {
            record->payload = malloc(payload_bytes);
            if (record->payload != NULL) {
                memcpy(record->payload, payload, payload_bytes);
                record->payload_bytes = payload_bytes;
                draw_payload_bytes += payload_bytes;
                draw_capture_scope_payload_bytes += payload_bytes;
            }
        }
        if (payload_bytes != 0u && record->payload == NULL) {
            record->packet.payload_dropped = 1u;
            ++dropped_draw_packets;
        }
        if (draw_sequence > draw_read_sequence + TSFP_NATIVE_SCENE_DRAW_CAPACITY) {
            ++dropped_draw_packets;
        }
    }
    unlock_scene();
}

void tsfp_native_scene_bridge_record_draw_capture_timing(uint64_t elapsed_ns,
                                                          uint32_t payload_bytes) {
    atomic_fetch_add_explicit(&draw_capture_timed_calls, 1u, memory_order_relaxed);
    atomic_fetch_add_explicit(&draw_capture_elapsed_ns, elapsed_ns, memory_order_relaxed);
    atomic_fetch_add_explicit(&draw_capture_payload_bytes, payload_bytes, memory_order_relaxed);
}

void tsfp_native_scene_bridge_draw_capture_timing(uint64_t *calls, uint64_t *elapsed_ns,
                                                   uint64_t *payload_bytes) {
    if (calls != NULL) *calls = atomic_load_explicit(&draw_capture_timed_calls, memory_order_relaxed);
    if (elapsed_ns != NULL) *elapsed_ns = atomic_load_explicit(&draw_capture_elapsed_ns, memory_order_relaxed);
    if (payload_bytes != NULL) *payload_bytes = atomic_load_explicit(&draw_capture_payload_bytes, memory_order_relaxed);
}

uint64_t tsfp_native_scene_bridge_read_draw(uint64_t after_sequence, uint64_t *sequence,
                                             struct tsfp_native_scene_draw_packet *packet,
                                             uint8_t *payload, uint32_t payload_capacity,
                                             uint32_t *payload_bytes) {
    if (!atomic_load_explicit(&scene_enabled, memory_order_acquire) || sequence == NULL || packet == NULL ||
        payload_bytes == NULL) {
        return 0;
    }
    lock_scene();
    const uint64_t current = draw_sequence;
    uint64_t next = after_sequence + 1u;
    const uint64_t oldest = current > TSFP_NATIVE_SCENE_DRAW_CAPACITY
                                ? current - TSFP_NATIVE_SCENE_DRAW_CAPACITY + 1u
                                : (current == 0u ? 0u : 1u);
    if (next < oldest) {
        next = oldest;
    }
    if (current == 0u || next > current) {
        unlock_scene();
        return 0;
    }
    struct draw_record *record = &draw_queue[next % TSFP_NATIVE_SCENE_DRAW_CAPACITY];
    if (record->sequence != next) {
        unlock_scene();
        return 0;
    }
    *packet = record->packet;
    *sequence = next;
    *payload_bytes = record->payload_bytes;
    if (record->payload_bytes == 0u) {
        if (next > draw_read_sequence) {
            draw_read_sequence = next;
        }
        unlock_scene();
        return next;
    }
    if (record->payload_bytes != 0u && (payload == NULL || payload_capacity < record->payload_bytes)) {
        unlock_scene();
        return next;
    }
    if (record->payload_bytes != 0u) {
        memcpy(payload, record->payload, record->payload_bytes);
        draw_payload_bytes -= record->payload_bytes;
        free(record->payload);
        record->payload = NULL;
        record->payload_bytes = 0u;
    }
    if (next > draw_read_sequence) {
        draw_read_sequence = next;
    }
    unlock_scene();
    return next;
}

void tsfp_native_scene_bridge_draw_counters(uint64_t *unscoped, uint64_t *dropped,
                                             uint64_t *scope_errors) {
    lock_scene();
    if (unscoped != NULL) *unscoped = unscoped_draws;
    if (dropped != NULL) *dropped = dropped_draw_packets;
    if (scope_errors != NULL) *scope_errors = draw_capture_scope_errors;
    unlock_scene();
}

void tsfp_native_scene_bridge_retire_object(uint32_t record_index) {
    if (!atomic_load_explicit(&scene_enabled, memory_order_acquire) ||
        record_index >= TSFP_NATIVE_SCENE_OBJECT_CAPACITY) {
        return;
    }
    lock_scene();
    ++scene_sequence;
    if (scene_sequence == 0) {
        ++scene_sequence;
    }
    struct scene_publication *publication =
        &scene_queue[scene_sequence % TSFP_NATIVE_SCENE_QUEUE_CAPACITY];
    publication->sequence = scene_sequence;
    publication->kind = TSFP_NATIVE_SCENE_KIND_RETIRE;
    publication->room_count = 0;
    publication->object_count = 1;
    publication->view_count = 0;
    publication->object_records[0] = record_index;
    unlock_scene();
}

void tsfp_native_scene_bridge_publish(const uint8_t *rooms, uint32_t room_count,
                                      int32_t draw_depth, const uint32_t *object_records,
                                      uint32_t object_count,
                                      const struct tsfp_native_scene_view *view) {
    if (!atomic_load_explicit(&scene_enabled, memory_order_acquire) ||
        room_count > TSFP_NATIVE_SCENE_ROOM_CAPACITY ||
        object_count > TSFP_NATIVE_SCENE_OBJECT_CAPACITY ||
        (room_count != 0 && rooms == NULL) ||
        (object_count != 0 && object_records == NULL)) {
        return;
    }
    lock_scene();
    ++scene_sequence;
    if (scene_sequence == 0) {
        ++scene_sequence;
    }
    struct scene_publication *publication =
        &scene_queue[scene_sequence % TSFP_NATIVE_SCENE_QUEUE_CAPACITY];
    publication->sequence = scene_sequence;
    latest_frame_sequence = scene_sequence;
    publication->kind = TSFP_NATIVE_SCENE_KIND_FRAME;
    publication->room_count = room_count;
    publication->object_count = object_count;
    publication->view_count = view == NULL ? 0 : 1;
    publication->draw_depth = draw_depth;
    if (room_count != 0) {
        memcpy(publication->rooms, rooms, room_count);
    }
    if (object_count != 0) {
        memcpy(publication->object_records, object_records,
               object_count * sizeof(publication->object_records[0]));
    }
    if (view != NULL) {
        publication->view = *view;
    }
    unlock_scene();
}

uint64_t tsfp_native_scene_bridge_read(uint64_t after_sequence, uint64_t *sequence,
                                       uint8_t *rooms, uint32_t room_capacity,
                                       uint32_t *room_count, int32_t *draw_depth,
                                       uint32_t *kind, uint32_t *object_records,
                                       uint32_t object_capacity, uint32_t *object_count,
                                       struct tsfp_native_scene_view *view, uint32_t *view_count) {
    if (!atomic_load_explicit(&scene_enabled, memory_order_acquire) || sequence == NULL ||
        room_count == NULL || draw_depth == NULL || kind == NULL || object_count == NULL ||
        view_count == NULL) {
        return 0;
    }
    lock_scene();
    const uint64_t current = scene_sequence;
    uint64_t next = after_sequence + 1;
    const uint64_t oldest = current > TSFP_NATIVE_SCENE_QUEUE_CAPACITY
                                ? current - TSFP_NATIVE_SCENE_QUEUE_CAPACITY + 1
                                : (current == 0 ? 0 : 1);
    if (next < oldest) {
        next = oldest;
    }
    if (current == 0 || next > current) {
        unlock_scene();
        return 0;
    }
    const struct scene_publication *publication =
        &scene_queue[next % TSFP_NATIVE_SCENE_QUEUE_CAPACITY];
    if (publication->sequence != next || publication->room_count > room_capacity ||
        publication->object_count > object_capacity ||
        (publication->room_count != 0 && rooms == NULL) ||
        (publication->object_count != 0 && object_records == NULL) ||
        (publication->view_count != 0 && view == NULL)) {
        unlock_scene();
        return 0;
    }
    if (publication->room_count != 0) {
        memcpy(rooms, publication->rooms, publication->room_count);
    }
    if (publication->object_count != 0) {
        memcpy(object_records, publication->object_records,
               publication->object_count * sizeof(publication->object_records[0]));
    }
    if (publication->view_count != 0) {
        *view = publication->view;
    }
    *sequence = next;
    *kind = publication->kind;
    *room_count = publication->room_count;
    *draw_depth = publication->draw_depth;
    *object_count = publication->object_count;
    *view_count = publication->view_count;
    unlock_scene();
    return next;
}
