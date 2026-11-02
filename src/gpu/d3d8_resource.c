/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * See d3d8_resource.h.
 */
#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <pthread.h>
#include <string.h>
#include <time.h>
#include "d3d8_resource.h"
#include "../host/native_scene_bridge.h"
#include "../host/recomp_abi.h"

#include "d3d8_guest.h"
#include "d3d8_combiner.h"
#include "d3d8_dirty.h"
#include "d3d8_texture_dirty.h"
#include "d3d8_state.h"
#include "live_texture.h"
#include <float.h>
#include "d3d8_gpu.h"
#include "d3d8_hle.h"
#include "d3d8_pushbuffer.h"
#include "guest_mem.h"

#if defined(__GNUC__) || defined(__clang__)
#pragma weak g_esp
#endif
#include "kernel_call.h"
#include "guest_mem.h"

#define KERNEL_ORDINAL_MM_ALLOCATE_CONTIGUOUS_EX 166u

/* The allocation wrapper's flag word for the data block is 0xB2800000: alignment 1 << 2 and
 * protect 0x404 (MEASURED from the wrapper's decode, 0x00380C57 onwards). */
#define BUFFER_ALIGNMENT 4u
#define BUFFER_PROTECT 0x404u

#define REGISTRY_CAPACITY 128u

typedef struct {
    uint32_t header;
    uint32_t physical;
    uint32_t virtual_address;
    uint32_t bytes;
    uint64_t allocation_generation;
} buffer_record;

static buffer_record registry[REGISTRY_CAPACITY];
static size_t registry_count;
static uint32_t header_heap;
static uint32_t observed_scissor[4];
static uint32_t observed_scissor_count;
static uint32_t observed_scissor_exclusive;
/* Original Register masks a virtual allocation address into a 28-bit Data word.
 * This sidecar records that exact relationship; it is not the synthetic physical
 * address used by MmGetPhysicalAddress, and never searches possible VA aliases. */
typedef struct {
    uint32_t header, common_identity, data, format, size;
    uint32_t region_address, region_physical, region_requested, region_size;
    uint32_t virtual_data;
    uint64_t identity, allocation_generation;
    bool valid;
} resource_alias;
/* T755: the table grows (it was a fixed 128 whose overflow DROPPED the record silently, and the
 * title registers 490 front end textures before the first movie surface, so the attract movie's
 * own Register was lost and its LockRect stopped as "no verified registered allocation alias").
 * A header is never dropped for want of room: the table doubles to ALIAS_LIMIT, and at the limit a
 * record that can no longer verify (stale stack header, freed allocation) is reused, else the
 * Register stops by name. */
#define ALIAS_INITIAL_CAPACITY 128u
#define ALIAS_LIMIT 16384u
static resource_alias *aliases;
static size_t alias_count;
static size_t alias_capacity;
static uint64_t alias_identity;
static pthread_mutex_t alias_lock = PTHREAD_MUTEX_INITIALIZER;

typedef enum {
    ALIAS_REJECTION_NONE,
    ALIAS_REJECTION_NO_CONTIGUOUS_REGION,
    ALIAS_REJECTION_REGION_TOO_LARGE,
    ALIAS_REJECTION_DATA_OUTSIDE_REGION,
    ALIAS_REJECTION_VIRTUAL_RETAINED,
    ALIAS_REJECTION_COPY_UNVERIFIED
} alias_rejection_reason;

typedef struct {
    uint32_t header;
    uint32_t base;
    uint32_t offset;
    alias_rejection_reason reason;
} alias_rejection;

/* Diagnostics are bounded like the alias table. If every row has been used, a row is
 * replaced cyclically; the next Register clears the header's previous rejection. */
static alias_rejection rejected_aliases[ALIAS_LIMIT];
static size_t rejection_count;
static size_t rejection_next;
#define IMMUTABLE_COMMON_MASK 0xFF870000u /* all bits except low16 refs and 0x780000 busy bits */


void d3d8_resource_reset(void)
{
    registry_count = 0u;
    pthread_mutex_lock(&alias_lock);
    alias_count = 0u;
    header_heap = 0u;
    memset(observed_scissor, 0, sizeof(observed_scissor));
    observed_scissor_count = 0u;
    observed_scissor_exclusive = 0u;
    memset(rejected_aliases, 0, sizeof(rejected_aliases));
    rejection_count = 0u;
    rejection_next = 0u;
    pthread_mutex_unlock(&alias_lock);
}

size_t d3d8_resource_buffer_count(void)
{
    return registry_count;
}

uint32_t d3d8_resource_virtual_of_physical(uint32_t physical)
{
    for (size_t index = 0u; index < registry_count; index++) {
        if (registry[index].physical == physical &&
            kmem_allocation_generation(registry[index].virtual_address) == registry[index].allocation_generation) {
            return registry[index].virtual_address;
        }
    }
    return 0u;
}

static uint32_t resource_buffer_bytes(uint32_t header)
{
    for (size_t index = 0u; index < registry_count; index++) {
        if (registry[index].header == header &&
            kmem_allocation_generation(registry[index].virtual_address) == registry[index].allocation_generation) {
            return registry[index].bytes;
        }
    }
    return 0u;
}

static uint64_t packet_hash_word(uint64_t hash, uint32_t word)
{
    for (unsigned byte = 0u; byte < 4u; byte++) {
        hash ^= (uint8_t)(word >> (byte * 8u));
        hash *= UINT64_C(1099511628211);
    }
    return hash;
}

static uint64_t packet_hash_bytes(uint64_t hash, const uint8_t *bytes, uint32_t length)
{
    for (uint32_t index = 0u; index < length; index++) {
        hash ^= bytes[index];
        hash *= UINT64_C(1099511628211);
    }
    return hash;
}

static bool append_guest_span(uint8_t *payload, uint32_t *used, uint32_t capacity,
                              uint32_t address, uint32_t bytes);

void d3d8_resource_note_scissor(uint32_t count, uint32_t exclusive, const uint32_t rectangle[4])
{
    if (rectangle == NULL || !tsfp_native_scene_bridge_capture_enabled()) return;
    memcpy(observed_scissor, rectangle, sizeof(observed_scissor));
    observed_scissor_count = count;
    observed_scissor_exclusive = exclusive;
}

static bool capture_shader_inputs(uint8_t *payload, uint32_t *used, uint32_t capacity,
                                  struct tsfp_native_scene_draw_packet *packet)
{
    const uint32_t program_address = D3D8_DEVICE_BASE + 0x10A8u;
    const uint32_t program_bytes = 136u * 16u;
    if (packet->vertex_shader_handle != 0u) {
        const uint32_t program_at = *used;
        if (!append_guest_span(payload, used, capacity, program_address, program_bytes)) return false;
        packet->vertex_program_bytes = program_bytes;
        packet->vertex_program_hash = packet_hash_bytes(UINT64_C(14695981039346656037),
                                                        payload + program_at, program_bytes);
        const uint32_t constants_bytes = D3D8_CONSTANT_REGISTERS * 16u;
        const uint32_t constants_at = *used;
        if (!append_guest_span(payload, used, capacity, D3D8_CONSTANT_SHADOW, constants_bytes)) return false;
        packet->vertex_constants_bytes = constants_bytes;
        packet->vertex_constants_hash = packet_hash_bytes(UINT64_C(14695981039346656037),
                                                          payload + constants_at, constants_bytes);
    }

