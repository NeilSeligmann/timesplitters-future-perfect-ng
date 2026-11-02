/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "dsound_effects_descriptor.h"
#include <string.h>

static void put_word(uint8_t *at, uint32_t value)
{
    for (unsigned i = 0u; i < 4u; ++i) at[i] = (uint8_t)(value >> (i * 8u));
}

bool dsound_effects_descriptor_build(const dsound_effects_metadata *metadata,
                                     uint32_t code_base, uint32_t state_base,
                                     uint32_t workspace_base, uint32_t y_base,
                                     uint8_t output[DSOUND_EFFECTS_DESCRIPTOR_BYTES])
{
    if (metadata == NULL || output == NULL || metadata->map_count > DSOUND_EFFECTS_MAX_MAPS)
        return false;
    uint8_t descriptor[DSOUND_EFFECTS_DESCRIPTOR_BYTES] = {0};
    put_word(descriptor, metadata->map_count);
    put_word(descriptor + 4u, metadata->workspace_bytes);
    for (uint32_t i = 0u; i < metadata->map_count; ++i) {
        const dsound_effects_map *map = &metadata->maps[i];
        uint8_t *entry = descriptor + 8u + i * 32u;
        const uint32_t words[8] = {
            code_base + map->code_offset - metadata->code_offset, map->code_bytes,
            state_base + map->state_offset - metadata->state_offset, map->state_bytes,
            y_base, map->y_bytes,
            workspace_base + map->workspace_offset, map->workspace_bytes,
        };
        for (unsigned j = 0u; j < 8u; ++j) put_word(entry + j * 4u, words[j]);
    }
    memcpy(output, descriptor, sizeof(descriptor));
    return true;
}
