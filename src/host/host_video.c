/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "host_video.h"
#include <stdio.h>
#include "xmv_original.h"
#include "d3d8_resource.h"
#include "tsfp_audio_host_wiring.h"

int host_video_configure(options *opts)
{
    if (opts->native_xmv && !opts->native_xmv_explicit && !xmv_original_ready()) {
        opts->native_xmv = false; /* T1093: the disc default needs the compiled profile, quietly absent otherwise */
    }
    if (opts->native_xmv && !xmv_original_ready()) {
        fprintf(stderr, "--native-xmv requires the compiled retained XMV profile; "
                        "generate it with tools.gen_xmv_original_bodies before building\n");
        return 2;
    }
    if (!xmv_original_configure(opts->native_xmv)) {
        fprintf(stderr, "could not configure original XMV routes\n");
        return 2;
    }
    if (!xmv_original_set_skip_intro(opts->skip_intro)) {
        fprintf(stderr, "--skip-intro requires the configured retained original XMV profile\n");
        return 2;
    }
    xmv_original_set_trace(opts->trace_xmv);
    tsfp_audio_host_set_movie_trace(opts->trace_xmv);
    xmv_original_set_data_resolver(d3d8_resource_virtual_of_registered_data);
    xmv_original_set_frame_dump(opts->dump_xmv_frames, opts->dump_xmv_frames_max);
    if (!xmv_original_set_entry_capture(opts->capture_xmv_entries, opts->capture_xmv_range_first,
                                        opts->capture_xmv_range_last, opts->capture_xmv_range_count)) {
        fprintf(stderr, "could not configure the XMV entry capture\n");
        return 2;
    }
    if (!xmv_original_set_seed(opts->seed_xmv_entry_set, opts->seed_xmv_entry, opts->seed_xmv_result,
                               opts->seed_xmv_patch, opts->seed_xmv_patch_count)) {
        fprintf(stderr, "could not configure the seeded XMV run\n");
        return 2;
    }
    return 0;
}

int host_video_substitute(const options *opts)
{
    if (!xmv_original_set_substitute(opts->xmv_substitute)) {
        fprintf(stderr, "--xmv-substitute needs FROM=TO with names of 1 to 32 letters, digits or underscores\n");
        return 2;
    }
    if (opts->xmv_substitute != NULL) {
        printf("xmv substitute ON (opt-in, T394): CreateDecoderForFile of movie '%s' opens the other disc movie instead. "
               "FABRICATED, the title's path is rewritten in guest memory\n", opts->xmv_substitute);
    }
    if (opts->native_xmv) {
        printf("xmv original CPU profile enabled; original decoder over the disc's own movie data\n");
    }
    return 0;
}

void host_video_note_call(uint32_t callee) { xmv_original_note_call(callee); }
bool host_video_cleanup(void) { return xmv_original_configure(false); }