    const uint32_t wrapper = d3d8_device_load32(0x784u);
    if (wrapper == 0u || wrapper > UINT32_MAX - 11u) return true;
    uint32_t definition = 0u;
    if (!kernel_guest_read_bytes(wrapper + 8u, &definition, sizeof(definition)) ||
        definition == 0u || definition > UINT32_MAX - 239u) return true;
    const uint32_t definition_at = *used;
    if (!append_guest_span(payload, used, capacity, definition, 240u)) return false;
    packet->pixel_shader_bytes = 240u;
    packet->pixel_shader_hash = packet_hash_bytes(UINT64_C(14695981039346656037),
                                                  payload + definition_at, 240u);
    return true;
}

#define SCENE_DRAW_PACKET_BYTES (4u * 1024u * 1024u)
#define SCENE_VERTEX_DECLARATION_BYTES (16u * 16u)
static _Thread_local uint8_t *scene_draw_scratch;
static _Thread_local uint32_t scene_draw_scratch_bytes;

static uint64_t scene_capture_clock_ns(void)
{
    struct timespec now;
    if (clock_gettime(CLOCK_MONOTONIC, &now) != 0) return 0u;
    return (uint64_t)now.tv_sec * UINT64_C(1000000000) + (uint64_t)now.tv_nsec;
}

static bool append_guest_span(uint8_t *payload, uint32_t *used, uint32_t capacity,
                              uint32_t address, uint32_t bytes)
{
    if (bytes == 0u) return true;
    if (bytes > capacity - *used || kernel_guest_at_ro(address, bytes) == NULL ||
        !kernel_guest_read_bytes(address, payload + *used, bytes)) return false;
    *used += bytes;
    return true;
}

static bool capture_vertex_span(uint8_t *payload, uint32_t *used, uint32_t capacity,
                                struct tsfp_native_scene_draw_packet *packet,
                                uint32_t indexed, uint32_t first, uint32_t count,
                                uint32_t index_data)
{
    if (packet->vertex_buffer == 0u || packet->vertex_stride == 0u || count == 0u) return true;
    uint32_t low = first, high = first;
    if (indexed != 0u) {
        if (count > UINT32_MAX / 2u || kernel_guest_at_ro(index_data, count * 2u) == NULL) return false;
        low = UINT16_MAX;
        high = 0u;
        for (uint32_t index = 0u; index < count; index++) {
            uint8_t encoded[2];
            if (!kernel_guest_read_bytes(index_data + index * 2u, encoded, sizeof(encoded))) return false;
            const uint32_t value = (uint32_t)encoded[0] | ((uint32_t)encoded[1] << 8u);
            if (value < low) low = value;
            if (value > high) high = value;
        }
    } else {
        const uint64_t last = (uint64_t)first + count - 1u;
        if (last > UINT32_MAX) return false;
        high = (uint32_t)last;
    }
    const uint32_t vertex_count = high - low + 1u;
    packet->vertex_first = low;
    const uint64_t byte_count = (uint64_t)vertex_count * packet->vertex_stride;
    const uint32_t physical = d3d8_guest_load32(packet->vertex_buffer + 4u);
    const uint32_t data = d3d8_resource_virtual_of_physical(physical);
    const uint32_t base_vertex = indexed != 0u ? d3d8_device_load32(0x1Cu) : 0u;
    const uint64_t offset = ((uint64_t)base_vertex + low) * packet->vertex_stride;
    const uint32_t allocation_bytes = packet->vertex_buffer_bytes;
    if (data == 0u || byte_count > UINT32_MAX || offset + byte_count > allocation_bytes ||
        (uint64_t)data + offset > UINT32_MAX) return false;
    packet->vertex_bytes = (uint32_t)byte_count;
    return append_guest_span(payload, used, capacity, (uint32_t)((uint64_t)data + offset), packet->vertex_bytes);
}

/* Capture the active NV2A register 1 array separately from the D3D8 stream
 * table above. The renderer's typed binding path keeps this array at the
 * 0x55F9BC/0x55F9C0 pair; it can differ from the guest D3D stream buffer and
 * declaration. The D3D stream's vertex range supplies the shared draw indices. */
static bool capture_register1_span(uint8_t *payload, uint32_t *used, uint32_t capacity,
                                   const struct tsfp_native_scene_draw_packet *packet)
{
    if (packet->payload_dropped || packet->vertex_stride == 0u || packet->vertex_bytes == 0u ||
        packet->vertex_bytes % packet->vertex_stride != 0u) return false;

    const uint32_t stride = d3d8_guest_load32(0x0055F9C0u) & 0xFFu;
    const uint32_t physical = d3d8_guest_load32(0x0055F9BCu) & 0x7FFFFFFFu;
    const uint32_t vertex_count = packet->vertex_bytes / packet->vertex_stride;
    if (stride < 12u || physical == 0u || vertex_count == 0u) return false;
    const uint64_t offset = (uint64_t)packet->vertex_first * stride;
    const uint64_t byte_count = (uint64_t)vertex_count * stride;
    if (offset + byte_count > UINT32_MAX || byte_count > UINT32_MAX ||
        *used > capacity || byte_count + 24u > capacity - *used) return false;

    uint32_t base = d3d8_resource_virtual_of_physical(physical);
    if (base == 0u && kernel_guest_at_ro(physical, 1u) != NULL) base = physical;
    if (base == 0u || (uint64_t)base + offset + byte_count > UINT32_MAX) return false;
    const uint32_t address = base + (uint32_t)offset;
    if (kernel_guest_at_ro(address, (uint32_t)byte_count) == NULL) return false;

    uint8_t trailer[24] = {'R', '1', 'V', 'S'};
    const uint32_t words[5] = {1u, stride, packet->vertex_first, 0x32u, (uint32_t)byte_count};
    for (uint32_t index = 0u; index < 5u; index++) {
        const uint32_t value = words[index];
        trailer[4u + index * 4u + 0u] = (uint8_t)value;
        trailer[4u + index * 4u + 1u] = (uint8_t)(value >> 8u);
        trailer[4u + index * 4u + 2u] = (uint8_t)(value >> 16u);
        trailer[4u + index * 4u + 3u] = (uint8_t)(value >> 24u);
    }
    const uint32_t original_used = *used;
    memcpy(payload + *used, trailer, sizeof(trailer));
    *used += sizeof(trailer);
    if (!append_guest_span(payload, used, capacity, address, (uint32_t)byte_count)) {
        *used = original_used;
        return false;
    }
    return true;
}

static bool capture_texture_span(uint8_t *payload, uint32_t *used, uint32_t capacity,
                                 struct tsfp_native_scene_draw_packet *packet, uint32_t stage)
{
    const uint32_t header = packet->texture_headers[stage];
    if (header == 0u) return true;
    live_texture_binding binding = {
        .header = header,
        .format = packet->texture_formats[stage],
        .size_word = d3d8_guest_load32(header + 0x10u),
        .data = packet->texture_data[stage],
        .mip_limit = 0u,
    };
    live_texture_plan plan;
    if (!live_texture_plan_binding(&binding, true, &plan) || plan.source_bytes > capacity - *used) return false;
    uint32_t address = 0u;
    uint64_t identity = 0u;
    const char *refusal = NULL;
    if (d3d8_resource_try_alias(header, binding.data, plan.source_bytes, &address, &identity, &refusal) !=
        D3D8_RESOURCE_ALIAS_RESOLVED) return false;
    (void)identity;
    (void)refusal;
    packet->texture_bytes[stage] = plan.source_bytes;
    return append_guest_span(payload, used, capacity, address, plan.source_bytes);
}

/* 0x86480 calls 0xA79B0(room) at 0x86579. While that room's object draws
 * descend through 0xA2430 and the D3D hooks, the live guest stack still holds
 * the caller return PC followed by 0xA79B0's room argument. Read that original
 * argument; do not infer a room from the collector's flattened scratch lists. */
