/* SPDX-License-Identifier: GPL-3.0-or-later
 * Registration snapshot, not a count of fully implemented XDK functions.
 * Initialization follows host main: adopt the measured D3D surface first, then
 * device_register; reset DSOUND/input registries. No handler is called.
 */
#include <stdio.h>
#include <string.h>

#include "d3d8_device.h"
#include "d3d8_hle.h"
#include "d3d8_surface_adapter.h"
#include "d3d8_target_query.h"
#include "tsfp_audio_host_lifecycle.h"
#include "xdk_registration_report.h"
#include "xdk_surface.h"
#include "tsfp_input_host_wiring.h"
#include "xinput_hle.h"
#include "xgrph_hle.h"
#include "xgrph_object_lifetime.h"
#include "xgrph_shader_query.h"
#include "xgrph_texture.h"
#include "xgrph_swizzle.h"
#include "xnet_hle.h"
#include "xonline_hle.h"
#include "xonline_offline.h"
#include "xnet_offline.h"
#include "xnet_random.h"

int main(int argc, char **argv)
{
    bool local_entropy = false;
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--xnet-local-entropy") != 0) {
            fprintf(stderr, "xdk_report: unknown policy %s\n", argv[i]);
            return 2;
        }
        local_entropy = true;
    }
    tsfp_audio_host_reset_all();
    tsfp_audio_host_register_report_surface();
    tsfp_input_host_reset_all();
    tsfp_input_host_register_report_surface();
    /* Distinct struct types are copied field by field, without aliasing casts. */
    d3d8_xdk_row rows[XDK_SURFACE_COUNT];
    for (size_t i = 0u; i < XDK_SURFACE_COUNT; i++) {
        rows[i].address = xdk_surface[i].address;
        rows[i].section = xdk_surface[i].section;
        rows[i].name = xdk_surface[i].name;
        rows[i].sites = xdk_surface[i].sites;
    }
    if (!d3d8_surface_adopt(rows, XDK_SURFACE_COUNT, D3D8_SECTION_D3D)) {
        fputs("xdk_report: could not adopt the measured D3D surface\n", stderr);
        return 1;
    }
    (void)d3d8_device_register();
    (void)d3d8_target_query_register();
    if (!xgrph_hle_adopt((const xgrph_xdk_row *)xdk_surface, XDK_SURFACE_COUNT)) {
        fputs("xdk_report: could not adopt the measured XGRPH surface\n", stderr);
        d3d8_hle_shutdown();
        return 1;
    }
    (void)xgrph_texture_register();
    (void)xgrph_object_lifetime_register();
    (void)xgrph_swizzle_register();
    (void)xgrph_shader_query_register();
    if (!xnet_hle_adopt((const xnet_xdk_row *)xdk_surface, XDK_SURFACE_COUNT)) {
        fputs("xdk_report: could not adopt the measured XNET surface\n", stderr);
        d3d8_hle_shutdown();
        xgrph_hle_shutdown();
        return 1;
    }
    (void)xnet_offline_register();
    if (local_entropy) (void)xnet_random_register();
    /* T1071: XONLINE handlers exist only under the opt-in --xonline-offline policy. */
    if (!xonline_hle_adopt((const xonline_xdk_row *)xdk_surface, XDK_SURFACE_COUNT)) {
        fputs("xdk_report: could not adopt the measured XONLINE surface\n", stderr);
        d3d8_hle_shutdown();
        xgrph_hle_shutdown();
        xnet_hle_shutdown();
        return 1;
    }
    xonline_offline_reset();
    (void)xonline_offline_register();
    const bool okay = xdk_registration_report_write(
        stdout, (const xdk_registration_row *)xdk_surface, XDK_SURFACE_COUNT);
    d3d8_hle_shutdown();
    xgrph_hle_shutdown();
    xnet_hle_shutdown();
    xonline_hle_shutdown();
    if (!okay) {
        fputs("xdk_report: surface/registry mismatch or output failure\n", stderr);
        return 1;
    }
    return 0;
}
