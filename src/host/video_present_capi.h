#ifndef TSFP_VIDEO_PRESENT_CAPI_H
#define TSFP_VIDEO_PRESENT_CAPI_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* Calls must be serialized. A returned picture is borrowed until the next successful
 * vblank or destruction; submissions copy their input. */
typedef struct tsfp_video_latch tsfp_video_latch;
typedef struct {
    uint64_t number;
    uint32_t width, height;
    const uint8_t *pixels;
    size_t len;
} tsfp_video_picture;

tsfp_video_latch *tsfp_video_latch_new(void);
void tsfp_video_latch_free(tsfp_video_latch *latch);
bool tsfp_video_latch_submit(tsfp_video_latch *latch, uint64_t number, uint32_t width,
                             uint32_t height, const uint8_t *pixels, size_t len);
bool tsfp_video_latch_vblank(tsfp_video_latch *latch, tsfp_video_picture *out);
/* Borrowed until the next successful submit, vblank, or latch destruction. */
bool tsfp_video_latch_pending(const tsfp_video_latch *latch, tsfp_video_picture *out);

typedef struct tsfp_video_playback tsfp_video_playback;
typedef struct {
    uint64_t dropped, late_shown;
    size_t queue_max;
    uint64_t queue_timeouts, model_gap_max_ns, model_gap_after;
} tsfp_video_playback_timing_snapshot;
tsfp_video_playback *tsfp_video_playback_new(void);
void tsfp_video_playback_free(tsfp_video_playback *playback);
size_t tsfp_video_playback_len(const tsfp_video_playback *playback);
bool tsfp_video_playback_timing(const tsfp_video_playback *playback,
                                tsfp_video_playback_timing_snapshot *out);
/* Returns 0 for failure, 1 for queued, 2 for full. */
uint32_t tsfp_video_playback_submit(tsfp_video_playback *playback, uint64_t number,
                                    uint32_t width, uint32_t height, const uint8_t *pixels,
                                    size_t len, uint64_t vt_ns);
void tsfp_video_playback_timeout(tsfp_video_playback *playback);
/* Pixels remain valid until the next successful tick or destruction. */
bool tsfp_video_playback_tick(tsfp_video_playback *playback, bool has_clock, uint64_t media_vt,
                              bool everything, tsfp_video_picture *out);

#endif
