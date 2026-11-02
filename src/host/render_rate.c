/* SPDX-License-Identifier: GPL-3.0-or-later
 * T2263: presenter-rate experiment (docs/t2237-frame-measurements.md, T2263 section). Opt-in, default off.
 *
 * The Rust-ports host links with -Wl,--wrap=sub_0003C440, so the outer loop 0x3D370 (another translation unit) calls
 * __wrap_sub_0003C440 here. With TSFP_RENDER_RATE_EXTRA unset or 0 it is the lifted original, unchanged. With N in 1..7
 * the lifted frame runs once (update U0..U4 with input, camera, simulation and the sound tick that carries the audio sink
 * backpressure of T2261, then render R0..R3), and its render half is replayed N more times per sim tick, each preceded by
 * a present (0x232A0, flip 0x23300, the outer loop order) through the render half of the c2r port of 0x3C440
 * (crates/tsfp-ports/src/c2r/t2049_exec_003.rs, render_half_replay; the frame scratch arena is rewound per replay).
 * The last replay is presented by the outer loop as usual. FABRICATED pacing model, measurement only. */
#include "frame_state_sync.h"
#include "rust_ext_bridge.h"

#include <stdio.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <errno.h>
#include <stdatomic.h>

#include "host_runtime.h"
#include "present_sink.h"
#include "kernel_clock.h"
#include "kernel_vtime.h"
#include "t2409_trace.h"
/* T2207: Rust-owned simulation schedule (crates/tsfp-ports/src/wall_clock.rs); render only samples it. */
typedef struct {
    uint64_t epoch_ns, tick, until_ns, total, rebases;
} tsfp_simulation_clock;
typedef struct { uint32_t pending_timing_completion; } tsfp_simulation_frame_boundary;
extern uint64_t tsfp_simulation_clock_begin(tsfp_simulation_clock *state, uint64_t now_ns);
extern uint64_t tsfp_simulation_clock_begin_fast_forward(tsfp_simulation_clock *state, uint64_t initial_epoch_ns);
extern void tsfp_simulation_frame_begin(tsfp_simulation_frame_boundary *state);
extern bool tsfp_simulation_frame_timing_complete(tsfp_simulation_frame_boundary *state);
extern uint32_t tsfp_frame_state_simulation_stamp_snapshot(void);
extern uint64_t tsfp_host_audio_overrun_frames(void) __attribute__((weak));
#include "../game/game_replace.h"

extern void tsfp_render_half_replay(rust_ext_registers *regs, uintptr_t offset, void (*call)(uint32_t, rust_ext_registers *));
extern uint32_t tsfp_render_rate_extra_frames(void);
extern __thread int g_df; /* direction flag, src/game/codewrite_tx.h */
void __real_sub_0003C440(void);
void __wrap_sub_0003C440(void);
void __real_sub_001538C0(void);
void __wrap_sub_001538C0(void);
void __real_sub_0003D370(void);
void __wrap_sub_0003D370(void);
void __real_sub_000A1B80(void);
void __wrap_sub_000A1B80(void);
static void render_replays(uint32_t extra, int trace);
extern unsigned long long rust_lane_render_backlog(void);
extern bool tsfp_hud_messages_lifecycle_enabled(void) __attribute__((weak));
extern bool tsfp_hud_message_trace_snapshot(uint32_t slot, uint32_t *output,
                                            uint32_t output_words) __attribute__((weak));

/* T2292: adaptive EXTRA. TSFP_RENDER_RATE_ADAPT=N (opt-in, default off): a replay frame is skipped while the live rt-host
 * renderer has more than N completed frames still to replay, so a slow adapter (llvmpipe) sheds the optional replay
 * presents instead of filling its queue. Real frames are never shed. */
static uint64_t adapt_shed;
static unsigned long long adapt_limit(void)
{
    static long long limit = -2;
    if (limit == -2) {
        const char *value = getenv("TSFP_RENDER_RATE_ADAPT");
        limit = value != NULL && value[0] != '\0' ? atoll(value) : -1;
        if (limit >= 0) fprintf(stderr, "render rate    T2292 adapt ON: replay frames shed while the renderer is more than %lld frames behind\n", limit);
    }
    return limit < 0 ? ~0ull : (unsigned long long)limit;
}

/* T2269: replay write census (docs/t2237-frame-measurements.md, T2269 section). Opt-in TSFP_RENDER_RATE_CENSUS=1, default off.
 * Snapshots the guest data range after the lifted frame and lists, after each replay, the byte ranges the replay left
 * different (render-half writes that are not idempotent across a replay). Measurement only. */
#define CENSUS_LO 0x004B0000u
#define CENSUS_HI 0x00800000u
static unsigned char *census_snapshot;

static int census_enabled(void)
{
    static int enabled = -1;
    if (enabled < 0) enabled = getenv("TSFP_RENDER_RATE_CENSUS") != NULL;
    return enabled;
}

static void census_take(void)
{
    if (!census_snapshot) census_snapshot = malloc(CENSUS_HI - CENSUS_LO);
    if (!census_snapshot) return;
    memcpy(census_snapshot, (const unsigned char *)(g_xbox_mem_offset + CENSUS_LO), CENSUS_HI - CENSUS_LO);
}

static void census_state(const char *label)
{
    fprintf(stderr, "census %s 7B0C7C=%08X 7B0CC0=%08X 7E71C8=%08X 7B0CE0=%u flag7DE458=%08X\n", label,
            guest_read32(0x007B0C7Cu), guest_read32(0x007B0CC0u), guest_read32(0x007E71C8u), guest_read32(0x007B0CE0u),
            guest_read32(0x007DE458u));
    uint32_t cameras = guest_read32(0x007B0CE0u), base = guest_read32(0x007B0CE4u);
    for (uint32_t i = 0u; i < cameras && i < 4u; i++) {
        uint32_t cam = base + i * 0xFFCu;
        fprintf(stderr, "census %s cam%u %08X flags21C=%08X m204=%08X m208=%08X slot200=%08X\n", label, i, cam,
                guest_read32(cam + 0x21Cu), guest_read32(cam + 0x204u), guest_read32(cam + 0x208u), guest_read32(cam + 0x200u));
    }
}

static void census_diff(uint64_t replay)
{
    if (!census_snapshot) return;
    const unsigned char *now = (const unsigned char *)(g_xbox_mem_offset + CENSUS_LO);
    uint32_t size = CENSUS_HI - CENSUS_LO, ranges = 0u, bytes = 0u, start = 0u, end = 0u;
    int open = 0;
    char line[4096];
    line[0] = 0;
    int used = 0;
    for (uint32_t i = 0u; i <= size; i++) {
        int differs = i < size && now[i] != census_snapshot[i];
        if (differs) {
            bytes++;
            if (open && i - end <= 8u) { end = i + 1u; continue; }
        }
        if (open && (differs || i == size || i - end > 8u)) {
            ranges++;
            if (used < (int)sizeof line - 64)
                used += snprintf(line + used, sizeof line - (size_t)used, " %X+%X", CENSUS_LO + start, end - start);
            open = 0;
        }
        if (differs) { open = 1; start = i; end = i + 1u; }
    }
    fprintf(stderr, "census replay %llu bytes %u ranges %u:%s\n", (unsigned long long)replay, bytes, ranges, line);
}


