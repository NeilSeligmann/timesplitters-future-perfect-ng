/* SPDX-License-Identifier: GPL-3.0-or-later */
/* Include the real sink so the oracle calls its private production function. */
#include "../../../src/host/present_sink.c"

size_t stretch_ring_oracle(int16_t *ring, size_t capacity, uint16_t channels, size_t *head,
                           size_t *fill, int16_t *scratch, size_t frames, size_t requested)
{
    present_audio_sink sink = {0};
    sink.ring = ring;
    sink.ring_frames = capacity;
    sink.channels = channels;
    sink.ring_head = *head;
    sink.ring_fill = *fill;
    sink.stretch_scratch = scratch;
    stretch_load(&sink, frames);
    size_t consumed = 0;
    stretch_consume(&sink, requested, &consumed);
    *head = sink.ring_head;
    *fill = sink.ring_fill;
    return consumed;
}

void ramp_oracle(int16_t *samples, size_t frames, uint16_t channels, uint32_t *gain, uint32_t target)
{
    present_audio_sink sink = {0};
    sink.stretch_fifo = samples;
    sink.stretch_fifo_len = frames;
    sink.channels = channels;
    sink.stretch_gain_q16 = *gain;
    stretch_ramp(&sink, target);
    *gain = sink.stretch_gain_q16;
}

void fill_oracle(size_t fill, size_t window, size_t pull, double *value, bool *valid)
{
    present_audio_sink sink = {0};
    sink.ring_fill = fill;
    sink.fill_smooth_frames = window;
    sink.fill_ema = *value;
    sink.fill_ema_valid = *valid;
    fill_smooth_update(&sink, pull);
    *value = sink.fill_ema;
    *valid = sink.fill_ema_valid;
}

uint32_t continuous_ratio_oracle(size_t fill, size_t low, uint32_t floor, uint32_t *ratio)
{
    present_audio_sink sink = {0};
    sink.ring_fill = fill;
    sink.slow_low_frames = low;
    sink.slow_floor_q16 = floor;
    sink.slow_ratio_q16 = *ratio;
    uint32_t result = continuous_ratio(&sink);
    *ratio = sink.slow_ratio_q16;
    return result;
}

size_t latency_excess_oracle(size_t fill, size_t target, size_t hysteresis, bool *trimming)
{
    present_audio_sink sink = {0};
    sink.ring_fill = fill;
    sink.latency_target_frames = target;
    sink.latency_hysteresis_frames = hysteresis;
    sink.latency_trimming = *trimming;
    size_t result = latency_excess(&sink);
    *trimming = sink.latency_trimming;
    return result;
}

size_t silence_oracle(int16_t *ring, size_t frames, uint16_t channels, size_t *head,
                      size_t *fill, size_t excess, uint64_t *pulled, uint64_t *silence)
{
    present_audio_sink sink = {0};
    sink.ring = ring;
    sink.ring_frames = frames;
    sink.channels = channels;
    sink.ring_head = *head;
    sink.ring_fill = *fill;
    sink.counts.pulled = *pulled;
    sink.counts.latency_silence_frames = *silence;
    size_t result = latency_skip_silence(&sink, excess);
    *head = sink.ring_head;
    *fill = sink.ring_fill;
    *pulled = sink.counts.pulled;
    *silence = sink.counts.latency_silence_frames;
    return result;
}

typedef struct {
    size_t fill, need, smooth_window;
    double ema;
    bool ema_valid;
    size_t low;
    uint32_t floor, ratio, engage, leave, down, up;
    uint64_t engagements, panic_pulls, max_step;
} ratio_state;

uint32_t stretch_ratio_oracle(ratio_state *s, size_t pull)
{
    present_audio_sink sink = {0};
    sink.stretch = audio_stretch_create(2, 160);
    if (!sink.stretch) abort();
    s->need = audio_stretch_input_needed(sink.stretch, audio_stretch_search(sink.stretch));
    sink.ring_fill = s->fill;
    sink.fill_smooth_frames = s->smooth_window;
    sink.fill_ema = s->ema;
    sink.fill_ema_valid = s->ema_valid;
    sink.slow_low_frames = s->low;
    sink.slow_floor_q16 = s->floor;
    sink.slow_ratio_q16 = s->ratio;
    sink.stretch_engage_q16 = s->engage;
    sink.stretch_leave_q16 = s->leave;
    sink.stretch_slew_down_q16 = s->down;
    sink.stretch_slew_up_q16 = s->up;
    sink.counts.stretch_engagements = s->engagements;
    sink.counts.stretch_panic_pulls = s->panic_pulls;
    sink.counts.stretch_max_step_q16 = s->max_step;
    uint32_t result = stretch_ratio(&sink, pull);
    s->ratio = sink.slow_ratio_q16;
    s->engagements = sink.counts.stretch_engagements;
    s->panic_pulls = sink.counts.stretch_panic_pulls;
    s->max_step = sink.counts.stretch_max_step_q16;
    audio_stretch_destroy(sink.stretch);
    return result;
}

