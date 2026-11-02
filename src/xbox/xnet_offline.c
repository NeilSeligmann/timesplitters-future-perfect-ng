/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "xnet_offline.h"
#include "kernel_call.h"
#include "xnet_hle.h"

static xnet_offline_stats state;
static xnet_last_error_reader last_error_reader;
static xnet_last_error_writer last_error_writer;

void xnet_offline_reset(void) { state = (xnet_offline_stats){0}; }
xnet_offline_stats xnet_offline_stats_get(void) { return state; }
void xnet_offline_set_last_error_reader(xnet_last_error_reader reader) { last_error_reader = reader; }
void xnet_offline_set_last_error_writer(xnet_last_error_writer writer) { last_error_writer = writer; }
uint16_t xnet_swap16(uint16_t value) { return (uint16_t)((value << 8) | (value >> 8)); }
uint32_t xnet_swap32(uint32_t value)
{
    return (value << 24) | ((value & 0xFF00u) << 8) | ((value >> 8) & 0xFF00u) | (value >> 24);
}

static uint32_t ntohs_handler(void *context)
{
    uint32_t value = 0u;
    if (!kernel_frame_arg(context, 0u, &value))
        xnet_hle_fatal(XNET_NTOHS_ENTRY, "ntohs argument is unreadable");
    state.ntohs_calls++;
    return xnet_swap16((uint16_t)value);
}

static uint32_t htonl_handler(void *context)
{
    uint32_t value = 0u;
    if (!kernel_frame_arg(context, 0u, &value))
        xnet_hle_fatal(XNET_HTONL_ENTRY, "htonl argument is unreadable");
    state.htonl_calls++;
    return xnet_swap32(value);
}

static uint32_t last_error_handler(void *context)
{
    (void)context;
    uint32_t error = 0u;
    if (last_error_reader == NULL || !last_error_reader(&error))
        xnet_hle_fatal(XNET_WSAGETLASTERROR_ENTRY,
                       "WSAGetLastError needs the thread's TLS last-error slot and none is readable");
    state.last_error_calls++;
    return error;
}

/* T3413 startup references: the host keeps the two WORD counts the original keeps at +D30h/+D32h of
 * the stack object, because no object exists. Both wrap modulo 65536 like the original. */
static uint16_t xnet_references, wsa_references;

static uint32_t drop_startup_handler(void *context)
{
    (void)context; /* the params block is not read: no stack object is built */
    state.drop_startup_calls++;
    xnet_references++;
    return 0u;
}

static bool write_u16(uint32_t address, uint16_t value)
{
    return kernel_guest_write_u8(address, (uint8_t)value) && kernel_guest_write_u8(address + 1u, (uint8_t)(value >> 8));
}

static uint32_t drop_wsa_startup_handler(void *context)
{
    uint32_t version = 0u, output = 0u;
    if (!kernel_frame_arg(context, 0u, &version) || !kernel_frame_arg(context, 1u, &output))
        xnet_hle_fatal(XNET_WSASTARTUP_ENTRY, "WSAStartup arguments are unreadable");
    /* MEASURED WSADATA stores of the original success path (docs/t1091-xnet-lifecycle.md), in order;
     * all other bytes are preserved. The version is not validated (xemu: 0, 0101 and 0202 succeed). */
    if (!write_u16(output, (uint16_t)version) ||
        !write_u16(output + 2u, 0x0202u) || !kernel_guest_write_u32(output + 0x18Cu, 0u) ||
        !write_u16(output + 0x186u, 0u) || !write_u16(output + 0x188u, 0u) ||
        !kernel_guest_write_u8(output + 4u, 0u) || !kernel_guest_write_u8(output + 0x105u, 0u))
        xnet_hle_fatal(XNET_WSASTARTUP_ENTRY, "WSADATA at %#x is not writable", output);
    state.drop_startup_calls++;
    wsa_references++;
    return 0u;
}

/* Cleanup with a reference drops it and returns 0 (xemu reference, T1092); with none it is the
 * MEASURED no-object branch: XNetCleanup returns 0x276D, WSACleanup sets 10093 and returns -1. */
static uint32_t drop_xnet_cleanup_handler(void *context)
{
    (void)context;
    state.no_state_calls++;
    if (xnet_references == 0u) return XNET_WSANOTINITIALISED;
    xnet_references--;
    return 0u;
}

static bool record_not_initialised(uint32_t address, const char *name);

static uint32_t drop_wsa_cleanup_handler(void *context)
{
    (void)context;
    state.no_state_calls++;
    if (wsa_references == 0u) {
        (void)record_not_initialised(XNET_WSACLEANUP_ENTRY, "WSACleanup");
        return 0xFFFFFFFFu;
    }
    wsa_references--;
    return 0u;
}

/* Columns: address, name, kind. NS_WSA: SetLastError(10093) then -1. NS_VALUE(v): returns v, no
 * last error. Argument counts and stdcall pops come from the generated xdk_abi.inc rows (the
 * measurement also checks the pop). Startup and cleanup are the handlers above; the event calls
 * still stop by name, they need kernel event handles. */
