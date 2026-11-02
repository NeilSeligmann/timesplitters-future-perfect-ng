/* SPDX-License-Identifier: GPL-3.0-or-later
 * Per-root promotion policy. Callers must isolate and compare guest state;
 * this type only coordinates the verdicts of concurrent comparisons. */
#ifndef TSFP_SHADOW_PORT_POLICY_H
#define TSFP_SHADOW_PORT_POLICY_H

#include <stdatomic.h>
#include <stdbool.h>
#include <stdint.h>

typedef enum { SHADOW_COMPARE, SHADOW_PRIMARY, SHADOW_LIFTED } shadow_route;
typedef struct {
    atomic_flag lock;
    uint64_t clean, diff, in_flight;
    uint64_t promote_at;
    bool disabled, promoted;
} shadow_port_policy;

#define SHADOW_PORT_POLICY_INIT(threshold) { ATOMIC_FLAG_INIT, 0, 0, 0, (threshold), false, false }

static inline void shadow_policy_lock(shadow_port_policy *p)
{
    while (atomic_flag_test_and_set_explicit(&p->lock, memory_order_acquire)) { }
}
static inline void shadow_policy_unlock(shadow_port_policy *p)
{
    atomic_flag_clear_explicit(&p->lock, memory_order_release);
}

static inline shadow_route shadow_policy_begin(shadow_port_policy *p)
{
    shadow_policy_lock(p);
    shadow_route route = p->disabled ? SHADOW_LIFTED : p->promoted ? SHADOW_PRIMARY : SHADOW_COMPARE;
    if (route == SHADOW_COMPARE) ++p->in_flight;
    shadow_policy_unlock(p);
    return route;
}

/* A fault is a difference. A pending comparison must finish before promotion,
 * otherwise a later failing comparison could leave an unchecked primary call. */
static inline void shadow_policy_finish(shadow_port_policy *p, bool equal)
{
    shadow_policy_lock(p);
    if (p->in_flight != 0) {
        --p->in_flight;
        if (equal) ++p->clean;
        else { ++p->diff; p->disabled = true; }
        if (!p->disabled && p->in_flight == 0 && p->clean >= p->promote_at)
            p->promoted = true;
    }
    shadow_policy_unlock(p);
}

/* A lifted fault is not a verdict on the port. */
static inline void shadow_policy_cancel(shadow_port_policy *p)
{
    shadow_policy_lock(p);
    if (p->in_flight != 0) {
        --p->in_flight;
        if (!p->disabled && p->in_flight == 0 && p->clean >= p->promote_at)
            p->promoted = true;
    }
    shadow_policy_unlock(p);
}

/* Used for a fault after promotion or a denylist entry at startup. */
static inline void shadow_policy_disable(shadow_port_policy *p)
{
    shadow_policy_lock(p);
    p->disabled = true;
    shadow_policy_unlock(p);
}

static inline void shadow_policy_primary_fault(shadow_port_policy *p)
{
    shadow_policy_lock(p);
    ++p->diff;
    p->disabled = true;
    shadow_policy_unlock(p);
}
#endif