static uint32_t active_room_from_guest_stack(void)
{
    const uint32_t scoped_room = tsfp_native_scene_bridge_current_draw_room();
    if (scoped_room > 0u && scoped_room <= UINT8_MAX) return scoped_room;
    if (&g_esp == NULL || g_esp == 0u) return UINT32_MAX;
    const uint32_t first = g_esp;
    const uint32_t limit = first <= UINT32_MAX - 0x10000u ? first + 0x10000u : UINT32_MAX;
    for (uint32_t address = first; address <= limit - 8u; address += 4u) {
        uint32_t return_pc = 0u;
        if (!kernel_guest_read_u32(address, &return_pc)) break;
        if (return_pc != 0x0008657Eu) continue;
        uint32_t room = 0u;
        if (kernel_guest_read_u32(address + 4u, &room) && room <= UINT8_MAX) return room;
    }
    return UINT32_MAX;
}

/* Observe D3D8 bindings at the actual draw entry. Room identity comes from the
 * live 0xA79B0 stack argument made by the lifted per-room traversal. This is
 * observational: it neither narrows room scratch nor replays/reorders draws. */
void d3d8_resource_capture_scene_draw(uint32_t indexed, uint32_t primitive,
                                      uint32_t first, uint32_t count,
                                      uint32_t index_data)
{
    uint32_t payload_remaining = 0u;
    if (!tsfp_native_scene_bridge_draw_capture_budget(&payload_remaining)) {
        return;
    }
    const uint64_t capture_started = scene_capture_clock_ns();
    struct tsfp_native_scene_draw_packet packet = {0};
    packet.room = active_room_from_guest_stack();
    packet.indexed = indexed;
    packet.primitive = primitive;
    packet.first = first;
    packet.count = count;
    const uint32_t stream_table = 0x003E2BA8u;
    packet.vertex_stride = d3d8_guest_load32(stream_table);
    packet.vertex_buffer = d3d8_guest_load32(stream_table + 8u);
    packet.vertex_buffer_bytes = resource_buffer_bytes(packet.vertex_buffer);
    packet.vertex_declaration = d3d8_device_load32(0x794u);
    packet.vertex_shader_handle = d3d8_device_load32(0x798u);
    packet.vertex_shader_index = d3d8_device_load32(0x79Cu);
    for (uint32_t word = 0u; word < 4u; word++) {
        packet.viewport_words[word] = d3d8_device_load32(0xEE0u + word * 4u);
        packet.viewport_target[word] = d3d8_device_load32(0x954u + word * 4u);
    }
    memcpy(packet.scissor_words, observed_scissor, sizeof(packet.scissor_words));
    packet.scissor_count = observed_scissor_count;
    packet.scissor_exclusive = observed_scissor_exclusive;
    if (indexed != 0u) {
        packet.index_buffer = index_data;
        packet.index_buffer_bytes = count <= UINT32_MAX / 2u ? count * 2u : 0u;
    }
    uint64_t material = UINT64_C(14695981039346656037);
    for (uint32_t stage = 0u; stage < 4u; stage++) {
        const uint32_t texture = d3d8_device_load32(0x0F88u + stage * 4u);
        packet.texture_headers[stage] = texture;
        if (texture != 0u) {
            packet.texture_data[stage] = d3d8_guest_load32(texture + 4u);
            packet.texture_formats[stage] = d3d8_guest_load32(texture + 0x0Cu);
            packet.texture_sizes[stage] = d3d8_guest_load32(texture + 0x10u);
        }
        material = packet_hash_word(material, texture);
        material = packet_hash_word(material, packet.texture_data[stage]);
        material = packet_hash_word(material, packet.texture_formats[stage]);
        material = packet_hash_word(material, packet.texture_sizes[stage]);
    }
    packet.material_key = material;
    uint64_t state = UINT64_C(14695981039346656037);
    for (uint32_t word = 0u; word < TSFP_NATIVE_SCENE_DRAW_STATE_WORDS; word++) {
        packet.render_state[word] = d3d8_guest_load32(D3D8_GLOBAL_RS_SHADOW + word * 4u);
        state = packet_hash_word(state, packet.render_state[word]);
    }
    packet.blend_enable = packet.render_state[27u];
    packet.blend_source = packet.render_state[19u];
    packet.blend_destination = packet.render_state[20u];
    packet.blend_operation = packet.render_state[171u];
    packet.depth_enable = packet.render_state[7u];
    packet.depth_write = packet.render_state[14u];
    packet.depth_function = packet.render_state[23u];
    packet.cull_mode = packet.render_state[22u];
    packet.stencil_enable = packet.render_state[52u];
    packet.stencil_fail = packet.render_state[53u];
    packet.stencil_depth_fail = packet.render_state[54u];
    packet.stencil_pass = packet.render_state[55u];
    packet.stencil_function = packet.render_state[56u];
    packet.stencil_reference = packet.render_state[57u];
    packet.stencil_mask = packet.render_state[58u];
    packet.stencil_write_mask = packet.render_state[59u];
    for (uint32_t word = 0u; word < 128u; word++) {
        packet.texture_stage_state[word] = d3d8_guest_load32(0x003E3AC0u + word * 4u);
        state = packet_hash_word(state, packet.texture_stage_state[word]);
    }
    state = packet_hash_word(state, packet.vertex_declaration);
    state = packet_hash_word(state, packet.vertex_shader_handle);
    state = packet_hash_word(state, packet.vertex_shader_index);
    state = packet_hash_word(state, d3d8_guest_load32(D3D8_GLOBAL_DIRTY_MASK));
    uint32_t payload_capacity = payload_remaining < SCENE_DRAW_PACKET_BYTES
                                    ? payload_remaining
                                    : SCENE_DRAW_PACKET_BYTES;
    uint8_t *payload = NULL;
    if (payload_capacity != 0u) {
        if (scene_draw_scratch_bytes < payload_capacity) {
            uint8_t *grown = realloc(scene_draw_scratch, payload_capacity);
            if (grown != NULL) {
                scene_draw_scratch = grown;
                scene_draw_scratch_bytes = payload_capacity;
            }
        }
        if (scene_draw_scratch_bytes >= payload_capacity) payload = scene_draw_scratch;
    }
    if (payload == NULL) {
        packet.payload_dropped = 1u;
        tsfp_native_scene_bridge_record_draw(&packet, NULL, 0u);
        const uint64_t capture_finished = scene_capture_clock_ns();
        tsfp_native_scene_bridge_record_draw_capture_timing(
            capture_finished >= capture_started ? capture_finished - capture_started : 0u, 0u);
        return;
    }
    uint32_t payload_bytes = 0u;
    if (!capture_vertex_span(payload, &payload_bytes, payload_capacity, &packet,
                             indexed, first, count, index_data)) packet.payload_dropped = 1u;
    if (indexed != 0u && count <= UINT32_MAX / 2u) {
        packet.index_bytes = count * 2u;
        if (!append_guest_span(payload, &payload_bytes, payload_capacity, index_data,
                               packet.index_bytes)) packet.payload_dropped = 1u;
    }
    for (uint32_t stage = 0u; stage < 4u; stage++) {
        if (!capture_texture_span(payload, &payload_bytes, payload_capacity, &packet, stage))
            packet.payload_dropped = 1u;
    }
    if (!capture_shader_inputs(payload, &payload_bytes, payload_capacity, &packet))
        packet.payload_dropped = 1u;
    /* Preserve the guest declaration block after the other draw spans. The
     * sidecar reader identifies it by its fixed 16-entry, 16-byte layout. */
    if (packet.vertex_declaration != 0u &&
        kernel_guest_at_ro(packet.vertex_declaration, SCENE_VERTEX_DECLARATION_BYTES) != NULL) {
        uint32_t declaration_end = payload_bytes;
        if (append_guest_span(payload, &declaration_end, payload_capacity,
                              packet.vertex_declaration, SCENE_VERTEX_DECLARATION_BYTES)) {
            payload_bytes = declaration_end;
        }
    }
    (void)capture_register1_span(payload, &payload_bytes, payload_capacity, &packet);
    material = packet_hash_word(material, (uint32_t)packet.vertex_program_hash);
    material = packet_hash_word(material, (uint32_t)(packet.vertex_program_hash >> 32u));
    material = packet_hash_word(material, (uint32_t)packet.pixel_shader_hash);
    material = packet_hash_word(material, (uint32_t)(packet.pixel_shader_hash >> 32u));
    packet.material_key = material;
    state = packet_hash_word(state, (uint32_t)packet.vertex_program_hash);
    state = packet_hash_word(state, (uint32_t)(packet.vertex_program_hash >> 32u));
    state = packet_hash_word(state, (uint32_t)packet.vertex_constants_hash);
    state = packet_hash_word(state, (uint32_t)(packet.vertex_constants_hash >> 32u));
    state = packet_hash_word(state, (uint32_t)packet.pixel_shader_hash);
    state = packet_hash_word(state, (uint32_t)(packet.pixel_shader_hash >> 32u));
    packet.state_key = state;
    packet.payload_bytes = payload_bytes;
    tsfp_native_scene_bridge_record_draw(&packet, payload, payload_bytes);
    const uint64_t capture_finished = scene_capture_clock_ns();
    tsfp_native_scene_bridge_record_draw_capture_timing(
        capture_finished >= capture_started ? capture_finished - capture_started : 0u,
        payload_bytes);
}

