/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * T1508 code-write roots (003BFA40, 003AA1D0): constructors whose original rewrites its own
 * code when handed a hostile object pointer. The registrations live in codewrite_roots.inc so
 * tools/replace can hash and mutate them as one unit; they use the code-write replacement ABI
 * of codewrite_replace.h.
 */
#include "codewrite_replace.h"

#include "codewrite_roots.inc"