/* T2273: bone palette rebuild in replays (docs/t2237-frame-measurements.md, T2273 section). 0x8F6B0 builds a skinned object's
 * bone palettes through 0xA1B80 only while the per camera bit obj+0x14 & (1 << cam[0x214]) is clear; 0xA1B80 sets the bit and
 * takes the palettes (instance[8 + k] for each set k with 0xA1AB0(obj, k) != 0) from the frame scratch arena 0x3E970. The replay
 * rewinds that arena, so without a rebuild it draws with palettes whose memory it reallocates, and a set first selected in the
 * replay stays null (fault 0x1530E0+0xAC from 0x18EA90). Every 0xA1B80 call of the frame is logged here (object, camera bit),
 * and before each replay those bits are cleared so the replay rebuilds its palettes like a fresh frame. Only active with
 * TSFP_RENDER_RATE_EXTRA; TSFP_RENDER_RATE_KEEP_PALETTES=1 restores the T2269 behaviour (A/B). Measurement only. */
#define PALETTE_LOG_MAX 4096u
static uint32_t palette_log_obj[PALETTE_LOG_MAX];
static uint16_t palette_log_bit[PALETTE_LOG_MAX];
static uint32_t palette_log_count, palette_log_dropped;
static uint64_t palette_builds, palette_cleared;

void __wrap_sub_000A1B80(void)
{
    const uint32_t obj = game_stack_arg(0);
    const uint32_t cam = guest_read32(0x007B0CC0u);
    const uint16_t bit = (uint16_t)(1u << (guest_read32(cam + 0x214u) & 31u));
    if (palette_log_count < PALETTE_LOG_MAX) {
        palette_log_obj[palette_log_count] = obj;
        palette_log_bit[palette_log_count] = bit;
        palette_log_count++;
    } else palette_log_dropped++;
    palette_builds++;
    __real_sub_000A1B80();
}

static void palette_rebuild_prepare(void)
{
    static int keep = -1;
    if (keep < 0) keep = getenv("TSFP_RENDER_RATE_KEEP_PALETTES") != NULL;
    if (keep) return;
    for (uint32_t i = 0u; i < palette_log_count; i++) {
        uint16_t *field = (uint16_t *)(g_xbox_mem_offset + palette_log_obj[i] + 0x14u);
        if (*field & palette_log_bit[i]) { *field = (uint16_t)(*field & ~palette_log_bit[i]); palette_cleared++; }
    }
    palette_log_count = 0u; /* the replay logs its own rebuilds again */
}

/* T2268: wall-clock step source (docs/t2237-frame-measurements.md, T2268 section). Opt-in TSFP_RENDER_RATE_STEP=wall, default off.
 * The frame wait 0x1538C0 (called by the outer loop after this frame, before the real present) takes the engine step [0x4E794C]
 * as the modelled vblank counter [0x563918] minus the counter it stored at the previous wait ([0x7A58B0], shifted into
 * [0x7A58AC] at its entry), clamped to 1..3. In free run every present advances that counter, so the T2263 replay presents
 * doubled the step. Here, after the frame and its replays, [0x7A58B0] is rewritten to counter - k, k = wall-clock 60 Hz
 * vblanks since the previous tick (fraction carried, clamped 1..3 like the engine), so the step counts real time and the
 * wait passes at once (k >= 1, no fabricated poll blank). FABRICATED pacing model, measurement only. */
static void wall_clock_step(void)
{
    static int mode = -1;
    if (mode < 0) {
        const char *value = getenv("TSFP_RENDER_RATE_STEP");
        mode = value != NULL && value[0] == 'w';
        if (mode) fprintf(stderr, "render rate    T2268 step source WALL: engine step = wall-clock 60 Hz vblanks per tick (TSFP_RENDER_RATE_STEP)\n");
    }
    if (!mode) return;
    static double last_seconds = -1.0, carry;
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    const double seconds = (double)now.tv_sec + (double)now.tv_nsec * 1e-9;
    if (last_seconds < 0.0) {
        last_seconds = seconds;
        return;
    }
    carry += (seconds - last_seconds) * 60;
    last_seconds = seconds;
    uint32_t vblanks = carry > 0.0 ? (uint32_t)carry : 0u;
    if (vblanks < 1u) vblanks = 1u;
    if (vblanks > 3u) {
        vblanks = 3u;
        carry = 0.0; /* engine clamp: a long frame does not pile up catch-up steps */
    } else {
        /* T2287: deduct the forced step too (carry down to -1), else jitter around a 60 Hz tick gave mean step 1.095 */
        carry -= (double)vblanks;
        if (carry < -1.0) carry = -1.0;
    }
    static uint64_t ticks, step_sum;
    ticks++;
    step_sum += vblanks;
    if ((ticks & (ticks - 1u)) == 0u && ticks >= 64u)
        fprintf(stderr, "render rate    wall step ticks %llu mean step %.3f\n", (unsigned long long)ticks, (double)step_sum / (double)ticks);
    guest_write32(0x007A58B0u, guest_read32(0x00563918u) - vblanks);
}

/* T2279: replay-present decoupling (docs/t2237-frame-measurements.md, T2279 section). Opt-in TSFP_RENDER_RATE_DECOUPLE=1 with
 * TSFP_RENDER_RATE_EXTRA, default off. The T2278 run showed each replay present costing a full presenter job slot (the vblank job
 * waits for the frame in flight) and the guest thread pacing on the audio sink backpressure. Here a replay present skips instead
 * of waiting (present_video_sink_replay_slot_busy), the audio ring drops its overflow instead of blocking, and the tick is paced
 * by a 60 Hz wall deadline instead: replays run only while the deadline is ahead, then the thread sleeps to it, so the engine
 * step stays 1 and the spare time becomes extra presents. FABRICATED pacing model, measurement only. */
static int decouple_enabled(void)
{
    static int enabled = -1;
    if (enabled < 0) {
        const char *value = getenv("TSFP_RENDER_RATE_DECOUPLE");
        enabled = value != NULL && value[0] == '1';
        if (enabled) {
            present_audio_sink_set_backpressure(false);
            fprintf(stderr, "render rate    T2279 decouple ON: replay presents skip a busy presenter slot, audio backpressure off, 60 Hz wall deadline\n");
        }
    }
    return enabled;
}

static double monotonic_seconds(void)
{
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    return (double)now.tv_sec + (double)now.tv_nsec * 1e-9;
}

static double decouple_deadline;

/* T2289: replay cost per tick (docs/t2237-frame-measurements.md, T2289 section). Wall seconds of the live tick (begin to the
 * end of the real 0x3C440), of each replay's present calls and render half, and the slack left at the deadline. Logged with
 * the decouple line, reset per log interval. Measurement only. */
static double cost_tick_start, cost_live, cost_present, cost_half, cost_slack, cost_live_max, cost_replay_max;
static uint64_t cost_ticks, cost_replays, cost_no_time;

/* T2287: wrapped ticks per wall second since the previous decouple log line */
static double decouple_rate(uint64_t ticks)
{
    static double last_seconds;
    static uint64_t last_ticks;
    const double now = monotonic_seconds();
    const double rate = last_seconds > 0.0 && now > last_seconds ? (double)(ticks - last_ticks) / (now - last_seconds) : 0.0;
    last_seconds = now;
    last_ticks = ticks;
    return rate;
}

/* T1670: native fixed-tick schedule and Rust-owned callback timer table, both default off.
 * Timer callbacks cross a typed host-to-lifted call seam; table records never enter guest memory. */