#define NS_WSA 0xFFFFFFFFu, true
#define NS_VALUE(v) (v), false
#define XNET_NOSTATE_TABLE(X) \
    X(0x00431700u, XNetRandom, NS_VALUE(XNET_WSANOTINITIALISED)) \
    X(0x0043170Bu, XNetCreateKey, NS_VALUE(XNET_WSANOTINITIALISED)) \
    X(0x00431716u, XNetRegisterKey, NS_VALUE(XNET_WSANOTINITIALISED)) \
    X(0x00431721u, XNetUnregisterKey, NS_VALUE(XNET_WSANOTINITIALISED)) \
    X(0x0043172Cu, XNetXnAddrToInAddr, NS_VALUE(XNET_WSANOTINITIALISED)) \
    X(0x00431743u, Sub00431743, NS_VALUE(XNET_WSANOTINITIALISED)) \
    X(0x0043174Eu, XNetDnsLookup, NS_VALUE(XNET_WSANOTINITIALISED)) \
    X(0x00431765u, XNetDnsRelease, NS_VALUE(XNET_WSANOTINITIALISED)) \
    X(0x00431770u, XNetQosListen, NS_VALUE(XNET_WSANOTINITIALISED)) \
    X(0x0043177Fu, XNetQosLookup, NS_VALUE(XNET_WSANOTINITIALISED)) \
    X(0x0043179Bu, XNetQosRelease, NS_VALUE(XNET_WSANOTINITIALISED)) \
    X(0x004317A6u, XNetGetTitleXnAddr, NS_VALUE(1u)) \
    X(0x004317B1u, socket, NS_WSA) \
    X(0x004317BCu, closesocket, NS_WSA) \
    X(0x004317C7u, shutdown, NS_WSA) \
    X(0x004317D2u, ioctlsocket, NS_WSA) \
    X(0x004317DDu, setsockopt, NS_WSA) \
    X(0x004317ECu, getsockname, NS_WSA) \
    X(0x004317F7u, getpeername, NS_WSA) \
    X(0x00431802u, bind, NS_WSA) \
    X(0x0043180Du, connect, NS_WSA) \
    X(0x00431818u, accept, NS_WSA) \
    X(0x00431823u, select, NS_WSA) \
    X(0x0043184Cu, recv, NS_WSA) \
    X(0x00431866u, recvfrom, NS_WSA) \
    X(0x00431875u, send, NS_WSA) \
    X(0x0043188Fu, sendto, NS_WSA) \
    X(0x00431968u, XNetInAddrToXnAddr, NS_VALUE(XNET_WSANOTINITIALISED)) \
    X(0x0043197Fu, XNetUnregisterInAddr, NS_VALUE(XNET_WSANOTINITIALISED)) \
    X(0x0043198Au, XNetConnect, NS_VALUE(XNET_WSANOTINITIALISED)) \
    X(0x00431995u, XNetGetConnectStatus, NS_VALUE(3u)) \
    X(0x004319A0u, Sub004319A0, NS_VALUE(XNET_WSANOTINITIALISED))

static bool record_not_initialised(uint32_t address, const char *name)
{
    if (last_error_writer == NULL || !last_error_writer(XNET_WSANOTINITIALISED))
        xnet_hle_fatal(address, "%s needs the thread's TLS last-error slot to record 10093 and none is writable",
                       name);
    return true;
}

static uint32_t no_state_call(uint32_t address, const char *name, uint32_t result, bool sets_error)
{
    uint32_t singleton = 0u;
    if (!kernel_guest_read_u32(XNET_SINGLETON_POINTER, &singleton))
        xnet_hle_fatal(address, "%s: the XNET singleton word at 0x7715EC is unreadable", name);
    if (singleton != 0u)
        xnet_hle_fatal(address, "%s with a live XNET stack object %#x: its methods are not modelled (T1091)",
                       name, singleton);
    if (sets_error) (void)record_not_initialised(address, name);
    state.no_state_calls++;
    return result;
}

#define X(address, name, kind) \
    static uint32_t no_state_##name(void *context) \
    { \
        (void)context; \
        return no_state_call(address, #name, kind); \
    }
XNET_NOSTATE_TABLE(X)
#undef X

size_t xnet_offline_register_drop_policy(void)
{
    xnet_references = wsa_references = 0u;
    size_t taken = (size_t)xnet_hle_register(XNET_STARTUP_ENTRY, drop_startup_handler) +
                   (size_t)xnet_hle_register(XNET_WSASTARTUP_ENTRY, drop_wsa_startup_handler) +
                   (size_t)xnet_hle_register(XNET_XNETCLEANUP_ENTRY, drop_xnet_cleanup_handler) +
                   (size_t)xnet_hle_register(XNET_WSACLEANUP_ENTRY, drop_wsa_cleanup_handler);
#define X(address, name, kind) taken += (size_t)xnet_hle_register(address, no_state_##name);
    XNET_NOSTATE_TABLE(X)
#undef X
    return taken;
}

size_t xnet_offline_register(void)
{
    return (size_t)xnet_hle_register(XNET_NTOHS_ENTRY, ntohs_handler) +
           (size_t)xnet_hle_register(XNET_HTONL_ENTRY, htonl_handler) +
           (size_t)xnet_hle_register(XNET_WSAGETLASTERROR_ENTRY, last_error_handler);
}
