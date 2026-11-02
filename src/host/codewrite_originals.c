/* SPDX-License-Identifier: GPL-3.0-or-later */
/* T1818: pristine-entry fallback; the retained lift performs its own guest ret. */
#include <stdint.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
extern void tsfp_codewrite_original_003c97ab(void);
extern void tsfp_codewrite_original_0040f720(void);
extern void tsfp_codewrite_original_0038aeb0(void);
extern void tsfp_codewrite_original_0038c010(void);
extern void tsfp_codewrite_original_003b9bc0(void);
extern void tsfp_codewrite_original_003bfa40(void);
extern void tsfp_codewrite_original_003aa1d0(void);
/* Counts committed fast calls and pristine-entry lifted fallback dispatches.
 * Relaxed atomics include guest worker threads without changing guest state. */
static const uint32_t roots[] = {0x003C97ABu, 0x0040F720u, 0x0038AEB0u,
    0x0038C010u, 0x003B9BC0u, 0x003BFA40u, 0x003AA1D0u};
static atomic_ullong paths[7][2];
static atomic_flag registered = ATOMIC_FLAG_INIT;
static atomic_flag printed = ATOMIC_FLAG_INIT;
void game_codewrite_print_paths(void);
void game_codewrite_print_paths(void)
{
    if (atomic_flag_test_and_set_explicit(&printed, memory_order_relaxed)) return;
    for (unsigned i = 0u; i < 7u; i++)
        fprintf(stderr, "CODEWRITE_PATH root=%08X fast=%llu fallback=%llu\n",
            roots[i], atomic_load_explicit(&paths[i][0], memory_order_relaxed),
            atomic_load_explicit(&paths[i][1], memory_order_relaxed));
}
void game_codewrite_path(uint32_t root, int fallback);
void game_codewrite_path(uint32_t root, int fallback)
{
    if (!atomic_flag_test_and_set_explicit(&registered, memory_order_relaxed))
        (void)atexit(game_codewrite_print_paths);
    for (unsigned i = 0u; i < 7u; i++)
        if (root == roots[i]) {
            atomic_fetch_add_explicit(&paths[i][fallback != 0], 1u, memory_order_relaxed);
            return;
        }
}
void game_codewrite_original(uint32_t root);
void game_codewrite_original(uint32_t root)
{
    game_codewrite_path(root, 1);
    switch (root) {
    case 0x003C97ABu: tsfp_codewrite_original_003c97ab(); return;
    case 0x0040F720u: tsfp_codewrite_original_0040f720(); return;
    case 0x0038AEB0u: tsfp_codewrite_original_0038aeb0(); return;
    case 0x0038C010u: tsfp_codewrite_original_0038c010(); return;
    case 0x003B9BC0u: tsfp_codewrite_original_003b9bc0(); return;
    case 0x003BFA40u: tsfp_codewrite_original_003bfa40(); return;
    case 0x003AA1D0u: tsfp_codewrite_original_003aa1d0(); return;
    default: __builtin_trap();
    }
}