static int rust_frame_loop_enabled(void)
{
    static int enabled = -1;
    if (enabled < 0) {
        const char *value = getenv("TSFP_RUST_FRAME_LOOP");
        enabled = value != NULL && value[0] == '1';
        if (enabled) {
            extern void tsfp_rust_enable_frame_timing_port(void);
            typedef struct {
                uint32_t vblank_count, elapsed_frames, frames, multi_vblank_frames;
                uint32_t simulation_stamp, sampled_vblank, raw_vblank_step, outer_frames;
                uint32_t frame_phase;
                uint32_t event_latch, event_callback, pause_word;
                uint32_t timing_history[15];
                uint32_t step_outputs[4];
            } tsfp_frame_state_seed;
            _Static_assert(sizeof(tsfp_frame_state_seed) == 124u,
                           "Rust frame-state seed ABI layout");
            extern bool tsfp_frame_state_select_native(const tsfp_frame_state_seed *seed);
            static const uint32_t timing_history_bases[3] = {0x007A58A0u, 0x007A58C0u, 0x007A58E0u};
            uint32_t timing_history[15];
            tsfp_frame_state_seed seed = {0};
            seed.step_outputs[0] = guest_read32(0x004E7940u);
            seed.step_outputs[1] = guest_read32(0x004E7944u);
            seed.step_outputs[2] = guest_read32(0x004E7948u);
            seed.step_outputs[3] = guest_read32(0x004E794Cu);
            const uint32_t frame_clock_words[4] = {
                guest_read32(0x007497A8u), guest_read32(0x007497ACu),
                guest_read32(0x006B7AF0u), guest_read32(0x006B7AD4u),
            };
            for (uint32_t series = 0; series < 3; ++series) {
                for (uint32_t sample = 0; sample < 5; ++sample) {
                    timing_history[series * 5 + sample] = guest_read32(timing_history_bases[series] + sample * 4u);
                }
            }
            seed.vblank_count = guest_read32(0x00563918u);
            seed.elapsed_frames = guest_read32(0x007497B0u);
            seed.frames = guest_read32(0x007497B8u);
            seed.multi_vblank_frames = guest_read32(0x007497BCu);
            seed.simulation_stamp = guest_read32(0x007DE318u);
            seed.sampled_vblank = frame_clock_words[0];
            seed.raw_vblank_step = frame_clock_words[1];
            seed.outer_frames = frame_clock_words[2];
            seed.frame_phase = frame_clock_words[3];
            seed.event_latch = guest_read32(0x007EB2E0u);
            seed.event_callback = guest_read32(0x007EB2E4u);
            seed.pause_word = guest_read32(0x007497B4u);
            memcpy(seed.timing_history, timing_history, sizeof seed.timing_history);
            tsfp_rust_enable_frame_timing_port();
            if (!tsfp_frame_state_select_native(&seed)) abort();
            fprintf(stderr, "simulation     Rust-owned vblank/frame counters and phase, event latch/callback, pause/step outputs, and timing histories ON (T2409)\n");
        }
        if (enabled) fprintf(stderr, "simulation     T1670 Rust fixed-60 scheduler and Rust-owned 75-slot timers ON (TSFP_RUST_FRAME_LOOP)\n");
    }
    return enabled;
}

/* T1670 isolation switch: keep the Rust frame scheduler on while sending only
 * the timer ABI back through the lifted implementation. Defaults to the frame
 * loop selection; the override exists to bisect frame-loop failures. */
static int rust_game_timers_enabled(void)
{
    static int enabled = -1;
    if (enabled < 0) {
        const char *value = getenv("TSFP_RUST_GAME_TIMERS");
        enabled = rust_frame_loop_enabled() && !(value != NULL && value[0] == '0');
        if (rust_frame_loop_enabled() && !enabled)
            fprintf(stderr, "game timers    lifted timer ABI selected (TSFP_RUST_GAME_TIMERS=0)\n");
    }
    return enabled;
}

static uint32_t game_timer_simulation_stamp(void)
{
    return rust_frame_loop_enabled() ? tsfp_frame_state_simulation_stamp_snapshot()
                                     : guest_read32(0x007DE318u);
}

/* T2487: fast-forward capture/replay mode, defined with the fixed-tick clock below. */
static int fast_forward_enabled(void);

/* T2282: native Rust deadline arithmetic; wall waits stay at the host boundary.
 * Default play opts in via TSFP_SIMULATION_PACE. Diagnostics can retain the oracle. */
static int simulation_paced(void)
{
    static int enabled = -1;
    if (enabled < 0) {
        const char *value = getenv("TSFP_SIMULATION_PACE");
        enabled = (value && value[0] == '1') || rust_frame_loop_enabled();
    }
    return enabled || decouple_enabled() || fast_forward_enabled();
}

/* Enter an ext root through the host root dispatcher so tools.rust_trace's call-args stream records the entry
 * (a direct rust_ext_try left the timers gate with no root calls to window on). */
extern uint32_t host_rust_ports_lifted_root_enter(uint32_t va);
static int timer_root_try(uint32_t va)
{
    TSFP_FRAME_SYNC_IN();
    const int taken = game_rust_try != NULL ? game_rust_try(va) : rust_ext_try(va);
    TSFP_FRAME_SYNC_OUT();
    return taken;
}

/* T2409: the selected outer loop must enter the typed 0x3C440 bridge too;
 * otherwise this wrapper falls back to lifted C and bypasses FrameEventState.
 * The translated body remains a TEMPORARY-SHIM pending its readable Rust port. */
static void call_frame_update_and_draw(void)
{
    if (!rust_frame_loop_enabled()) {
        (void)host_rust_ports_lifted_root_enter(0x0003C440u);
        __real_sub_0003C440();
        return;
    }
    if (!timer_root_try(0x0003C440u)) {
        fprintf(stderr, "simulation     selected Rust frame body was not admitted by its typed bridge\n");
        abort();
    }
}

typedef struct {
    uint32_t mode;
    int32_t elapsed;
    uint32_t stamp, period_bits, argument, callback;
} rust_game_timer_entry;
_Static_assert(sizeof(rust_game_timer_entry) == 0x18u, "Rust game timer entry layout");
extern int32_t tsfp_game_timer_add(uint32_t mode, float seconds, uint32_t argument,
                                   uint32_t callback, uint32_t stamp, uint32_t scale);
extern void tsfp_game_timer_clear(void);
extern void tsfp_game_timer_update(uint32_t stamp, void (*dispatch)(uint32_t, uint32_t));
extern bool tsfp_game_timer_snapshot(uint32_t *count, rust_game_timer_entry *entries);
extern uint32_t tsfp_frame_state_vblank_snapshot(void);
extern uint32_t tsfp_frame_state_elapsed_frames_snapshot(void);
extern uint32_t tsfp_frame_state_frames_snapshot(void);
extern uint32_t tsfp_frame_state_multi_vblank_snapshot(void);
extern uint32_t tsfp_frame_state_pause_snapshot(void);
extern uint32_t tsfp_frame_state_timing_history_snapshot(uint32_t index);
extern uint32_t tsfp_frame_state_step_output_snapshot(uint32_t index);
extern uint32_t tsfp_frame_state_clock_snapshot(uint32_t index);
extern uint32_t tsfp_frame_state_event_snapshot(uint32_t index);
extern bool tsfp_weapon_hands_enabled(void) __attribute__((weak));
extern bool tsfp_weapon_inventory_trace_snapshot(uint32_t player_slot, uint32_t *output,
                                                 uint32_t output_words) __attribute__((weak));
extern bool tsfp_player_interaction_trace_snapshot(uint32_t *output,
                                                   uint32_t output_words) __attribute__((weak));
extern bool tsfp_player_interaction_trace_diagnostics_snapshot(uint32_t *output,
                                                                uint32_t output_words) __attribute__((weak));
extern bool tsfp_weapon_inventory_adapter_diagnostics(uint32_t *output,
                                                      uint32_t output_words) __attribute__((weak));