uint32_t d3d8_create_buffer(uint32_t length)
{
    if (header_heap == 0u || !kmem_heap_valid(header_heap)) {
        header_heap = kmem_heap_create(0u, GUEST_HEAP_CHUNK_MIN, 0u);
        if (header_heap == 0u) {
            return 0u;
        }
    }
    const uint32_t header = kmem_heap_alloc(header_heap, D3D8_BUFFER_HEADER_BYTES);
    if (header == 0u) {
        return 0u;
    }

    const uint32_t args[5] = {length, 0u, 0xFFFFFFFFu, BUFFER_ALIGNMENT, BUFFER_PROTECT};
    const uint32_t data = d3d8_kernel_call(KERNEL_ORDINAL_MM_ALLOCATE_CONTIGUOUS_EX, args, 5u);
    if (data == 0u) {
        (void)kmem_heap_free(header_heap, header);
        return 0u;
    }

    if (registry_count >= REGISTRY_CAPACITY) {
        d3d8_hle_fatal(0x003D4EE0u,
                       "more than %u live buffers: the (physical, virtual) registry is full",
                       (unsigned)REGISTRY_CAPACITY);
    }
    const uint32_t physical = kmem_physical_address(data) & 0x0FFFFFFFu;
    registry[registry_count].physical = physical;
    registry[registry_count].header = header;
    registry[registry_count].virtual_address = data;
    registry[registry_count].bytes = length;
    registry[registry_count].allocation_generation = kmem_allocation_generation(data);
    registry_count++;

    d3d8_guest_store32(header, D3D8_BUFFER_COMMON);
    d3d8_guest_store32(header + 4u, physical);
    d3d8_guest_store32(header + 8u, 0u);
    return header;
}

uint32_t d3d8_vertex_buffer_lock2(uint32_t header, uint32_t flags)
{
    const uint32_t data = d3d8_guest_load32(header + 4u);
    const uint32_t virtual_address = d3d8_resource_virtual_of_physical(data);
    if (virtual_address == 0u) {
        d3d8_hle_fatal(0x003D4F30u,
                       "buffer %#x has unregistered physical Data %#x; Lock2 cannot map it",
                       (unsigned)header, (unsigned)data);
    }
    /* 0x003D4F35..5D: the cache-invalidation method is emitted unless bit 0x10 is set. */
    if ((flags & 0x10u) == 0u) {
        d3d8_pushbuffer_emit_pair(0x00041710u, 0u);
    }
    /* 0x003D6C20: resource Common bits select the device fence; otherwise Lock is the fence.
     * Route both through the shared instantaneous consumer rather than discard synchronization. */
    if ((flags & 0xA0u) == 0u && d3d8_guest_load32(D3D8_DEVICE_POINTER_SLOT) != 0u) {
        const uint32_t common = d3d8_guest_load32(header);
        const uint32_t lock = d3d8_guest_load32(header + 8u);
        if ((common & 0x00780000u) != 0u) {
            d3d8_gpu_fence_wait(d3d8_device_load32(D3D8_DEV_FENCE), 2u);
        } else if (lock != 0u) {
            d3d8_gpu_fence_wait(lock, 0x10u);
        }
    }
    return virtual_address;
}

uint32_t d3d8_set_stream_source(uint32_t stream, uint32_t header, uint32_t stride)
{
    const uint32_t table = 0x003E2BA8u;
    if (stream > (UINT32_MAX - table - 8u) / 12u) {
        d3d8_hle_fatal(0x003D58F0u, "stream index %#x overflows the guest table", (unsigned)stream);
    }
    const uint32_t row = table + stream * 12u;
    if (header != 0u) {
        d3d8_guest_store32(header, d3d8_guest_load32(header) + 0x00080000u);
    }
    const uint32_t old = d3d8_guest_load32(row + 8u);
    if (old != 0u) {
        d3d8_guest_store32(old + 8u, d3d8_device_load32(D3D8_DEV_FENCE));
        const uint32_t common = d3d8_guest_load32(old) - 0x00080000u;
        d3d8_guest_store32(old, common);
        if ((common & 0x0078FFFFu) == 0u) {
            d3d8_hle_fatal(0x003D58F0u,
                           "stream release of buffer %#x needs unimplemented resource destruction",
                           (unsigned)old);
        }
    }
    const uint32_t old_stride = d3d8_guest_load32(row);
    d3d8_guest_store32(row, stride);
    d3d8_guest_store32(row + 8u, header);
    const uint32_t dirty = d3d8_guest_load32(D3D8_GLOBAL_DIRTY_MASK);
    d3d8_guest_store32(D3D8_GLOBAL_DIRTY_MASK, dirty | (stride == old_stride ? 0x40u : 0x70u));
    return stride; /* eax at both measured returns; the public function is void. */
}

/* Plan the recovered 0x003DED80 family before any deferred-state writes. Each emitter has its own
 * reservation preamble, so a roll-over can fall between two of them (and between texture
 * stages): the plan walks a simulated writer through the same sites in the original order. */
static void plan_draw_dirty(uint32_t dirty, d3d8_pushbuffer_sim *sim)
{
    if ((dirty & ~0xC0FF7F7Fu) != 0u)
        d3d8_hle_fatal(0x003D4FB0u, "unsupported draw dirty mask %#x", (unsigned)dirty);
    if ((dirty & 0x100u) != 0u) d3d8_plan_point_state(sim);
    /* 0x003E11D0: with a bound pixel shader it returns the mask without writes, otherwise it is the
     * fixed-function selection (T443) and may add 0x400F. */
    if ((dirty & 0x800u) != 0u && d3d8_device_load32(0x784u) == 0u)
        dirty = d3d8_plan_fixed_function_combiner(dirty, sim);
    if ((dirty & 0x4000u) != 0u) d3d8_plan_shader_stage_program(sim);
    if ((dirty & 0xFu) != 0u) d3d8_plan_texture_stages(dirty, sim);
    if ((dirty & 0x2000u) != 0u) d3d8_plan_fog(sim);
    if ((dirty & 0x400u) != 0u) d3d8_plan_texture_transforms(sim);
    if ((dirty & 0xFF1000u) != 0u) d3d8_plan_default_lighting(dirty, sim);
    if ((dirty & 0x200u) != 0u) d3d8_plan_fixed_function_matrices(dirty, sim);
}

