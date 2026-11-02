/* Generator for the retained effect command log (T3949). Replays a fixed LCG command stream (frames with random
 * mixbuffer input, immediate/deferred map writes, commits, state reads) on the C effects core and prints one
 * FNV-1a line per event. The Rust twin is `command_log` in tests/retail_image.rs; the two outputs must be equal.
 * Build: cc -I src/audio -I <xemu apu/dsp> log_driver.c crates/tsfp-audio-conformance/oracle/dsound_effects_dsp.c ...cipher.c <xemu dsp objs> */
#include <stdio.h>
#include <stdlib.h>
#include "dsound_effects_dsp.h"
static uint8_t *rd(const char *p, size_t n) { FILE *f = fopen(p, "rb"); uint8_t *b = malloc(n); if (!f || fread(b, 1, n, f) != n) exit(2); fclose(f); return b; }
static uint64_t seed = 0x2545F4914F6CDD1DULL;
static uint32_t rnd(void) { seed = seed * 6364136223846793005ULL + 1442695040888963407ULL; return (uint32_t)(seed >> 32); }
static uint64_t fnv(uint64_t h, uint32_t w) { for (int i = 0; i < 4; i++) { h ^= (w >> (8 * i)) & 0xff; h *= 0x100000001b3ULL; } return h; }
int main(int c, char **v) {
    (void)c;
    uint8_t *fw = rd(v[1], 0x5cc), *im = rd(v[2], 18608);
    dsound_effects_dsp *d = dsound_effects_dsp_create(fw, 0x5cc, im, 18608);
    if (!d) { puts("create failed"); return 1; }
    uint32_t in[1024] = {0}, out[1024];
    for (int i = 0; i < 600; i++) {
        for (int k = 0; k < 1024; k += 32) { int on = (rnd() & 3) != 0; uint32_t r = rnd(); in[k] = !on ? 0 : getenv("FULLSCALE") ? r & 0xffffff : (uint32_t)((int32_t)(r % 16384) - 8192) & 0xffffff; }
        if (i % 7 == 3) {
            unsigned m = rnd() % 9; uint8_t b[8];
            b[0] = (uint8_t)rnd(); b[1] = (uint8_t)rnd(); b[2] = 0x40; b[3] = 0; b[4] = (uint8_t)m; b[5] = 0x11; b[6] = 0x22; b[7] = 0;
            { int df = rnd() & 1; printf("w%d m%u %d\n", i, m, getenv("NOWRITE") ? 1 : dsound_effects_dsp_write(d, m, 0, b, 8, df)); }
        }
        if (i == 5) printf("c%d %d\n", i, dsound_effects_dsp_commit(d));
        int ok = dsound_effects_dsp_frame(d, in, out);
        uint64_t h = 0xcbf29ce484222325ULL;
        for (int k = 0; k < 1024; k++) h = fnv(h, out[k]);
        if (getenv("DUMPF") && atoi(getenv("DUMPF")) == i) for (int k = 0; k < 1024; k++) if (out[k]) fprintf(stderr, "%d=%x\n", k, out[k]);
        printf("f%d %d %016llx\n", i, ok, (unsigned long long)h); if (!ok) { fprintf(stderr, "%s\n", dsound_effects_dsp_error(d)); break; }
    }
    for (unsigned m = 0; m < 9; m++) {
        uint8_t b[16]; int ok = dsound_effects_dsp_read(d, m, 0, b, 16);
        uint64_t h = 0xcbf29ce484222325ULL;
        for (int k = 0; ok && k < 16; k++) h = fnv(h, b[k]);
        printf("r%u %d %016llx\n", m, ok, (unsigned long long)h);
    }
    return 0;
}