extern bool tsfp_weapon_transfer_diagnostics_snapshot(uint32_t *output,
                                                       uint32_t output_words) __attribute__((weak));
extern bool kernel_guest_read_bytes(uint32_t address, void *out, size_t length);
void tsfp_lane_x_call(uint32_t va, rust_ext_registers *regs);

static void game_timer_dispatch(uint32_t callback, uint32_t argument)
{
    rust_ext_registers regs;
    rust_ext_store(&regs);
    const uint32_t caller_esp = regs.esp;
    regs.esp -= 4u;
    guest_write32(regs.esp, argument);
    tsfp_lane_x_call(callback, &regs);
    regs.esp = caller_esp;
    rust_ext_load(&regs);
}

static float game_timer_float(uint32_t bits)
{
    float value;
    memcpy(&value, &bits, sizeof value);
    return value;
}

/* T2409: rust_trace reads these reserved host windows through guest_dump's
 * read-only adapter. Read Rust-owned state where the live path uses it; fields
 * still bypassed by lifted code are sampled from guest memory until routed.
 * No guest bytes are changed or mirrored. */
int tsfp_t2409_host_trace_read(uint32_t address, void *out, size_t length)
{
    TSFP_FRAME_SYNC_IN();
    const size_t interaction_trace_bytes = sizeof(uint32_t) * T3398_TRACE_PLAYER_INTERACTION_WORDS;
    if (address >= T3398_TRACE_PLAYER_INTERACTION &&
        (uint64_t)address + length <= (uint64_t)T3398_TRACE_PLAYER_INTERACTION + interaction_trace_bytes) {
        uint32_t words[T3398_TRACE_PLAYER_INTERACTION_WORDS] = {0};
        words[0] = 1; /* schema version */
        if (tsfp_player_interaction_trace_snapshot != NULL) {
            (void)tsfp_player_interaction_trace_snapshot(words, T3398_TRACE_PLAYER_INTERACTION_WORDS);
        }
        memcpy(out, (const unsigned char *)words + (address - T3398_TRACE_PLAYER_INTERACTION), length);
        return T2409_TRACE_READ_OK;
    }
    const size_t interaction_diagnostics_bytes =
        sizeof(uint32_t) * T3398_TRACE_PLAYER_INTERACTION_DIAGNOSTICS_WORDS;
    if (address >= T3398_TRACE_PLAYER_INTERACTION_DIAGNOSTICS &&
        (uint64_t)address + length <=
            (uint64_t)T3398_TRACE_PLAYER_INTERACTION_DIAGNOSTICS + interaction_diagnostics_bytes) {
        uint32_t words[T3398_TRACE_PLAYER_INTERACTION_DIAGNOSTICS_WORDS] = {0};
        words[0] = 1; /* schema version */
        if (tsfp_player_interaction_trace_diagnostics_snapshot != NULL) {
            (void)tsfp_player_interaction_trace_diagnostics_snapshot(
                words, T3398_TRACE_PLAYER_INTERACTION_DIAGNOSTICS_WORDS);
        }
        memcpy(out, (const unsigned char *)words +
                       (address - T3398_TRACE_PLAYER_INTERACTION_DIAGNOSTICS),
               length);
        return T2409_TRACE_READ_OK;
    }
    for (uint32_t slot = 0; slot < 4; ++slot) {
        const uint32_t base = T3397_TRACE_HUD_MESSAGES + slot * T3397_TRACE_HUD_MESSAGES_SLOT_STRIDE;
        const uint64_t end = (uint64_t)address + length;
        if (address < base || address >= (uint64_t)base + T3397_TRACE_HUD_MESSAGES_BYTES) continue;
        if (end > (uint64_t)base + T3397_TRACE_HUD_MESSAGES_BYTES) return T2409_TRACE_READ_FAILED;

        uint8_t projection[T3397_TRACE_HUD_MESSAGES_BYTES] = {0};
        if (tsfp_hud_messages_lifecycle_enabled != NULL && tsfp_hud_messages_lifecycle_enabled()) {
            uint32_t words[T3397_TRACE_HUD_MESSAGES_WORDS];
            if (tsfp_hud_message_trace_snapshot == NULL ||
                !tsfp_hud_message_trace_snapshot(slot, words, T3397_TRACE_HUD_MESSAGES_WORDS)) {
                return T2409_TRACE_READ_FAILED;
            }
            memcpy(projection, words, sizeof projection);
        } else {
            const uint32_t record = 0x007A6160u + slot * 0x177Cu;
            uint32_t cursor;
            if (!kernel_guest_read_bytes(record + 0x30u, &cursor, sizeof cursor)) {
                return T2409_TRACE_READ_FAILED;
            }
            memcpy(projection, &cursor, sizeof cursor);
            for (uint32_t index = 0; index < 16; ++index) {
                uint8_t *entry = projection + 8u + index * 0x16Cu;
                if (!kernel_guest_read_bytes(record + 0x38u + index * 0x16Cu, entry, 0x16Cu)) {
                    return T2409_TRACE_READ_FAILED;
                }
                uint8_t *text = entry + 4u;
                uint8_t *terminator = memchr(text, 0, 320u);
                if (terminator != NULL) {
                    memset(terminator + 1, 0, (size_t)(text + 320u - terminator - 1));
                }
            }
            if (!kernel_guest_read_bytes(record + 0x1710u, projection + 0x16E0u, sizeof(uint32_t))) {
                return T2409_TRACE_READ_FAILED;
            }
        }
        memcpy(out, projection + (address - base), length);
        return T2409_TRACE_READ_OK;
    }
    const size_t weapon_diagnostic_bytes = sizeof(uint32_t) * T2411_TRACE_WEAPON_DIAGNOSTICS_WORDS;
    if (address >= T2411_TRACE_WEAPON_DIAGNOSTICS &&
        (uint64_t)address + length <= (uint64_t)T2411_TRACE_WEAPON_DIAGNOSTICS + weapon_diagnostic_bytes) {
        uint32_t words[T2411_TRACE_WEAPON_DIAGNOSTICS_WORDS] = {0};
        words[0] = 1; /* schema version */
        if (tsfp_weapon_inventory_adapter_diagnostics != NULL) {
            (void)tsfp_weapon_inventory_adapter_diagnostics(words,
                                                             T2411_TRACE_WEAPON_DIAGNOSTICS_WORDS);
        }
        memcpy(out, (const unsigned char *)words + (address - T2411_TRACE_WEAPON_DIAGNOSTICS), length);
        return T2409_TRACE_READ_OK;
    }
    const size_t weapon_transfer_diagnostic_bytes =
        sizeof(uint32_t) * T2411_TRACE_WEAPON_TRANSFER_DIAGNOSTICS_WORDS;
    if (address >= T2411_TRACE_WEAPON_TRANSFER_DIAGNOSTICS &&
        (uint64_t)address + length <=
            (uint64_t)T2411_TRACE_WEAPON_TRANSFER_DIAGNOSTICS + weapon_transfer_diagnostic_bytes) {
        uint32_t words[T2411_TRACE_WEAPON_TRANSFER_DIAGNOSTICS_WORDS] = {0};
        words[0] = 1; /* schema version */
        if (tsfp_weapon_transfer_diagnostics_snapshot != NULL) {
            (void)tsfp_weapon_transfer_diagnostics_snapshot(
                words, T2411_TRACE_WEAPON_TRANSFER_DIAGNOSTICS_WORDS);
        }
        memcpy(out, (const unsigned char *)words +
                       (address - T2411_TRACE_WEAPON_TRANSFER_DIAGNOSTICS),
               length);
        return T2409_TRACE_READ_OK;
    }
    const size_t weapon_trace_bytes = sizeof(uint32_t) * T2411_TRACE_WEAPON_INVENTORY_WORDS;
    if (address >= T2411_TRACE_WEAPON_INVENTORY &&
        (uint64_t)address + length <= (uint64_t)T2411_TRACE_WEAPON_INVENTORY + weapon_trace_bytes) {
        uint32_t words[T2411_TRACE_WEAPON_INVENTORY_WORDS];
        if (tsfp_weapon_hands_enabled != NULL && tsfp_weapon_inventory_trace_snapshot != NULL &&
            tsfp_weapon_hands_enabled() &&
            tsfp_weapon_inventory_trace_snapshot(0, words, T2411_TRACE_WEAPON_INVENTORY_WORDS)) {
            memcpy(out, (const unsigned char *)words + (address - T2411_TRACE_WEAPON_INVENTORY), length);
            return T2409_TRACE_READ_OK;
        }

        /* Off/reference capture projection, matching the typed owner fields in fixed order. */
        memset(words, 0, sizeof words);
        words[0] = 1; /* schema version */
        /* 0x7356d8 stores the extension-array pointer; entries have 0xea0-byte stride. */
        const uint32_t trace_player_slot = 0u;
        uint32_t extension_array_base;
        if (!kernel_guest_read_bytes(0x007356D8u, &extension_array_base,
                                     sizeof extension_array_base) ||
            extension_array_base == 0) {
            return T2409_TRACE_READ_FAILED;
        }
        const uint64_t extension_base_wide =
            (uint64_t)extension_array_base + (uint64_t)trace_player_slot * 0x0EA0u;
        if (extension_base_wide > (uint64_t)UINT32_MAX - (0x0EA0u - 1u)) {
            return T2409_TRACE_READ_FAILED;
        }
        const uint32_t extension_base = (uint32_t)extension_base_wide;
        uint32_t player_slot = UINT32_MAX;
        if (!kernel_guest_read_bytes(extension_base + 8u, &player_slot, sizeof player_slot)) {
            return T2409_TRACE_READ_FAILED;
        }
        if (player_slot != trace_player_slot) {
            memcpy(out, (const unsigned char *)words + (address - T2411_TRACE_WEAPON_INVENTORY), length);
            return T2409_TRACE_READ_OK;
        }
        words[1] = 1; /* player slot 0 is present */
        size_t cursor = 2;
        const uint32_t selection_offsets[2] = {0x94u, 0x98u};
        for (size_t i = 0; i < 2; ++i) {
            uint32_t entry;
            if (!kernel_guest_read_bytes(extension_base + selection_offsets[i], &entry, sizeof entry)) {
                return T2409_TRACE_READ_FAILED;
            }
            words[cursor++] = entry < 70u ? entry : UINT32_MAX;
        }
        uint32_t default_throwable, inventory_flags;
        if (!kernel_guest_read_bytes(extension_base + 0xA0u, &default_throwable, sizeof default_throwable) ||
            !kernel_guest_read_bytes(extension_base + 0x28u, &inventory_flags, sizeof inventory_flags)) {
            return T2409_TRACE_READ_FAILED;
        }
        words[cursor++] = default_throwable < 70u ? default_throwable : UINT32_MAX;
        words[cursor++] = (inventory_flags & 0x8u) != 0;
        for (size_t i = 0; i < 70; ++i) {
            uint8_t value;
            if (!kernel_guest_read_bytes(extension_base + 0x54Cu + (uint32_t)i, &value, sizeof value)) {
                return T2409_TRACE_READ_FAILED;
            }
            words[cursor++] = value != 0;
        }
        for (size_t i = 0; i < 70; ++i) {
            uint8_t value;
            if (!kernel_guest_read_bytes(extension_base + 0x592u + (uint32_t)i, &value, sizeof value)) {
                return T2409_TRACE_READ_FAILED;
            }
            words[cursor++] = value != 0;
        }
        for (size_t i = 0; i < 44; ++i) {
            int16_t value;
            if (!kernel_guest_read_bytes(extension_base + 0x61Eu + (uint32_t)i * 2u, &value, sizeof value)) {
                return T2409_TRACE_READ_FAILED;
            }
            words[cursor++] = (uint32_t)(int32_t)value;
        }
        /* Match Rust HandSide order: Right (ext+0x1d4), then Left (ext+0xa4). */
        const uint32_t hand_offsets[2] = {0x1D4u, 0xA4u};
        for (size_t hand_index = 0; hand_index < 2; ++hand_index) {
            const uint32_t hand = extension_base + hand_offsets[hand_index];
            uint32_t record, child, timer;
            if (!kernel_guest_read_bytes(hand + 0x30u, &record, sizeof record) ||
                !kernel_guest_read_bytes(hand + 0xE8u, &child, sizeof child) ||
                !kernel_guest_read_bytes(hand + 0xF8u, &timer, sizeof timer)) {
                return T2409_TRACE_READ_FAILED;
            }
            words[cursor++] = record == UINT32_MAX ? UINT32_MAX : record & 0xFFFFu;
            for (size_t i = 0; i < 44; ++i) {
                int16_t value;
                if (!kernel_guest_read_bytes(hand + 0x38u + (uint32_t)i * 2u, &value, sizeof value)) {
                    return T2409_TRACE_READ_FAILED;
                }
                words[cursor++] = (uint32_t)(int32_t)value;
            }
            for (size_t i = 0; i < 44; ++i) {
                int16_t value;
                if (!kernel_guest_read_bytes(hand + 0x90u + (uint32_t)i * 2u, &value, sizeof value)) {
                    return T2409_TRACE_READ_FAILED;
                }
                words[cursor++] = (uint32_t)(int32_t)value;
            }
            words[cursor++] = timer;
            for (size_t i = 0; i < 2; ++i) {
                uint32_t transition;
                if (!kernel_guest_read_bytes(hand + 0xF0u + (uint32_t)i * 4u, &transition,
                                             sizeof transition)) {
                    return T2409_TRACE_READ_FAILED;
                }
                words[cursor++] = transition;
            }
            words[cursor++] = child;
        }
        if (!kernel_guest_read_bytes(extension_base + 0x5Cu, &words[cursor], sizeof(uint32_t)) ||
            !kernel_guest_read_bytes(extension_base + 0x60u, &words[cursor + 1], sizeof(uint32_t))) {
            return T2409_TRACE_READ_FAILED;
        }
        cursor += 2;
        if (cursor != T2411_TRACE_WEAPON_INVENTORY_WORDS) return T2409_TRACE_READ_FAILED;
        memcpy(out, (const unsigned char *)words + (address - T2411_TRACE_WEAPON_INVENTORY), length);
        return T2409_TRACE_READ_OK;
    }
    if (address == T2409_TRACE_FRAME_STEP_OUTPUTS && length == sizeof(uint32_t) * 4u) {
        uint32_t outputs[4];
        if (rust_frame_loop_enabled()) {
            for (uint32_t index = 0; index < 4; ++index) {
                outputs[index] = tsfp_frame_state_step_output_snapshot(index);
            }
        } else {
            static const uint32_t output_addresses[4] = {
                0x004E7940u, 0x004E7944u, 0x004E7948u, 0x004E794Cu,
            };
            for (uint32_t index = 0; index < 4; ++index) {
                if (!kernel_guest_read_bytes(output_addresses[index], &outputs[index], sizeof(uint32_t))) {
                    return T2409_TRACE_READ_FAILED;
                }
            }
        }
        memcpy(out, outputs, sizeof outputs);
        return T2409_TRACE_READ_OK;
    }
    if (address == T2409_TRACE_FRAME_CLOCK_WORDS && length == sizeof(uint32_t) * 4u) {
        uint32_t words[4];
        if (rust_frame_loop_enabled()) {
            words[0] = tsfp_frame_state_clock_snapshot(0);
            words[1] = tsfp_frame_state_clock_snapshot(1);
            words[2] = tsfp_frame_state_clock_snapshot(2);
            words[3] = tsfp_frame_state_clock_snapshot(3);
        } else {
            static const uint32_t word_addresses[4] = {
                0x007497A8u, 0x007497ACu, 0x006B7AF0u, 0x006B7AD4u,
            };
            for (uint32_t index = 0; index < 4; ++index) {
                if (!kernel_guest_read_bytes(word_addresses[index], &words[index], sizeof(uint32_t))) {
                    return T2409_TRACE_READ_FAILED;
                }
            }
        }
        memcpy(out, words, sizeof words);
        return T2409_TRACE_READ_OK;
    }
    if (address == T2409_TRACE_FRAME_TIMING_HISTORY && length == sizeof(uint32_t) * 15u) {
        uint32_t history[15];
        if (rust_frame_loop_enabled()) {
            for (uint32_t index = 0; index < 15; ++index) {
                history[index] = tsfp_frame_state_timing_history_snapshot(index);
            }
        } else {
            static const uint32_t history_bases[3] = {0x007A58A0u, 0x007A58C0u, 0x007A58E0u};
            for (uint32_t series = 0; series < 3; ++series) {
                for (uint32_t sample = 0; sample < 5; ++sample) {
                    uint32_t index = series * 5 + sample;
                    if (!kernel_guest_read_bytes(history_bases[series] + sample * 4u, &history[index], sizeof(uint32_t))) {
                        return T2409_TRACE_READ_FAILED;
                    }
                }
            }
        }
        memcpy(out, history, sizeof history);
        return T2409_TRACE_READ_OK;
    }
    if (address == T2409_TRACE_TIMER_COUNT && length == sizeof(uint32_t)) {
        uint32_t count;
        if (rust_game_timers_enabled()) {
            rust_game_timer_entry entries[75];
            if (!tsfp_game_timer_snapshot(&count, entries)) return T2409_TRACE_READ_FAILED;
        } else {
            if (!kernel_guest_read_bytes(0x0074C410u, &count, sizeof count)) return T2409_TRACE_READ_FAILED;
        }
        memcpy(out, &count, sizeof count);
        return T2409_TRACE_READ_OK;
    }
    if (address == T2409_TRACE_TIMER_ROWS && length == sizeof(rust_game_timer_entry) * 75u) {
        rust_game_timer_entry entries[75];
        if (rust_game_timers_enabled()) {
            uint32_t ignored_count;
            if (!tsfp_game_timer_snapshot(&ignored_count, entries)) return T2409_TRACE_READ_FAILED;
        } else {
            if (!kernel_guest_read_bytes(0x007A20E0u, entries, sizeof entries)) return T2409_TRACE_READ_FAILED;
        }
        memcpy(out, entries, sizeof entries);
        return T2409_TRACE_READ_OK;
    }
    if (address == T2409_TRACE_VBLANK_COUNT && length == sizeof(uint32_t)) {
        uint32_t count;
        if (rust_frame_loop_enabled()) {
            count = tsfp_frame_state_vblank_snapshot();
        } else if (!kernel_guest_read_bytes(0x00563918u, &count, sizeof count)) {
            return T2409_TRACE_READ_FAILED;
        }
        memcpy(out, &count, sizeof count);
        return T2409_TRACE_READ_OK;
    }
    if (address == T2409_TRACE_SIMULATION_STAMP && length == sizeof(uint32_t)) {
        uint32_t stamp;
        if (rust_frame_loop_enabled()) {
            stamp = tsfp_frame_state_simulation_stamp_snapshot();
        } else if (!kernel_guest_read_bytes(0x007DE318u, &stamp, sizeof stamp)) {
            return T2409_TRACE_READ_FAILED;
        }
        memcpy(out, &stamp, sizeof stamp);
        return T2409_TRACE_READ_OK;
    }
    if ((address == T2409_TRACE_ELAPSED_FRAMES || address == T2409_TRACE_FRAME_COUNT ||
         address == T2409_TRACE_MULTI_VBLANK_FRAMES ||
         address == T2409_TRACE_EVENT_LATCH || address == T2409_TRACE_EVENT_CALLBACK ||
         address == T2409_TRACE_PAUSE_WORD) &&
        length == sizeof(uint32_t)) {
        uint32_t value;
        /* The selected frame subsystem traces its owners directly, including
         * the event pair that the C2R frame body routes through MappedGuest. */
        if (rust_frame_loop_enabled()) {
            if (address == T2409_TRACE_PAUSE_WORD) value = tsfp_frame_state_pause_snapshot();
            else if (address == T2409_TRACE_ELAPSED_FRAMES) value = tsfp_frame_state_elapsed_frames_snapshot();
            else if (address == T2409_TRACE_FRAME_COUNT) value = tsfp_frame_state_frames_snapshot();
            else if (address == T2409_TRACE_MULTI_VBLANK_FRAMES) value = tsfp_frame_state_multi_vblank_snapshot();
            else value = tsfp_frame_state_event_snapshot(address == T2409_TRACE_EVENT_LATCH ? 0u : 1u);
        } else {
            const uint32_t guest_address = address == T2409_TRACE_ELAPSED_FRAMES       ? 0x007497B0u
                                           : address == T2409_TRACE_FRAME_COUNT          ? 0x007497B8u
                                           : address == T2409_TRACE_MULTI_VBLANK_FRAMES ? 0x007497BCu
                                           : address == T2409_TRACE_PAUSE_WORD           ? 0x007497B4u
                                           : address == T2409_TRACE_EVENT_LATCH          ? 0x007EB2E0u
                                                                                         : 0x007EB2E4u;
            if (!kernel_guest_read_bytes(guest_address, &value, sizeof value)) return T2409_TRACE_READ_FAILED;
        }
        memcpy(out, &value, sizeof value);
        return T2409_TRACE_READ_OK;
    }
    return T2409_TRACE_NOT_HANDLED;
}