static void emit_draw_dirty(uint32_t dirty)
{
    if ((dirty & 0x100u) != 0u) (void)d3d8_emit_point_state();
    if ((dirty & 0x800u) != 0u && d3d8_device_load32(0x784u) == 0u)
        dirty = d3d8_emit_fixed_function_combiner(dirty);
    if ((dirty & 0x4000u) != 0u) (void)d3d8_emit_shader_stage_program();
    if ((dirty & 0xFu) != 0u) (void)d3d8_emit_texture_stages(dirty);
    if ((dirty & 0x2000u) != 0u) (void)d3d8_emit_disabled_fog();
    /* 0x003DE080 and 0x003DEB80 return early for a vertex shader or the programmable mode. */
    if ((dirty & 0x400u) != 0u) d3d8_emit_texture_transforms();
    if ((dirty & 0xFF1000u) != 0u) (void)d3d8_emit_default_lighting_state(dirty);
    if ((dirty & 0x200u) != 0u) d3d8_emit_fixed_function_matrices(dirty);
    d3d8_guest_store32(D3D8_GLOBAL_DIRTY_MASK, dirty & 0xC0000070u);
}

/* 0x003DED80: the controller Begin and the swap composition call directly. It reads the mask as it
 * stands (DrawVertices clears 0x50 first, through 0x003DEE00, and calls the cascade with the rest). */
void d3d8_run_dirty_cascade(void)
{
    const uint32_t dirty = d3d8_guest_load32(D3D8_GLOBAL_DIRTY_MASK);
    if ((dirty & 0x3FFFFF8Fu) != 0u) {
        d3d8_pushbuffer_sim sim = d3d8_pushbuffer_sim_start();
        plan_draw_dirty(dirty, &sim);
    }
    emit_draw_dirty(dirty);
}

/* The stream work of 0x003DEE00 after its cascade: whether it runs, and the bytes it writes. The
 * descriptor and header spans are validated here so a truncated declaration refuses before writes. */
static bool stream_work_bytes(uint32_t dirty, uint32_t base_vertex, uint32_t entry, uint32_t *bytes_out)
{
    *bytes_out = 0u;
    if ((dirty & 0x40000000u) != 0u ||
        ((dirty & 0x40u) == 0u && d3d8_device_load32(0x20u) == base_vertex)) return false;
    const uint32_t declaration = d3d8_device_load32(0x794u);
    const uint32_t mapping = 0x005496A0u + (d3d8_guest_load32(declaration + 4u) & 0x10u);
    uint32_t bytes = (dirty & 0x10u) != 0u ? 68u : 0u;
    for (uint32_t attribute = 0u; attribute < 16u; attribute++) {
        const uint32_t slot = d3d8_guest_load8(mapping + attribute);
        const uint64_t descriptor64 = (uint64_t)declaration + slot * 16u;
        if (descriptor64 + 32u > UINT64_C(0x100000000) || kernel_guest_at_ro((uint32_t)descriptor64, 32u) == NULL)
            d3d8_hle_fatal(entry, "stream declaration is truncated");
        const uint32_t descriptor = (uint32_t)descriptor64;
        const uint32_t stream = d3d8_guest_load32(descriptor + 0x14u);
        if (stream >= 16u) d3d8_hle_fatal(entry, "stream index exceeds guest table");
        const uint32_t row = 0x003E2BA8u + stream * 12u;
        if (d3d8_guest_load32(descriptor + 0x1Cu) == 2u) continue;
        const uint32_t buffer = d3d8_guest_load32(row + 8u);
        if (buffer == 0u) continue;
        if ((uint64_t)buffer + 8u > UINT64_C(0x100000000) || kernel_guest_at_ro(buffer, 8u) == NULL)
            d3d8_hle_fatal(entry, "stream buffer header is truncated");
        bytes += 8u;
    }
    *bytes_out = bytes;
    return true;
}

/* 0x003DEE00, called by DrawVertices with its argument zero. */
static void flush_draw_streams(uint32_t base_vertex)
{
    /* The original keeps this saved mask for the stream work even after the
     * cascade clears the global. 0x3DEE1C clears 0x50 BEFORE the cascade reads it. */
    const uint32_t dirty = d3d8_guest_load32(D3D8_GLOBAL_DIRTY_MASK);
    const bool cascade = (dirty & 0x3FFFFF8Fu) != 0u;
    if (cascade) {
        d3d8_pushbuffer_sim sim = d3d8_pushbuffer_sim_start();
        plan_draw_dirty(dirty, &sim);
    }
    d3d8_guest_store32(D3D8_GLOBAL_DIRTY_MASK, dirty & 0xFFFFFFAFu);
    if (cascade) emit_draw_dirty(dirty & 0xFFFFFFAFu);
    if ((dirty & 0x40000000u) != 0u ||
        ((dirty & 0x40u) == 0u && d3d8_device_load32(0x20u) == base_vertex)) return;
    const uint32_t declaration = d3d8_device_load32(0x794u);
    const uint32_t mapping = 0x005496A0u + (d3d8_guest_load32(declaration + 4u) & 0x10u);
    d3d8_device_store32(0x20u, base_vertex);
    uint32_t cursor = d3d8_pushbuffer_begin();
    if ((dirty & 0x10u) != 0u) {
        d3d8_guest_store32(cursor, 0x00401760u);
        cursor += 4u;
        for (uint32_t attribute = 0u; attribute < 16u; attribute++) {
            const uint32_t slot = (uint32_t)d3d8_guest_load8(mapping + attribute);
            const uint32_t descriptor = declaration + slot * 16u;
            const uint32_t stream = d3d8_guest_load32(descriptor + 0x14u);
            const uint32_t stride = d3d8_guest_load32(0x003E2BA8u + stream * 12u);
            d3d8_guest_store32(cursor, (stride << 8) + d3d8_guest_load32(descriptor + 0x1Cu));
            cursor += 4u;
        }
    }
    for (uint32_t attribute = 0u; attribute < 16u; attribute++) {
        const uint32_t slot = (uint32_t)d3d8_guest_load8(mapping + attribute);
        const uint32_t descriptor = declaration + slot * 16u;
        if (d3d8_guest_load32(descriptor + 0x1Cu) == 2u) continue;
        const uint32_t stream = d3d8_guest_load32(descriptor + 0x14u);
        const uint32_t row = 0x003E2BA8u + stream * 12u;
        const uint32_t buffer = d3d8_guest_load32(row + 8u);
        if (buffer == 0u) continue;
        const uint32_t address = d3d8_guest_load32(row) * base_vertex + d3d8_guest_load32(buffer + 4u) +
                                 d3d8_guest_load32(descriptor + 0x18u) +
                                 d3d8_guest_load32(row + 4u);
        d3d8_guest_store32(cursor, 0x00041720u + attribute * 4u);
        d3d8_guest_store32(cursor + 4u, address);
        cursor += 8u;
    }
    d3d8_pushbuffer_end(cursor);
}

/* The deferred work of 0x003DEE00, the cascade and then the stream work, planned over the caller's simulation (T546).
 * Each emitter's own preamble may refill, so a caller lays what follows out from `sim->cursor`, not from the entry
 * cursor plus a byte count (the indexed draw used to refuse every planned refill for that reason). */
