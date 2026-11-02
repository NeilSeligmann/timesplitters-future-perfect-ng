/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_AUDIO_DSOUND_EFFECTS_DESCRIPTOR_H
#define TSFP_AUDIO_DSOUND_EFFECTS_DESCRIPTOR_H
#include "dsound_effects_metadata.h"

#define DSOUND_EFFECTS_DESCRIPTOR_BYTES 296u

/* C-host oracle for mapping validated image ranges into owned guest views. */
bool dsound_effects_descriptor_build(const dsound_effects_metadata *metadata,
                                     uint32_t code_base, uint32_t state_base,
                                     uint32_t workspace_base, uint32_t y_base,
                                     uint8_t output[DSOUND_EFFECTS_DESCRIPTOR_BYTES]);
#endif