size_t stretch_fifo_active_oracle(int16_t *fifo, uint16_t channels, size_t *position,
                                  size_t *length, int16_t *output, size_t frames, bool active)
{
    present_audio_sink sink = {0};
    sink.stretch = audio_stretch_create(channels, 160);
    if (!sink.stretch) abort();
    sink.channels = channels;
    sink.stretch_fifo = fifo;
    sink.stretch_fifo_pos = *position;
    sink.stretch_fifo_len = *length;
    sink.stretch_active = active;
    size_t consumed = 0;
    size_t result = stretch_serve(&sink, output, frames, 65536u, &consumed);
    *position = sink.stretch_fifo_pos;
    *length = sink.stretch_fifo_len;
    audio_stretch_destroy(sink.stretch);
    return result;
}

size_t stretch_fifo_oracle(int16_t *fifo, uint16_t channels, size_t *position,
                           size_t *length, int16_t *output, size_t frames)
{
    return stretch_fifo_active_oracle(fifo, channels, position, length, output, frames, false);
}

void stretch_hold_oracle(size_t *run, size_t full, size_t max, uint32_t *gain,
                         uint64_t *held, uint64_t *decayed)
{
    present_audio_sink sink = {0};
    int16_t input[320] = {0}, fifo[160] = {0}, scratch[1] = {0};
    sink.stretch = audio_stretch_create(1, 160);
    if (!sink.stretch || !audio_stretch_prime(sink.stretch, input, 320, fifo)) abort();
    sink.channels = 1;
    sink.stretch_fifo = fifo;
    sink.stretch_scratch = scratch;
    sink.stretch_hold_run = *run;
    sink.stretch_hold_full_frames = full;
    sink.stretch_hold_max_frames = max;
    sink.stretch_gain_q16 = *gain;
    sink.stretch_active = true;
    sink.counts.stretch_hold_frames = *held;
    sink.counts.stretch_decay_frames = *decayed;
    size_t consumed = 0;
    stretch_block(&sink, 0, &consumed);
    *run = sink.stretch_hold_run;
    *gain = sink.stretch_gain_q16;
    *held = sink.counts.stretch_hold_frames;
    *decayed = sink.counts.stretch_decay_frames;
    audio_stretch_destroy(sink.stretch);
}

void stretch_fade_oracle(size_t capacity, size_t *head, size_t *fill,
                         size_t *consumed, uint64_t *fadeouts, bool *active)
{
    present_audio_sink sink = {0};
    int16_t input[320] = {0}, fifo[160] = {0}, scratch[1] = {0};
    sink.stretch = audio_stretch_create(1, 160);
    if (!sink.stretch || !audio_stretch_prime(sink.stretch, input, 320, fifo)) abort();
    sink.channels = 1;
    sink.stretch_fifo = fifo;
    sink.stretch_scratch = scratch;
    sink.ring_frames = capacity;
    sink.ring_head = *head;
    sink.ring_fill = *fill;
    sink.stretch_hold_run = 160;
    sink.stretch_hold_max_frames = 160;
    sink.stretch_active = true;
    sink.counts.stretch_fadeouts = *fadeouts;
    stretch_block(&sink, 0, consumed);
    *head = sink.ring_head;
    *fill = sink.ring_fill;
    *fadeouts = sink.counts.stretch_fadeouts;
    *active = sink.stretch_active;
    audio_stretch_destroy(sink.stretch);
}

typedef struct {
    uint64_t position_q16;
    size_t tail, head, fill, consumed, hold_run;
    uint64_t blocks;
} progress_state;