void d3d8_draw_plan_deferred(d3d8_pushbuffer_sim *sim, uint32_t base_vertex)
{
    const uint32_t dirty = d3d8_guest_load32(D3D8_GLOBAL_DIRTY_MASK);
    if ((dirty & 0x3FFFFF8Fu) != 0u) plan_draw_dirty(dirty, sim);
    uint32_t stream_bytes;
    if (stream_work_bytes(dirty, base_vertex, 0x003D5050u, &stream_bytes))
        d3d8_pushbuffer_sim_site(sim, 0x003D5050u, stream_bytes);
}
void d3d8_draw_flush_streams(uint32_t base_vertex) { flush_draw_streams(base_vertex); }

/* Plan a whole DrawVertices before its first write: the deferred-state cascade, the stream work and
 * the draw packet's sized reservation (0x003D6B30) over one simulated writer. */
static void plan_draw_vertices(uint32_t chunks)
{
    const uint32_t dirty = d3d8_guest_load32(D3D8_GLOBAL_DIRTY_MASK);
    d3d8_pushbuffer_sim sim = d3d8_pushbuffer_sim_start();
    if ((dirty & 0x3FFFFF8Fu) != 0u) plan_draw_dirty(dirty, &sim);
    uint32_t stream_bytes;
    if (stream_work_bytes(dirty, 0u, 0x003D4FB0u, &stream_bytes))
        d3d8_pushbuffer_sim_site(&sim, 0x003D4FB0u, stream_bytes);
    const uint32_t dwords = chunks + 5u;
    (void)d3d8_pushbuffer_sim_reserve(&sim, dwords);
    if (kernel_guest_at_ro(sim.cursor, dwords * 4u) == NULL)
        d3d8_hle_fatal(0x003D4FB0u, "draw packet span %#x+%u is not mapped guest memory",
                       (unsigned)sim.cursor, (unsigned)(dwords * 4u));
}

uint32_t d3d8_draw_vertices(uint32_t primitive, uint32_t first, uint32_t count)
{
    if (count == 0u) {
        d3d8_hle_fatal(0x003D4FB0u,
                       "zero-count DrawVertices underflows the original chunk reservation; "
                       "that malformed path is not implemented");
    }
    const uint32_t chunks = ((count - 1u) >> 8) + 1u;
    plan_draw_vertices(chunks);
    d3d8_resource_capture_scene_draw(0u, primitive, first, count, 0u);
    flush_draw_streams(0u);
    /* 0x003D4FD1: the sized reservation, which may roll the ring over. */
    uint32_t cursor = d3d8_pushbuffer_reserve(chunks + 5u);
    d3d8_guest_store32(cursor, 0x000417FCu);
    d3d8_guest_store32(cursor + 4u, primitive);
    d3d8_guest_store32(cursor + 8u, (chunks << 18) + 0x40001810u);
    cursor += 12u;
    uint32_t remaining = count;
    for (uint32_t chunk = 0u; chunk < chunks; chunk++) {
        const uint32_t vertices = remaining > 256u ? 256u : remaining;
        d3d8_guest_store32(cursor, ((vertices - 1u) << 24) | first);
        cursor += 4u;
        first += 256u;
        remaining -= vertices;
    }
    d3d8_guest_store32(cursor, 0x000417FCu);
    d3d8_guest_store32(cursor + 4u, 0u);
    cursor += 8u;
    d3d8_pushbuffer_end(cursor);
    return cursor;
}

static uint32_t handler_draw_vertices(void *context)
{
    uint32_t arguments[3];
    for (unsigned i = 0u; i < 3u; i++) {
        if (!kernel_frame_arg((const kernel_call_frame *)context, i, &arguments[i])) {
            d3d8_hle_fatal(0x003D4FB0u, "DrawVertices argument %u cannot be read", i);
        }
    }
    return d3d8_draw_vertices(arguments[0], arguments[1], arguments[2]);
}

typedef enum {
    ALIAS_OK,
    ALIAS_HEADER_UNREADABLE,
    ALIAS_BINDING_MODIFIED,
    ALIAS_ALLOCATION_STALE
} alias_state;

/* The one verification of a recorded binding, shared by the resolver and by the reuse of a dead
 * record: the header still carries the identity it was registered with and the allocation it
 * named is still the live contiguous region it was. */
static alias_state alias_verify(const resource_alias *record)
{
    uint32_t words[5];
    if (kernel_guest_at_ro(record->header, 20u) == NULL ||
        !kernel_guest_read_bytes(record->header, words, sizeof(words)))
        return ALIAS_HEADER_UNREADABLE;
    if ((words[0] & IMMUTABLE_COMMON_MASK) != record->common_identity ||
        words[1] != record->data || words[3] != record->format || words[4] != record->size)
        return ALIAS_BINDING_MODIFIED;
    const guest_region *region = kmem_region_at(record->region_address);
    if (region == NULL || !region->contiguous || region->state != MEM_COMMIT ||
        region->physical != record->region_physical || region->generation != record->allocation_generation ||
        region->size != record->region_size ||
        region->requested != record->region_requested)
        return ALIAS_ALLOCATION_STALE;
    return ALIAS_OK;
}

/* A slot for a new record. Never drops: grow, then reuse a record that cannot verify any more,
 * then stop by name. */
static resource_alias *alias_new_slot(bool fatal)
{
    if (alias_count == alias_capacity && alias_capacity < ALIAS_LIMIT) {
        const size_t grown = alias_capacity == 0u ? ALIAS_INITIAL_CAPACITY : alias_capacity * 2u;
        resource_alias *table = realloc(aliases, grown * sizeof(*aliases));
        if (table == NULL) {
            if (!fatal) return NULL;
            pthread_mutex_unlock(&alias_lock);
            d3d8_hle_fatal(0x003D4D70u, "cannot grow the registered allocation alias table to %zu", grown);
        }
        aliases = table;
        alias_capacity = grown;
    }
    if (alias_count < alias_capacity) return &aliases[alias_count++];
    for (size_t i = 0u; i < alias_count; i++) {
        if (!aliases[i].valid || alias_verify(&aliases[i]) != ALIAS_OK) return &aliases[i];
    }
    if (!fatal) return NULL;
    pthread_mutex_unlock(&alias_lock);
    d3d8_hle_fatal(0x003D4D70u,
                   "registered allocation alias table is full: %u resources still verify, none can be reused",
                   (unsigned)ALIAS_LIMIT);
    return NULL;
}

static alias_rejection *rejection_find(uint32_t header)
{
    for (size_t i = 0u; i < ALIAS_LIMIT; i++) {
        if (rejected_aliases[i].reason != ALIAS_REJECTION_NONE &&
            rejected_aliases[i].header == header)
            return &rejected_aliases[i];
    }
    return NULL;
}

static void rejection_clear(uint32_t header)
{
    alias_rejection *rejection = rejection_find(header);
    if (rejection != NULL) {
        memset(rejection, 0, sizeof(*rejection));
        rejection_count--;
    }
}

static void rejection_record(uint32_t header, uint32_t base, uint32_t offset,
                             alias_rejection_reason reason)
{
    alias_rejection *rejection = rejection_find(header);
    if (rejection == NULL) {
        if (rejection_count < ALIAS_LIMIT) {
            for (size_t i = 0u; i < ALIAS_LIMIT; i++) {
                if (rejected_aliases[i].reason == ALIAS_REJECTION_NONE) {
                    rejection = &rejected_aliases[i];
                    rejection_count++;
                    break;
                }
            }
        } else {
            rejection = &rejected_aliases[rejection_next];
            rejection_next = (rejection_next + 1u) % ALIAS_LIMIT;
        }
    }
    if (rejection != NULL)
        *rejection = (alias_rejection){.header = header, .base = base, .offset = offset,
                                       .reason = reason};
}

