/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * T1508 second admission pass: the code-write roots with pointer-chasing bodies (003C97AB,
 * 0040F720, 003C2260, 0038AEB0, 0038AF20, 0038C010, 003B9BC0). The registrations live in
 * codewrite_tx_roots.inc so tools/replace can hash and mutate them as one unit; they use the
 * transaction ABI of codewrite_tx.h and the exact interpreter of codewrite_x86.h.
 */
#include "codewrite_tx.h"

#include "codewrite_tx_roots.inc"
