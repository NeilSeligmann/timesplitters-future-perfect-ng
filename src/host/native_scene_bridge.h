/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_NATIVE_SCENE_BRIDGE_H
#define TSFP_NATIVE_SCENE_BRIDGE_H

#include <stdint.h>

#define TSFP_NATIVE_SCENE_ROOM_CAPACITY 200u
#define TSFP_NATIVE_SCENE_QUEUE_CAPACITY 128u
#define TSFP_NATIVE_SCENE_OBJECT_CAPACITY 0x226u
#define TSFP_NATIVE_SCENE_KIND_FRAME 0u
#define TSFP_NATIVE_SCENE_KIND_RETIRE 1u
#define TSFP_NATIVE_SCENE_DRAW_CAPACITY 8192u
#define TSFP_NATIVE_SCENE_DRAW_PAYLOAD_LIMIT (32u * 1024u * 1024u)
#define TSFP_NATIVE_SCENE_DRAW_SCOPE_PACKET_LIMIT 256u
#define TSFP_NATIVE_SCENE_DRAW_SCOPE_PAYLOAD_LIMIT (2u * 1024u * 1024u)
#define TSFP_NATIVE_SCENE_DRAW_STATE_WORDS 256u

/* Copied at the D3D8 draw hook while the retained whole-scene pass is active.
 * room comes from the live lifted per-room caller argument on the guest stack.
 * Guest addresses identify source resources; bounded draw payloads are copied
 * into host-owned storage, but are not yet native renderer resource leases. */
struct tsfp_native_scene_draw_packet {
    uint64_t frame_sequence;
    uint32_t room;
    uint32_t indexed;
    uint32_t primitive;
    uint32_t first;
    uint32_t count;
    uint32_t vertex_buffer;
    uint32_t vertex_buffer_bytes;
    uint32_t vertex_stride;
    uint32_t vertex_first;
    uint32_t index_buffer;
    uint32_t index_buffer_bytes;
    uint32_t texture_headers[4];
    uint32_t texture_data[4];
    uint32_t texture_formats[4];
    uint32_t texture_sizes[4];
    uint32_t texture_stage_state[128];
    uint32_t viewport_words[4];
    uint32_t viewport_target[4];
    uint32_t scissor_words[4];
    uint32_t scissor_count;
    uint32_t scissor_exclusive;
    uint32_t vertex_declaration;
    uint32_t vertex_shader_handle;
    uint32_t vertex_shader_index;
    uint32_t vertex_program_bytes;
    uint32_t vertex_constants_bytes;
    uint32_t pixel_shader_bytes;
    uint64_t vertex_program_hash;
    uint64_t vertex_constants_hash;
    uint64_t pixel_shader_hash;
    uint32_t blend_enable;
    uint32_t blend_source;
    uint32_t blend_destination;
    uint32_t blend_operation;
    uint32_t depth_enable;
    uint32_t depth_write;
    uint32_t depth_function;
    uint32_t cull_mode;
    uint32_t stencil_enable;
    uint32_t stencil_function;
    uint32_t stencil_reference;
    uint32_t stencil_mask;
    uint32_t stencil_write_mask;
    uint32_t stencil_fail;
    uint32_t stencil_depth_fail;
    uint32_t stencil_pass;
    uint32_t vertex_bytes;
    uint32_t index_bytes;
    uint32_t texture_bytes[4];
    uint32_t payload_bytes;
    uint32_t payload_dropped;
    uint32_t render_state[TSFP_NATIVE_SCENE_DRAW_STATE_WORDS];
    uint64_t material_key;
    uint64_t state_key;
};

struct tsfp_native_room_coverage {
    int32_t depth;
    int32_t rectangle[4];
    uint32_t after_ordinary;
};
struct tsfp_native_camera_sample {
    float eye[3];
    float view[16];
    float projection[16];
    float tangent_extents[2];
    int32_t visible_rooms[2];
};
struct tsfp_native_scene_view {
    uint32_t viewer;
    float position[3];
    float matrix[16];
    float viewport[4];
    int32_t rectangle[4];
    uint32_t camera_valid;
    struct tsfp_native_camera_sample camera;
    uint32_t room_count;
    int32_t max_depth;
    struct tsfp_native_room_coverage coverage[TSFP_NATIVE_SCENE_ROOM_CAPACITY + 1];
};

/* Optional fixed-size mailbox for the no_std room/object collector and Bevy. */
void tsfp_native_scene_bridge_enable(int enabled);
void tsfp_native_scene_bridge_set_draw_capture_sample_interval(uint32_t interval);
int tsfp_native_scene_bridge_capture_enabled(void);
int tsfp_native_scene_bridge_draw_capture_active(void);
int tsfp_native_scene_bridge_draw_capture_budget(uint32_t *payload_remaining);
void tsfp_native_scene_bridge_begin_draw_capture(void);
void tsfp_native_scene_bridge_end_draw_capture(void);
uint32_t tsfp_native_scene_bridge_current_draw_room(void);
void tsfp_native_scene_bridge_note_call(uint32_t callee, uint32_t stack_pointer,
                                        uint32_t room_argument);
void tsfp_native_scene_bridge_note_collector_result(uint32_t outcome,
                                                     int32_t room_count,
                                                     int32_t draw_depth);
void tsfp_native_scene_bridge_collector_counters(uint64_t *guest_calls,
                                                  uint64_t *adapter_entries,
                                                  uint64_t *accepted,
                                                  uint64_t *bad_count,
                                                  uint64_t *bad_order,
                                                  int32_t *last_room_count,
                                                  int32_t *last_draw_depth);
void tsfp_native_scene_bridge_record_draw(const struct tsfp_native_scene_draw_packet *packet,
                                          const uint8_t *payload, uint32_t payload_bytes);
void tsfp_native_scene_bridge_record_draw_capture_timing(uint64_t elapsed_ns,
                                                          uint32_t payload_bytes);
void tsfp_native_scene_bridge_draw_capture_timing(uint64_t *calls, uint64_t *elapsed_ns,
                                                   uint64_t *payload_bytes);
uint64_t tsfp_native_scene_bridge_read_draw(uint64_t after_sequence, uint64_t *sequence,
                                             struct tsfp_native_scene_draw_packet *packet,
                                             uint8_t *payload, uint32_t payload_capacity,
                                             uint32_t *payload_bytes);
void tsfp_native_scene_bridge_draw_counters(uint64_t *unscoped, uint64_t *dropped,
                                             uint64_t *scope_errors);
void tsfp_native_scene_bridge_retire_object(uint32_t record_index);
void tsfp_native_scene_bridge_publish(const uint8_t *rooms, uint32_t room_count,
                                      int32_t draw_depth, const uint32_t *object_records,
                                      uint32_t object_count,
                                      const struct tsfp_native_scene_view *view);
uint64_t tsfp_native_scene_bridge_read(uint64_t after_sequence, uint64_t *sequence,
                                       uint8_t *rooms, uint32_t room_capacity,
                                       uint32_t *room_count, int32_t *draw_depth,
                                       uint32_t *kind, uint32_t *object_records,
                                       uint32_t object_capacity, uint32_t *object_count,
                                       struct tsfp_native_scene_view *view, uint32_t *view_count);

#endif
