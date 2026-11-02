/* SPDX-License-Identifier: GPL-3.0-or-later
 * T2023: the audio device seam of present_sink.c (audio_sink_sdl.h) for the Bevy shell. Linked into tsfp_host_bevylink ahead of
 * the tsfp_host_options archive, so audio_sink_sdl.c (the SDL3 device) is never pulled in. Instead of an SDL audio stream the
 * ring is pulled by the Bevy audio thread: crates/tsfp-bevy/src/audio.rs registers a rodio Decodable source that calls the
 * pull function handed over here. Same pull model as the SDL callback (a shortfall is silence, counted as an underrun by the
 * sink), the mute flag still pulls so the ring drains and the audio clock stays the master. */
#include "audio_sink_sdl.h"
#include "bevy_sink.h"

#include <stdatomic.h>
#include <string.h>

static atomic_bool s_mute;

void audio_sink_sdl_set_mute(bool mute)
{
    atomic_store(&s_mute, mute);
}

static size_t pull(void *context, int16_t *out, size_t frames)
{
    /* `channels` is 2 for the host (48000 Hz stereo S16); the Rust side asked for the same layout at attach. */
    const size_t real = present_audio_sink_pull((present_audio_sink *)context, out, frames);
    if (atomic_load(&s_mute)) {
        memset(out, 0, frames * 2u * sizeof(int16_t));
    }
    return real;
}

bool audio_sink_sdl_start(present_audio_sink *sink, uint32_t sample_rate, uint16_t channels, void **out_device,
                          const char **error)
{
    if (channels != 2u) {
        *error = "the Bevy audio shell plays stereo only";
        return false;
    }
    tsfp_bevy_audio_attach(pull, sink, sample_rate, channels);
    *out_device = sink; /* any non-NULL token: audio_sink_sdl_stop gets it back */
    return true;
}

void audio_sink_sdl_stop(void *device)
{
    if (device != NULL) {
        tsfp_bevy_audio_detach(); /* returns once no Bevy audio callback is inside pull() any more, before the ring goes away */
    }
}

size_t audio_sink_sdl_device_frames(void *device)
{
    (void)device;
    return 0u;
}