typedef struct {
    uint32_t mode, elapsed, stamp, period, argument, callback;
} game_timer_trace_entry;
static game_timer_trace_entry game_timer_trace_previous[75];
static uint32_t game_timer_trace_previous_count;
static int game_timer_trace_initialized;

/* Event-only diagnostic: suppresses the per-frame stamp churn and reports
 * adds, fires, completion/compaction, and count changes for both timer owners. */
static void game_timer_trace_snapshot(const char *phase)
{
    static int enabled = -1;
    if (enabled < 0) {
        const char *value = getenv("TSFP_TIMER_TRACE");
        enabled = value != NULL && value[0] == '1';
    }
    if (!enabled) return;

    rust_game_timer_entry native_entries[75];
    uint32_t count;
    const int native = rust_game_timers_enabled();
    if (native) {
        if (!tsfp_game_timer_snapshot(&count, native_entries)) return;
    } else {
        count = guest_read32(0x0074C410u);
    }
    const uint32_t tick = guest_read32(0x007497B0u);
    const uint32_t stamp = game_timer_simulation_stamp();
    if (!game_timer_trace_initialized || count != game_timer_trace_previous_count)
        fprintf(stderr, "timer event phase=%s frame=%u sim=%u count=%u previous=%u\n",
                phase, tick, stamp, count, game_timer_trace_previous_count);
    for (uint32_t index = 0; index < 75u; index++) {
        const uint32_t address = 0x007A20E0u + index * 0x18u;
        const rust_game_timer_entry *native_entry = &native_entries[index];
        const game_timer_trace_entry current = native ?
            (game_timer_trace_entry){ native_entry->mode, (uint32_t)native_entry->elapsed,
              native_entry->stamp, native_entry->period_bits, native_entry->argument, native_entry->callback } :
            (game_timer_trace_entry){ guest_read32(address + 0x00u), guest_read32(address + 0x04u),
              guest_read32(address + 0x08u), guest_read32(address + 0x0Cu),
              guest_read32(address + 0x10u), guest_read32(address + 0x14u) };
        const game_timer_trace_entry previous = game_timer_trace_previous[index];
        const int added = current.mode != previous.mode || current.argument != previous.argument ||
                          current.callback != previous.callback || current.period != previous.period ||
                          (strcmp(phase, "add") == 0 &&
                           (current.elapsed != previous.elapsed || current.stamp != previous.stamp));
        const int fired = (current.mode == 2u && previous.mode != 2u) ||
                          (current.mode == 1u && (int32_t)current.elapsed < (int32_t)previous.elapsed) ||
                          (current.mode == 0u && (int32_t)current.elapsed <= 0 && (int32_t)previous.elapsed > 0);
        if ((added || fired) && (current.mode != 0u || current.elapsed != 0u || current.callback != 0u))
            fprintf(stderr, "timer row phase=%s frame=%u sim=%u count=%u row=%u mode=%u elapsed=%d period=%08X arg=%08X cb=%08X stamp=%u\n",
                    phase, tick, stamp, count, index, current.mode, (int32_t)current.elapsed,
                    current.period, current.argument, current.callback, current.stamp);
        game_timer_trace_previous[index] = current;
    }
    game_timer_trace_previous_count = count;
    game_timer_trace_initialized = 1;
}