static const char *rejection_description(alias_rejection_reason reason)
{
    switch (reason) {
    case ALIAS_REJECTION_NO_CONTIGUOUS_REGION:
        return "base has no committed contiguous region";
    case ALIAS_REJECTION_REGION_TOO_LARGE:
        return "region is too large for the 28-bit alias";
    case ALIAS_REJECTION_DATA_OUTSIDE_REGION:
        return "Data offset is outside the region";
    case ALIAS_REJECTION_VIRTUAL_RETAINED:
        return "resource is virtual retained and has no physical alias";
    case ALIAS_REJECTION_COPY_UNVERIFIED:
        return "resource header copy has no verified complete live source";
    case ALIAS_REJECTION_NONE: break;
    }
    return "Register recorded no alias";
}

static void record_registered_alias_unlocked(uint32_t header, uint32_t base, uint32_t offset,
                                    uint32_t common, uint32_t data)
{
    resource_alias *record = NULL;
    for (size_t i = 0u; i < alias_count; i++) {
        if (aliases[i].header == header) { record = &aliases[i]; break; }
    }
    if (record != NULL) record->valid = false; /* Every Register invalidates the prior binding. */
    if (rejection_count != 0u) rejection_clear(header);
    const guest_region *region = kmem_region_containing(base);
    if (region == NULL || !region->contiguous || region->state != MEM_COMMIT) {
        const alias_rejection_reason reason = ALIAS_REJECTION_NO_CONTIGUOUS_REGION;
        rejection_record(header, base, offset, reason);
        return;
    }
    if (region->requested >= 0x10000000u) {
        const alias_rejection_reason reason = ALIAS_REJECTION_REGION_TOO_LARGE;
        rejection_record(header, base, offset, reason);
        return;
    }
    if (kernel_guest_at_ro(header, 20u) == NULL) return;
    if ((common & 0x70000u) == 0x20000u) {
        const alias_rejection_reason reason = ALIAS_REJECTION_VIRTUAL_RETAINED;
        rejection_record(header, base, offset, reason);
        return;
    }
    const uint64_t virtual_data = (uint64_t)base + offset;
    const uint64_t end = (uint64_t)region->address + region->requested;
    if (virtual_data < region->address || virtual_data >= end || virtual_data > UINT32_MAX) {
        const alias_rejection_reason reason = ALIAS_REJECTION_DATA_OUTSIDE_REGION;
        rejection_record(header, base, offset, reason);
        return;
    }
    if (record == NULL) record = alias_new_slot(true);
    record->header = header;
    record->common_identity = common & IMMUTABLE_COMMON_MASK;
    record->data = data;
    record->format = d3d8_guest_load32(header + 12u);
    record->size = d3d8_guest_load32(header + 16u);
    record->region_address = region->address;
    record->region_physical = region->physical;
    record->allocation_generation = region->generation;
    record->region_requested = region->requested;
    record->region_size = region->size;
    record->virtual_data = (uint32_t)virtual_data;
    record->identity = ++alias_identity;
    record->valid = true;
}

static void record_registered_alias(uint32_t header, uint32_t base, uint32_t offset,
                                    uint32_t common, uint32_t data)
{
    pthread_mutex_lock(&alias_lock);
    record_registered_alias_unlocked(header, base, offset, common, data);
    pthread_mutex_unlock(&alias_lock);
}

typedef struct {
    resource_alias record;
    uint32_t expected[5];
    bool verified;
    uint32_t source_header;
} copied_alias;

typedef struct {
    size_t count;
    copied_alias entries[];
} alias_copy;

static void *resource_copy_begin_unlocked(uint32_t destination, uint32_t source, uint32_t bytes)
{
    if (bytes == 0u || (uint64_t)source + bytes > UINT32_MAX ||
        (uint64_t)destination + bytes > UINT32_MAX ||
        kernel_guest_at_ro(source, bytes) == NULL || kernel_guest_at_ro(destination, bytes) == NULL)
        return NULL;
    size_t count = 0u;
    for (size_t i = 0u; i < alias_count; i++) {
        const resource_alias *record = &aliases[i];
        if ((uint64_t)record->header < (uint64_t)source + bytes &&
            (uint64_t)record->header + 20u > source) count++;
    }
    if (count == 0u) return NULL;
    alias_copy *copy = malloc(sizeof(*copy) + count * sizeof(copy->entries[0]));
    if (copy == NULL) return NULL;
    copy->count = 0u;
    for (size_t i = 0u; i < alias_count; i++) {
        const resource_alias *record = &aliases[i];
        if ((uint64_t)record->header >= (uint64_t)source + bytes ||
            (uint64_t)record->header + 20u <= source) continue;
        const int64_t destination_header = (int64_t)destination + record->header - source;
        if (destination_header <= 0 || destination_header + 20 > UINT32_MAX) continue;
        copied_alias *entry = &copy->entries[copy->count];
        if (!kernel_guest_read_bytes(record->header, entry->expected, sizeof(entry->expected))) continue;
        entry->record = *record;
        entry->source_header = record->header;
        entry->record.header = (uint32_t)destination_header;
        entry->verified = record->valid && record->header >= source &&
            (uint64_t)record->header + 20u <= (uint64_t)source + bytes && alias_verify(record) == ALIAS_OK;
        copy->count++;
    }
    return copy;
}

static size_t resource_copy_end_unlocked(void *transaction)
{
    alias_copy *copy = transaction;
    if (copy == NULL) return 0u;
    size_t transferred = 0u;
    for (size_t i = 0u; i < copy->count; i++) {
        const copied_alias *entry = &copy->entries[i];
        uint32_t actual[5];
        if (!entry->verified || !kernel_guest_read_bytes(entry->record.header, actual, sizeof(actual)) ||
            memcmp(actual, entry->expected, sizeof(actual)) != 0 || alias_verify(&entry->record) != ALIAS_OK) {
            rejection_record(entry->record.header, 0u, 0u, ALIAS_REJECTION_COPY_UNVERIFIED);
            continue;
        }
        resource_alias *target = NULL;
        for (size_t j = 0u; j < alias_count; j++) {
            if (aliases[j].header == entry->record.header) { target = &aliases[j]; break; }
        }
        if (target == NULL) target = alias_new_slot(false);
        if (target == NULL) {
            rejection_record(entry->record.header, 0u, 0u, ALIAS_REJECTION_COPY_UNVERIFIED);
            continue;
        }
        *target = entry->record;
        target->identity = ++alias_identity;
        rejection_clear(target->header);
        transferred++;
        static unsigned traced;
        if (traced < 32u && getenv("TSFP_RESOURCE_COPY_TRACE") != NULL) {
            fprintf(stderr, "RESOURCE_COPY source=%08x destination=%08x Data=%08x Format=%08x VA=%08x identity=%llu\n",
                    entry->source_header, target->header, target->data, target->format, target->virtual_data,
                    (unsigned long long)target->identity);
            traced++;
        }
    }
    free(copy);
    return transferred;
}

void *d3d8_resource_copy_begin(uint32_t destination, uint32_t source, uint32_t bytes)
{
    pthread_mutex_lock(&alias_lock);
    void *copy = resource_copy_begin_unlocked(destination, source, bytes);
    pthread_mutex_unlock(&alias_lock);
    return copy;
}

size_t d3d8_resource_copy_end(void *transaction)
{
    if (transaction == NULL) return 0u;
    pthread_mutex_lock(&alias_lock);
    const size_t transferred = resource_copy_end_unlocked(transaction);
    pthread_mutex_unlock(&alias_lock);
    return transferred;
}

