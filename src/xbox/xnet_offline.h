/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_XBOX_XNET_OFFLINE_H
#define TSFP_XBOX_XNET_OFFLINE_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* T1070: the XNET library functions whose original body is self contained, so the host answers them
 * exactly (MEASURED against the original bytes under Unicorn, tests/test_xnet_oracle.py). Everything
 * else in the XNET section is a method of the network stack object at [0x7715EC] (built by
 * XNetStartup 0x431BB1 through 0x4319AB, which allocates 0xD50 bytes and reads device config and
 * link state), has no host model, and still stops the run by name. No success is fabricated.
 *
 * 0x00432700 ntohs(x), stdcall ret 4: swaps the two bytes of the low word, result in ax (the upper
 *            half of eax is whatever the caller left, the host returns it zero).
 * 0x0043270F htonl(x), stdcall ret 4: bswap. ntohl and htonl are the same code, one address.
 * 0x00431D53 WSAGetLastError(), a bare jump to GetLastError 0x37E9A7: eax = the thread's last error
 *            word, [[fs:4] + [0x771368]*4] + 4. The host reads that same guest slot through a reader
 *            the host binary installs (the fs base is a host thread local), so a failure recorded by
 *            guest code is what is returned. With no reader the call stops by name. */
#define XNET_NTOHS_ENTRY 0x00432700u
#define XNET_HTONL_ENTRY 0x0043270Fu
#define XNET_WSAGETLASTERROR_ENTRY 0x00431D53u

/* T3413: Xbox Live drop policy (network/live is a drop system, docs/decisions/system-dispositions.md),
 * registered only with the opt-in `--xonline-offline`; see docs/t3413-xnet-offline.md.
 * XNetStartup 0x00431BB1 (1 arg) and WSAStartup 0x00431BC8 (2 args) return 0 as on a console with no
 * cable (xemu reference, T1092), WSAStartup with the MEASURED WSADATA stores, but NO stack object is
 * built: [0x7715EC] stays 0, which is FABRICATED (the original builds one, T1091 is open). The host
 * keeps the two startup reference counts, so XNetCleanup 0x00431CB6 / WSACleanup 0x00431CC3 return 0
 * while one is held and the MEASURED no-object result otherwise. Why success and not an error: the
 * Live caller 0x3B4F50 maps a failure to E_FAIL, 0x3B5530 then returns -3 without publishing
 * [0x7720E4], and 0x3B5670 (reached from 0x3C440) dereferences that NULL (T3413 replay, host fault). */
#define XNET_STARTUP_ENTRY 0x00431BB1u
#define XNET_WSASTARTUP_ENTRY 0x00431BC8u
#define XNET_XNETCLEANUP_ENTRY 0x00431CB6u
#define XNET_WSACLEANUP_ENTRY 0x00431CC3u
#define XNET_SINGLETON_POINTER 0x007715ECu
#define XNET_WSANOTINITIALISED 10093u /* 0x276D */

/* T3413: with no stack object every other XNET wrapper runs its original no-object branch. MEASURED
 * under Unicorn on the pinned retail image with [0x7715EC] == 0 (tests/test_t3413_xnet_no_state.py
 * parses XNET_NOSTATE_TABLE in xnet_offline.c and re-runs each row): the Winsock calls return
 * SOCKET_ERROR / INVALID_SOCKET (-1) after SetLastError(10093), the XNet* calls return 0x276D, address
 * and connect-status queries return 1 and 3, and no row writes its caller's buffers. The handlers
 * refuse by name if the guest singleton is ever nonzero, because then the object's methods run. */
typedef bool (*xnet_last_error_writer)(uint32_t last_error);

typedef bool (*xnet_last_error_reader)(uint32_t *last_error);

typedef struct {
    uint64_t ntohs_calls, htonl_calls, last_error_calls, drop_startup_calls, no_state_calls;
} xnet_offline_stats;

void xnet_offline_reset(void);
xnet_offline_stats xnet_offline_stats_get(void);
uint16_t xnet_swap16(uint16_t value);
uint32_t xnet_swap32(uint32_t value);
void xnet_offline_set_last_error_reader(xnet_last_error_reader reader);
void xnet_offline_set_last_error_writer(xnet_last_error_writer writer);
/* Registers the three handlers in the XNET module, returns how many were taken. */
size_t xnet_offline_register(void);
/* T3413: registers the four startup/cleanup handlers plus the 32 measured no-state wrappers and
 * resets the host reference counts; returns how many were taken (36 on the retail surface). */
size_t xnet_offline_register_drop_policy(void);
#endif
