/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_KERNEL_SERVICES_RUST_H
#define TSFP_KERNEL_SERVICES_RUST_H
#include "kernel_hle.h"
#ifdef TSFP_RUST_KSERVICES
uint32_t kernel_services_dispatch(unsigned ordinal, kernel_fn oracle, void *context);
void tsfp_services_reset(uint32_t ordinal);
void tsfp_services_policy(bool fail);
bool tsfp_services_setting(uint32_t index, const void *bytes, size_t count);
bool tsfp_services_eeprom(const void *bytes, const void *key);
bool tsfp_services_av(uint32_t pack, uint32_t region);
void tsfp_services_wide(bool wide);
uint64_t kernel_services_mismatches(unsigned ordinal);
uint64_t kernel_services_calls(unsigned ordinal);
unsigned kernel_services_mode(unsigned ordinal);
#endif
#endif