size_t d3d8_resource_alias_count(void)
{
    size_t live = 0u;
    pthread_mutex_lock(&alias_lock);
    for (size_t i = 0u; i < alias_count; i++) live += aliases[i].valid ? 1u : 0u;
    pthread_mutex_unlock(&alias_lock);
    return live;
}

static d3d8_resource_alias_result resource_try_alias_unlocked(uint32_t header, uint32_t data, uint32_t bytes,
                                                  uint32_t *address, uint64_t *identity,
                                                  const char **refusal)
{
    const resource_alias *record = NULL;
    for (size_t i = 0u; i < alias_count; i++) {
        if (aliases[i].header != header) continue;
        if (record != NULL) {
            *refusal = "ambiguous registered resource header";
            return D3D8_RESOURCE_ALIAS_REFUSED;
        }
        record = &aliases[i];
    }
    if (record == NULL) {
        const alias_rejection *rejection = rejection_find(header);
        if (rejection == NULL) return D3D8_RESOURCE_ALIAS_ABSENT;
        *refusal = rejection_description(rejection->reason);
        return D3D8_RESOURCE_ALIAS_REFUSED;
    }
    if (!record->valid) {
        *refusal = "registered resource binding is retired";
        return D3D8_RESOURCE_ALIAS_REFUSED;
    }
    switch (alias_verify(record)) {
    case ALIAS_HEADER_UNREADABLE: *refusal = "registered resource header is unreadable"; break;
    case ALIAS_BINDING_MODIFIED: *refusal = "registered resource binding was modified"; break;
    case ALIAS_ALLOCATION_STALE: *refusal = "registered resource allocation is stale"; break;
    case ALIAS_OK: {
        const uint64_t delta = (uint64_t)data - record->data;
        const uint64_t at = (uint64_t)record->virtual_data + delta;
        const uint64_t end = (uint64_t)record->region_address + record->region_requested;
        if (data > 0x0FFFFFFFu || data < record->data || bytes == 0u ||
            (uint64_t)data + bytes > 0x10000000u || at + bytes > end ||
            at + bytes > UINT32_MAX || kernel_guest_at_ro((uint32_t)at, bytes) == NULL) {
            *refusal = "registered resource span exceeds its requested allocation";
            break;
        }
        *address = (uint32_t)at;
        *identity = record->identity;
        return D3D8_RESOURCE_ALIAS_RESOLVED;
    }
    }
    return D3D8_RESOURCE_ALIAS_REFUSED;
}

d3d8_resource_alias_result d3d8_resource_try_alias(uint32_t header, uint32_t data, uint32_t bytes,
                                                  uint32_t *address, uint64_t *identity,
                                                  const char **refusal)
{
    pthread_mutex_lock(&alias_lock);
    const d3d8_resource_alias_result result = resource_try_alias_unlocked(header,data,bytes,address,identity,refusal);
    pthread_mutex_unlock(&alias_lock);
    return result;
}

uint32_t d3d8_resource_resolve_registered_alias_at(uint32_t entry, uint32_t header,
                                                    uint32_t data, uint32_t bytes)
{
    resource_alias snapshot;
    const resource_alias *record = NULL;
    alias_rejection rejected = {0};
    pthread_mutex_lock(&alias_lock);
    for (size_t i = 0u; i < alias_count; i++) {
        if (aliases[i].header == header && aliases[i].valid) { snapshot=aliases[i]; record=&snapshot; break; }
    }
    const alias_rejection *found = rejection_find(header);
    if (found != NULL) rejected=*found;
    pthread_mutex_unlock(&alias_lock);
    if (record == NULL || kernel_guest_at_ro(header, 20u) == NULL) {
        const alias_rejection *rejection = rejected.reason != ALIAS_REJECTION_NONE ? &rejected : NULL;
        if (rejection != NULL && kernel_guest_at_ro(header, 20u) != NULL)
            d3d8_hle_fatal(entry,
                           "resource %#x was registered at base %#x but no alias was recorded: %s (Data offset %#x)",
                           header, rejection->base, rejection_description(rejection->reason),
                           rejection->offset);
        d3d8_hle_fatal(entry, "resource %#x has no verified registered allocation alias", header);
    }
    switch (alias_verify(record)) {
    case ALIAS_OK: break;
    case ALIAS_HEADER_UNREADABLE:
        d3d8_hle_fatal(entry, "registered resource header is unreadable");
        break;
    case ALIAS_BINDING_MODIFIED:
        d3d8_hle_fatal(entry, "registered resource %#x binding was modified", header);
        break;
    case ALIAS_ALLOCATION_STALE:
        d3d8_hle_fatal(entry, "registered resource %#x allocation is stale", header);
        break;
    }
    const guest_region *region = kmem_region_at(record->region_address);
    if (data > 0x0FFFFFFFu)
        d3d8_hle_fatal(entry, "surface Data %#x is outside the 28-bit alias domain", data);
    const uint32_t delta = (data - record->data) & 0x0FFFFFFFu;
    const uint64_t address = (uint64_t)record->virtual_data + delta;
    const uint64_t end = (uint64_t)region->address + region->requested;
    if (bytes == 0u || address + bytes > end || address + bytes > UINT32_MAX ||
        kernel_guest_at_ro((uint32_t)address, bytes) == NULL)
        d3d8_hle_fatal(entry, "surface alias span exceeds its requested contiguous allocation");
    return (uint32_t)address;
}

uint32_t d3d8_resource_virtual_of_registered_data(uint32_t data)
{
    uint32_t address=0u;
    pthread_mutex_lock(&alias_lock);
    for (size_t i = 0u; i < alias_count; i++) {
        if (aliases[i].valid && aliases[i].data == data) { address=aliases[i].virtual_data; break; }
    }
    pthread_mutex_unlock(&alias_lock);
    return address;
}

uint32_t d3d8_resource_resolve_registered_alias(uint32_t header, uint32_t data, uint32_t bytes)
{
    return d3d8_resource_resolve_registered_alias_at(0x003D4AC0u, header, data, bytes);
}

uint32_t d3d8_register_resource(uint32_t header, uint32_t base)
{
    /* Validate the complete two-word input/output span before any mutation,
     * including a header that straddles an unmapped page or wraps the address. */
    if (kernel_guest_at_ro(header, 8u) == NULL)
        d3d8_hle_fatal(0x003D4D70u, "resource header %#x is not mapped for 8 bytes", header);
    const uint32_t offset = d3d8_guest_load32(header + 4u);
    const uint32_t common = d3d8_guest_load32(header);
    uint32_t data = offset + base;
    if ((common & 0x70000u) != 0x20000u) data &= 0x0FFFFFFFu;
    /* The binding is recorded BEFORE Data is relocated: a table that cannot take it stops here with
     * the header untouched (T755). */
    record_registered_alias(header, base, offset, common, data);
    d3d8_guest_store32(header + 4u, data);
    return data;
}

static uint32_t handler_register_resource(void *context)
{
    uint32_t arguments[2];
    for (unsigned i = 0u; i < 2u; i++) {
        if (!kernel_frame_arg((const kernel_call_frame *)context, i, &arguments[i]))
            d3d8_hle_fatal(0x003D4D70u, "resource registration argument %u is unreadable", i);
    }
    return d3d8_register_resource(arguments[0], arguments[1]);
}

size_t d3d8_resource_draw_register(void)
{
    return (size_t)d3d8_hle_register(0x003D4FB0u, handler_draw_vertices) +
           (size_t)d3d8_hle_register(0x003D4D70u, handler_register_resource);
}
