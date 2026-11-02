/* SPDX-License-Identifier: GPL-3.0-or-later */
/*
 * T3396: apply the host's narrow interception hook to generated direct calls.
 * This header follows recomp_copy_redirect.h in the lifted compile command.
 * It imports the generated register aliases in their generated-code context,
 * then wraps the ABI macro before each generated source is parsed.
 */
#ifndef TSFP_HOST_RECOMP_DIRECT_CALL_H
#define TSFP_HOST_RECOMP_DIRECT_CALL_H

#ifndef RECOMP_GENERATED_CODE
#define TSFP_DIRECT_CALL_UNDEFINE_GENERATED_CODE
#define RECOMP_GENERATED_CODE
#endif
#include "recomp_types.h"
#ifdef TSFP_DIRECT_CALL_UNDEFINE_GENERATED_CODE
#undef RECOMP_GENERATED_CODE
#undef TSFP_DIRECT_CALL_UNDEFINE_GENERATED_CODE
#endif
#include "recomp_cooperative.h"

#undef RECOMP_ABI_CALL
#ifdef RECOMP_ABI_CHECK
#define RECOMP_ABI_CALL(va, fn) do { \
    RECOMP_CALL_SAFEPOINT(va); \
    uint32_t _ab = g_ebx, _as = g_esi, _ad = g_edi, _ap = g_esp; \
    bool _recomp_direct_handled = recomp_direct_call_intercept((uint32_t)(va), &g_eax); \
    if (!_recomp_direct_handled) (fn)(); \
    else g_esp += 4u; \
    if (g_ebx != _ab || g_esi != _as || g_edi != _ad || g_esp < _ap + 4) \
        recomp_abi_violation_log((va), _ab, _as, _ad, _ap); \
} while(0)
#else
#define RECOMP_ABI_CALL(va, fn) do { \
    RECOMP_CALL_SAFEPOINT(va); \
    if (recomp_direct_call_intercept((uint32_t)(va), &g_eax)) g_esp += 4u; \
    else (fn)(); \
} while(0)
#endif

#endif