void __real_sub_001654A0(void);
void __real_sub_00165490(void);
void __real_sub_00165540(void);

/* T1670 Tier 0 adapter: reset/add/update select the native owner only when opted in. */
void __wrap_sub_00165490(void)
{
    if (!rust_game_timers_enabled()) {
        __real_sub_00165490();
        game_timer_trace_snapshot("clear");
        return;
    }
    tsfp_game_timer_clear();
    g_esp += 4u; /* the replaced function's RET */
    game_timer_trace_snapshot("clear");
}

void __wrap_sub_001654A0(void)
{
    if (!rust_game_timers_enabled()) {
        __real_sub_001654A0();
        game_timer_trace_snapshot("add");
        return;
    }
    g_eax = (uint32_t)tsfp_game_timer_add(game_stack_arg(0),
        game_timer_float(game_stack_arg(1)), game_stack_arg(2), game_stack_arg(3),
        game_timer_simulation_stamp(), guest_read32(0x004E8CECu));
    g_esp += 4u; /* the replaced function's RET */
    game_timer_trace_snapshot("add");
}

void __wrap_sub_00165540(void)
{
    if (!rust_game_timers_enabled()) {
        __real_sub_00165540();
        game_timer_trace_snapshot("update");
        return;
    }
    tsfp_game_timer_update(game_timer_simulation_stamp(), game_timer_dispatch);
    g_esp += 4u; /* the replaced function's RET */
    game_timer_trace_snapshot("update");
}
/* T2487: fast-forward replays (TSFP_FAST_FORWARD=1). The tick reads and advances the virtual guest time base
 * (kernel_vtime.h) instead of sleeping to a wall deadline, and the audio ring drops its overflow instead of pacing
 * the guest on the device drain. Input is keyed to guest polls (T2032), so the guest result does not depend on it. */
