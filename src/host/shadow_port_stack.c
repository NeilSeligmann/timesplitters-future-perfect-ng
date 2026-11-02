/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include "shadow_port_stack.h"
#include <stddef.h>
#include <pthread.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

_Static_assert(sizeof(size_t) >= 8, "shadow window requires a 64-bit host");
#define SHADOW_WINDOW_SIZE ((size_t)1u << 32)
static _Thread_local unsigned char *window;
static _Thread_local size_t active_page;
static _Thread_local size_t page_size;
static _Thread_local bool active;
static pthread_key_t window_key;
static pthread_once_t key_once = PTHREAD_ONCE_INIT;
static int key_error;
static void free_window(void *value) { if (value != NULL) (void)munmap(value, SHADOW_WINDOW_SIZE); }
static void make_key(void) { key_error = pthread_key_create(&window_key, free_window); }

bool shadow_stack_begin(uint32_t address, uint32_t argument, uintptr_t *offset)
{
    if (offset == NULL || active || address > UINT32_MAX - 3u) return false;
    if (window == NULL) {
        if (pthread_once(&key_once, make_key) != 0 || key_error != 0) return false;
        long size = sysconf(_SC_PAGESIZE);
        if (size <= 0 || ((size_t)size & ((size_t)size - 1u)) != 0) return false;
        page_size = (size_t)size;
        void *mapped = mmap(NULL, SHADOW_WINDOW_SIZE, PROT_NONE,
                            MAP_PRIVATE | MAP_ANONYMOUS | MAP_NORESERVE, -1, 0);
        if (mapped == MAP_FAILED) return false;
        if (pthread_setspecific(window_key, mapped) != 0) {
            (void)munmap(mapped, SHADOW_WINDOW_SIZE);
            return false;
        }
        window = mapped;
    }
    size_t page = (size_t)address & ~(page_size - 1u);
    if ((size_t)address - page > page_size - sizeof argument) return false;
    if (mprotect(window + page, page_size, PROT_READ | PROT_WRITE) != 0) return false;
    memset(window + page, 0, page_size);
    memcpy(window + address, &argument, sizeof argument);
    if (mprotect(window + page, page_size, PROT_READ) != 0) {
        shadow_stack_destroy();
        return false;
    }
    active_page = page;
    active = true;
    *offset = (uintptr_t)window;
    return true;
}

void shadow_stack_end(void)
{
    if (active) {
        if (mprotect(window + active_page, page_size, PROT_NONE) != 0) {
            shadow_stack_destroy();
            return;
        }
        (void)madvise(window + active_page, page_size, MADV_DONTNEED);
        active = false;
    }
}

void shadow_stack_destroy(void)
{
    if (window != NULL) (void)munmap(window, SHADOW_WINDOW_SIZE);
    if (window != NULL) (void)pthread_setspecific(window_key, NULL);
    window = NULL;
    active = false;
}