size_t stretch_progress_oracle(progress_state *state, uint32_t ratio)
{
    present_audio_sink sink = {0};
    int16_t ring[1024] = {0}, scratch[1024] = {0}, fifo[160] = {0};
    int16_t prime[320] = {0}, second_fifo[160] = {0};
    audio_stretch *other = audio_stretch_create(1, 160);
    sink.stretch = audio_stretch_create(1, 160);
    if (!other || !sink.stretch || !audio_stretch_prime(other, prime, 320, second_fifo) ||
        !audio_stretch_prime(sink.stretch, prime, 320, fifo)) abort();
    sink.channels = 1;
    sink.ring = ring;
    sink.ring_frames = 1024;
    sink.ring_head = state->head;
    sink.ring_fill = state->fill;
    sink.stretch_scratch = scratch;
    sink.stretch_scratch_frames = 1024;
    sink.stretch_fifo = fifo;
    sink.stretch_pos_q16 = state->position_q16;
    sink.stretch_tail_src = state->tail;
    sink.stretch_hold_run = state->hold_run;
    sink.counts.stretch_blocks = state->blocks;
    size_t chosen = 0, nominal = (size_t)(state->position_q16 >> 16);
    if (!audio_stretch_next(other, ring, state->fill, nominal, second_fifo, &chosen)) abort();
    stretch_block(&sink, ratio, &state->consumed);
    state->position_q16 = sink.stretch_pos_q16;
    state->tail = sink.stretch_tail_src;
    state->head = sink.ring_head;
    state->fill = sink.ring_fill;
    state->hold_run = sink.stretch_hold_run;
    state->blocks = sink.counts.stretch_blocks;
    audio_stretch_destroy(other);
    audio_stretch_destroy(sink.stretch);
    return chosen;
}

typedef struct {
    size_t head, fill, consumed, fifo_pos, fifo_len;
    bool active;
} leave_state;

bool stretch_leave_oracle(leave_state *state, size_t tail)
{
    present_audio_sink sink = {0};
    int16_t ring[1024] = {0}, scratch[1024] = {0}, fifo[160] = {0}, prime[320] = {0};
    sink.stretch = audio_stretch_create(1, 160);
    if (!sink.stretch || !audio_stretch_prime(sink.stretch, prime, 320, fifo)) abort();
    sink.channels = 1;
    sink.ring = ring;
    sink.ring_frames = 1024;
    sink.ring_head = state->head;
    sink.ring_fill = state->fill;
    sink.stretch_scratch = scratch;
    sink.stretch_scratch_frames = 1024;
    sink.stretch_fifo = fifo;
    sink.stretch_fifo_pos = state->fifo_pos;
    sink.stretch_fifo_len = state->fifo_len;
    sink.stretch_tail_src = tail;
    sink.stretch_active = state->active;
    bool result = stretch_leave(&sink, &state->consumed);
    state->head = sink.ring_head;
    state->fill = sink.ring_fill;
    state->fifo_pos = sink.stretch_fifo_pos;
    state->fifo_len = sink.stretch_fifo_len;
    state->active = sink.stretch_active;
    audio_stretch_destroy(sink.stretch);
    return result;
}

typedef struct {
    size_t fill, fifo_pos, fifo_len, tail;
    uint64_t position_q16;
    size_t hold_run;
    uint32_t gain_q16;
    bool active, fade_in_pending;
} prime_state;

void stretch_prime_oracle(prime_state *state, uint32_t ratio)
{
    present_audio_sink sink = {0};
    int16_t ring[1024] = {0}, scratch[1024] = {0}, fifo[160] = {0}, out[1] = {0};
    sink.stretch = audio_stretch_create(1, 160);
    if (!sink.stretch) abort();
    sink.channels = 1;
    sink.ring = ring;
    sink.ring_frames = 1024;
    sink.ring_fill = state->fill;
    sink.stretch_scratch = scratch;
    sink.stretch_fifo = fifo;
    sink.stretch_fifo_pos = state->fifo_pos;
    sink.stretch_fifo_len = state->fifo_len;
    sink.stretch_tail_src = state->tail;
    sink.stretch_pos_q16 = state->position_q16;
    sink.stretch_hold_run = state->hold_run;
    sink.stretch_gain_q16 = state->gain_q16;
    sink.stretch_active = state->active;
    sink.fade_in_pending = state->fade_in_pending;
    size_t consumed = 0;
    stretch_serve(&sink, out, 0, ratio, &consumed);
    state->fifo_pos = sink.stretch_fifo_pos;
    state->fifo_len = sink.stretch_fifo_len;
    state->tail = sink.stretch_tail_src;
    state->position_q16 = sink.stretch_pos_q16;
    state->hold_run = sink.stretch_hold_run;
    state->gain_q16 = sink.stretch_gain_q16;
    state->active = sink.stretch_active;
    state->fade_in_pending = sink.fade_in_pending;
    audio_stretch_destroy(sink.stretch);
}