static int fast_forward_enabled(void)
{
    static int cached = -1;
    if (cached < 0) {
        cached = kernel_vtime_enabled();
        if (cached) {
            present_audio_sink_set_backpressure(false);
            fprintf(stderr, "simulation     T2487 fast-forward: tick on the virtual guest time base, no wall-clock sleep, audio backpressure off (TSFP_FAST_FORWARD)\n");
        }
    }
    return cached;
}
static tsfp_simulation_clock simulation_clock;
static tsfp_simulation_frame_boundary simulation_frame_boundary;
/* T2348: Bevy's input recorder reads the fixed tick id without racing this owner thread. */
static _Atomic uint64_t simulation_tick_published;
uint64_t tsfp_host_simulation_tick(void)
{
    return atomic_load_explicit(&simulation_tick_published, memory_order_acquire);
}

static uint64_t simulation_step_sum;
static void decouple_tick_begin(void)
{
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    uint64_t ns = (uint64_t)now.tv_sec * 1000000000ull + (uint64_t)now.tv_nsec;
    /* T2487: under fast-forward "now" is the virtual guest time base, which decouple_tick_end advances to the tick
     * deadline (or an idle jump moved further), so each tick takes one virtual period and no wall time. */
    if (fast_forward_enabled()) ns = kernel_vtime_now_ns();
    if (!simulation_clock.epoch_ns && !fast_forward_enabled()) fprintf(stderr, "simulation     T2282 fixed 60 Hz absolute wall deadline; render uncapped; deterministic tick/replay clock (T2207 Rust schedule)\n");
    /* Re-base after a stall of more than three periods (boot loading, a hitch) is Rust policy now: lost time is not caught up,
     * matching the engine's 1..3 step clamp. Without it the measured RADV boot left the deadline 7.25 s behind. */
    simulation_step_sum += guest_read32(0x004E7944u);
    decouple_deadline = (double)tsfp_simulation_clock_begin(&simulation_clock, ns) * 1e-9;
    atomic_store_explicit(&simulation_tick_published, simulation_clock.total, memory_order_release);
}

static void decouple_tick_end(void)
{
    static uint64_t ticks, slept, interrupted;
    const double remaining = decouple_deadline - monotonic_seconds();
    ticks++;
    cost_ticks++;
    if (remaining > 0.0) cost_slack += remaining;
    if (fast_forward_enabled()) {
        kernel_vtime_advance_to_ns(simulation_clock.until_ns); /* T2487: the tick's period passes in virtual time only */
    } else if (remaining > 0.0) {
        /* T2287: absolute deadline, retried on EINTR. A relative nanosleep that a signal cut short (the SIGPROF sampler and
         * SIGUSR triggers) returned early and let the tick, and [0x6B7AF0], run 92-102/s once the presenter slot wait was gone. */
        struct timespec until;
        until.tv_sec = (time_t)(simulation_clock.until_ns / 1000000000ull);
        until.tv_nsec = (long)(simulation_clock.until_ns % 1000000000ull);
        while (clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &until, NULL) == EINTR) interrupted++;
        slept++;
    }
    if ((ticks & (ticks - 1u)) == 0u && ticks >= 64u) {
        const double wall = monotonic_seconds() - (double)simulation_clock.epoch_ns * 1e-9;
        fprintf(stderr, "simulation     ticks %llu since rebase %llu (%.6f s) mean step %.6f deadline lag ms %.3f rebases %llu slept %llu audio overrun %llu\n",
                (unsigned long long)simulation_clock.total, (unsigned long long)simulation_clock.tick, wall,
                (double)simulation_step_sum / (double)simulation_clock.total,
                (monotonic_seconds() - decouple_deadline) * 1000.0, (unsigned long long)simulation_clock.rebases,
                (unsigned long long)slept, (unsigned long long)(tsfp_host_audio_overrun_frames ? tsfp_host_audio_overrun_frames() : 0u));
        uint64_t skipped, busy;
        present_video_sink_replay_counts(&skipped, &busy);
        fprintf(stderr, "render rate    decouple ticks %llu slept %llu interrupted %llu ticks/s %.2f replay vblanks skipped %llu slot busy %llu\n",
                (unsigned long long)ticks, (unsigned long long)slept, (unsigned long long)interrupted, decouple_rate(ticks), (unsigned long long)skipped,
                (unsigned long long)busy);
        const double n = cost_ticks ? (double)cost_ticks : 1.0, r = cost_replays ? (double)cost_replays : 1.0;
        fprintf(stderr, "render rate    T2289 cost ticks %llu live ms mean %.3f max %.3f replays %llu present ms %.3f half ms %.3f replay max %.3f slack ms %.3f no-time %llu\n",
                (unsigned long long)cost_ticks, cost_live * 1000.0 / n, cost_live_max * 1000.0, (unsigned long long)cost_replays,
                cost_present * 1000.0 / r, cost_half * 1000.0 / r, cost_replay_max * 1000.0, cost_slack * 1000.0 / n,
                (unsigned long long)cost_no_time);
        cost_ticks = cost_replays = cost_no_time = 0u;
        cost_live = cost_present = cost_half = cost_slack = cost_live_max = cost_replay_max = 0.0;
    }
}

void __wrap_sub_0003C440(void)
{
    static int announced;
    const uint32_t extra = tsfp_render_rate_extra_frames();
    if (extra == 0u) {
        if (simulation_paced()) {
            decouple_tick_begin();
            tsfp_simulation_frame_begin(&simulation_frame_boundary);
        }
        call_frame_update_and_draw();
        wall_clock_step();
        return;
    }
    if (!announced) {
        announced = 1;
        fprintf(stderr, "render rate    T2263 experiment ON: %u extra render-only frames per sim tick (TSFP_RENDER_RATE_EXTRA)\n", extra);
    }
    static int trace = -1;
    if (trace < 0) trace = getenv("TSFP_RENDER_RATE_TRACE") != NULL;
    if (trace) fprintf(stderr, "render rate    frame enter\n");
    palette_log_count = 0u;
    const int decouple = simulation_paced();
    if (decouple) {
        decouple_tick_begin();
        tsfp_simulation_frame_begin(&simulation_frame_boundary);
    }
    cost_tick_start = monotonic_seconds();
    call_frame_update_and_draw();
    {
        const double live = monotonic_seconds() - cost_tick_start;
        cost_live += live;
        if (live > cost_live_max) cost_live_max = live;
    }
    if (trace) fprintf(stderr, "render rate    frame done\n");
    render_replays(extra, trace);
    wall_clock_step();
}

/* The measured outer loop calls 0x1538C0 after 0x3C440 and its counter sample.
 * Finish the fixed-tick deadline only after that original vblank/timing update,
 * so yielding to the host scheduler does not split frame work from its clock event. */
void __wrap_sub_001538C0(void)
{
    const int rust_frame_loop = rust_frame_loop_enabled();
    if (!rust_frame_loop || !timer_root_try(0x001538C0u)) {
        (void)host_rust_ports_lifted_root_enter(0x001538C0u);
        __real_sub_001538C0();
    }
    if (tsfp_simulation_frame_timing_complete(&simulation_frame_boundary)) {
        decouple_tick_end();
    }
}

/* T2409: the opt-in Rust outer loop owns 0x6B7AF0 through FrameClockState.
 * The lifted root remains the A/B fallback until the frame/event replay gate passes. */
void __wrap_sub_0003D370(void)
{
    const uint32_t entry_esp = g_esp;
    if (!rust_frame_loop_enabled()) {
        __real_sub_0003D370();
        return;
    }
    if (!timer_root_try(0x0003D370u)) {
        fprintf(stderr, "simulation     selected Rust frame loop was not admitted by its typed bridge\n");
        abort();
    }
    if (g_esp != entry_esp) abort();
    g_esp = entry_esp + 4u; /* the selected root returns through its guest RET slot */
}

static void render_replays(uint32_t extra, int trace)
{
    /* The selected C2R frame body writes this word through FrameEventState;
     * consult that owner instead of the now-stale guest seed. */
    const uint32_t event_callback = rust_frame_loop_enabled()
                                        ? tsfp_frame_state_event_snapshot(1u)
                                        : guest_read32(0x007EB2E4u);
    if (event_callback != 0u) return; /* the frame took its alternative path (no R0..R3 render half) */
    if (guest_read32(0x004C0A2Cu) != 0u) return; /* the outer loop 0x3D370 leaves after this frame (mode change) */
    /* Frames with session flag 0x20000000 set (the load transition) are not replayed: there the replayed render half faults
     * (MEASURED, docs T2263 section: null matrix source in 0x1530E0); TSFP_RENDER_RATE_ALL=1 replays them anyway. */
    static int all = -1;
    if (all < 0) all = getenv("TSFP_RENDER_RATE_ALL") != NULL;
    if (!all && (guest_read32(0x007DE458u) & 0x20000000u) != 0u) return;
    static uint64_t replays;
    const int decouple = decouple_enabled();
    for (uint32_t index = 0u; index < extra; index++) {
        if (fast_forward_enabled()) break; /* fast-forward: no extra display-rate render replays between ticks */
        if (decouple && monotonic_seconds() >= decouple_deadline) { cost_no_time++; break; } /* T2279: no time left before the tick deadline */
        if (rust_lane_render_backlog() > adapt_limit()) { adapt_shed++; break; } /* T2292: renderer behind, shed the replay */
        if (decouple) kernel_clock_render_replay(true);
        rust_ext_registers regs;
        rust_ext_store(&regs);
        regs.eflags = g_df ? 0x400u : 0u;
        /* The presents run below the dead 0x3C440 frame: its saved words and the local at the old [esp+0x10] (written
         * by the update half) are read again by the replayed render half. */
        static int no_present = -1;
        if (no_present < 0) no_present = getenv("TSFP_RENDER_RATE_NO_PRESENT") != NULL; /* diagnostic: replay without the present */
        const double replay_start = monotonic_seconds();
        regs.esp -= 0x100u;
        if (!no_present) {
            if (decouple) present_video_sink_set_replay_present(true);
            rust_ext_guest_call(0x000232A0u, &regs);
            rust_ext_guest_call(0x00023300u, &regs);
            if (decouple) present_video_sink_set_replay_present(false);
        }
        regs.esp += 0x100u;
        const double half_start = monotonic_seconds();
        cost_present += half_start - replay_start;
        if (trace) fprintf(stderr, "render rate    replay enter\n");
        palette_rebuild_prepare();
        if (census_enabled()) { census_state("before"); census_take(); }
        tsfp_render_half_replay(&regs, (uintptr_t)g_xbox_mem_offset, rust_ext_guest_call);
        rust_ext_load(&regs);
        {
            const double end = monotonic_seconds();
            cost_half += end - half_start;
            cost_replays++;
            if (end - replay_start > cost_replay_max) cost_replay_max = end - replay_start;
        }
        if (decouple) kernel_clock_render_replay(false);
        if (census_enabled()) { census_state("after"); census_diff(replays); }
        replays++;
    }
    if ((replays & (replays - 1u)) == 0u)
        fprintf(stderr, "render rate    replays %llu shed %llu palette builds %llu bits cleared %llu log overflow %u\n", (unsigned long long)replays,
                (unsigned long long)adapt_shed,
                (unsigned long long)palette_builds, (unsigned long long)palette_cleared, palette_log_dropped);
}
