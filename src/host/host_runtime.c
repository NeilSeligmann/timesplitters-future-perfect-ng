/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Stopping a lifted-code run. See host_runtime.h for why this exists, and for why
 * the jump buffer is per-thread while the signal handlers are not.
 */

#define _GNU_SOURCE /* ucontext_t register names and process_vm_readv for T505 */
#include "host_core_policy.h"
#include "host_rss_watchdog.h"
#include "host_runtime.h"

#include <errno.h>
#include <pthread.h>
#include <signal.h>
#include <stdatomic.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/uio.h>
#include <ucontext.h>
#if defined(__linux__) && defined(__x86_64__)
#include <execinfo.h>
#endif
#include "kernel_call.h"
#include <unistd.h>

/* Thread-local, and that is load-bearing. A fault is delivered to the thread that
 * caused it, so a shared buffer would mean one thread jumping onto another
 * thread's stack. `__thread` in the main executable is initial-exec TLS, which is
 * a fixed offset from the thread pointer and needs no lazy allocation -- which is
 * what makes reading it from a signal handler sound. */
#if defined(__GNUC__) || defined(__clang__)
#define HOST_TLS __thread
#else
#define HOST_TLS _Thread_local
#endif

static HOST_TLS sigjmp_buf t_jmp;
static HOST_TLS host_stop t_stop;
static HOST_TLS volatile sig_atomic_t t_armed;
static HOST_TLS volatile host_run_scope *t_scopes[HOST_RUN_SCOPE_MAX];
/* T827: the depth is the single publication word. Push and pop change it with one aligned store, so a handler on
 * this thread sees either the old or the new stack, never a half built one, and no signal mask is needed (the
 * pthread_sigmask pair was 28 percent of the guest thread CPU during the movies). */
static HOST_TLS volatile unsigned t_depth;
static HOST_TLS uint64_t t_owner_token;
static _Atomic uint64_t owner_sequence=1u;
_Static_assert(ATOMIC_POINTER_LOCK_FREE==2,"signal target requires lock-free pointers");
static _Atomic(host_fault_guest_ebp_fn) guest_ebp_reader;


/* The signals a wrong guest pointer or a bad decode can produce. SIGBUS is
 * included because a misaligned access on a mapped page is not SIGSEGV, and a
 * run that died of one would otherwise look like a clean exit. */
static const int FAULT_SIGNALS[] = {SIGSEGV, SIGBUS, SIGFPE, SIGILL};
#define FAULT_SIGNAL_COUNT (sizeof(FAULT_SIGNALS) / sizeof(FAULT_SIGNALS[0]))

/* Dispositions are process-wide, so the install is too: reference-counted, with
 * the saved originals captured exactly once. Without the count, the first thread
 * to finish would restore `fault_handler` itself as the "original" and leave it
 * installed forever, so a fault during teardown would jump into a dead frame. */
static pthread_mutex_t install_lock = PTHREAD_MUTEX_INITIALIZER;
static unsigned install_count;
static struct sigaction previous[FAULT_SIGNAL_COUNT];

/* Emit a single async-signal-safe line for threads without a guest jump target. */
static void host_unarmed_fault(int sig, siginfo_t *info, void *context)
{
    char line[160];
    size_t n = 0;
    const char prefix[] = "HOST STOP: host-thread fault signal=0x";
    for (size_t i = 0; i < sizeof prefix - 1; i++) line[n++] = prefix[i];
    uintptr_t values[3] = {(uintptr_t)(unsigned)sig, info ? (uintptr_t)info->si_addr : 0u, 0u};
#if defined(__linux__) && defined(__x86_64__)
    if (context) values[2] = (uintptr_t)((ucontext_t *)context)->uc_mcontext.gregs[REG_RIP];
#else
    (void)context;
#endif
    const char *labels[] = {" fault_address=0x", " instruction=0x", "\n"};
    for (size_t v = 0; v < 3; v++) {
        for (int shift = (int)(sizeof(uintptr_t) * 8u) - 4; shift >= 0; shift -= 4)
            line[n++] = "0123456789abcdef"[(values[v] >> shift) & 15u];
        for (const char *p = labels[v]; *p; p++) line[n++] = *p;
    }
    (void)write(STDERR_FILENO, line, n);
    (void)signal(sig, SIG_DFL);
    (void)raise(sig);
}

static uint64_t owner_token(void)
{
    if(t_owner_token==0u) {
        uint64_t next=atomic_load_explicit(&owner_sequence,memory_order_relaxed);
        for(;;) {
            /* Saturate before wrap: an old thread incarnation is never reused. */
            if(next==UINT64_MAX)abort();
            if(atomic_compare_exchange_weak_explicit(&owner_sequence,&next,next+1u,
                    memory_order_relaxed,memory_order_relaxed)) {
                t_owner_token=next;break;
            }
        }
    }
    return t_owner_token;
}
bool host_run_scope_init(volatile host_run_scope *scope)
{
    if(scope==NULL || scope->active ||
       (scope->owner_token!=0u && scope->owner_token!=owner_token()))return false;
    scope->owner_token=owner_token();scope->initialized=true;return true;
}
sigjmp_buf *host_run_scope_jmp(volatile host_run_scope *scope)
{
    if(scope==NULL || scope->active || !scope->initialized || scope->owner_token!=owner_token())return NULL;
    /* sigsetjmp owns this buffer; volatile applies to the caller's scope lifetime. */
    return (sigjmp_buf *)(void *)&scope->jump;
}
bool host_run_scope_push(volatile host_run_scope *scope)
{
    if(!t_armed || scope==NULL || scope->owner_token!=owner_token() ||
       !scope->initialized || scope->active || t_depth==HOST_RUN_SCOPE_MAX)return false;
    const unsigned depth=t_depth;
    scope->active=true;t_scopes[depth]=scope;
    atomic_signal_fence(memory_order_seq_cst);
    t_depth=depth+1u;return true;
}
bool host_run_scope_pop(volatile host_run_scope *scope)
{
    if(!t_armed || scope==NULL || scope->owner_token!=owner_token() ||
       !scope->active || t_depth==0u || t_scopes[t_depth-1u]!=scope)return false;
    const unsigned depth=t_depth-1u;
    t_depth=depth;
    atomic_signal_fence(memory_order_seq_cst);
    t_scopes[depth]=NULL;scope->active=false;scope->initialized=false;return true;
}
unsigned host_run_scope_depth(void){return t_depth;}
static _Noreturn void jump_active(void)
{
    const unsigned depth=t_depth;
    volatile host_run_scope *scope=depth!=0u?t_scopes[depth-1u]:NULL;
    if(scope!=NULL)siglongjmp(*(sigjmp_buf *)(void *)&scope->jump,1);
    siglongjmp(t_jmp,1);
}
_Noreturn void host_run_rethrow(const host_stop *record)
{
    if(!t_armed || record==NULL)abort();
    t_stop=*record;jump_active();
}

const char *host_stop_reason_str(host_stop_reason reason)
{
    switch (reason) {
    case HOST_STOP_RETURNED:
        return "guest entry point returned";
    case HOST_STOP_ICALL_UNRESOLVED:
        return "indirect call to an unresolved target";
    case HOST_STOP_ICALL_NOT_CODE:
        return "indirect call to a non-code address";
    case HOST_STOP_UNIMPLEMENTED:
        return "unimplemented instruction";
    case HOST_STOP_KERNEL_UNIMPLEMENTED:
        return "kernel ordinal with no implementation";
    case HOST_STOP_KERNEL_ABI_UNKNOWN:
        return "kernel ordinal with no known argument count";
    case HOST_STOP_XDK_UNIMPLEMENTED:
        return "XDK function with no implementation";
    case HOST_STOP_XDK_ABI_UNKNOWN:
        return "XDK function with no established calling convention";
    case HOST_STOP_XDK_UNROUTED:
        return "XDK function in a section with no HLE module";
    case HOST_STOP_XDK_NOT_MEASURED:
        return "XDK dispatch to an address outside the measured surface";
    case HOST_STOP_FAULT:
        return "host fault";
    case HOST_STOP_HOST_SHUTDOWN:
        return "host shutdown requested";
    case HOST_STOP_BUDGET:
        return "step budget exhausted";
    case HOST_STOP_THREAD_EXITED:
        return "guest thread exited via PsTerminateSystemThread";
    case HOST_STOP_THREAD_TIMEOUT:
        return "guest thread still running when the watchdog expired";
    case HOST_STOP_THREAD_NO_CODE:
        return "guest thread entry address has no runnable code";
    case HOST_STOP_FIRMWARE_RETURN:
        return "the title asked to reboot (HalReturnToFirmware)";
    case HOST_STOP_KERNEL_FATAL:
        return "fatal kernel call (bugcheck or undispatched exception raise)";
    case HOST_STOP_BRIDGE_CONTRACT:
        return "Rust port bridge contract broken (missing callee or bad ESP after a nested guest call)";
    }
    return "unknown";
}

sigjmp_buf *host_run_jmp(void)
{
    return &t_jmp;
}

bool host_run_armed(void)
{
    return t_armed;
}

const host_stop *host_run_result(void)
{
    return &t_stop;
}

void host_run_set_fault_guest_ebp(host_fault_guest_ebp_fn reader)
{
    atomic_store_explicit(&guest_ebp_reader, reader, memory_order_release);
}

/* T505: read memory that may be unmapped without faulting. A wild `rbp` or guest `ebp` is
 * exactly what a fault tends to leave behind, and a second fault inside the handler would
 * lose the report. `process_vm_readv` on our own pid reports EFAULT instead, and like
 * `getpid` it is a plain system call, so it is as safe here as `signal()` is. */
static bool safe_read(uintptr_t address, void *out, size_t size)
{
    struct iovec local = {.iov_base = out, .iov_len = size};
    struct iovec remote = {.iov_base = (void *)address, .iov_len = size};
    return process_vm_readv(kernel_host_pid(), &local, 1, &remote, 1, 0) == (ssize_t)size;
}

static void clear_fault_evidence(host_stop *stop)
{
    stop->fault_rip = 0u;
    stop->fault_stack_top = 0u;
    stop->fault_guest_ebp = 0u;
    stop->fault_host_frame_count = 0u;
    stop->fault_guest_frame_count = 0u;
}

/* Guest frames: `[ebp]` is the caller's saved ebp and `[ebp + 4]` the return address, and a
 * chain only ever climbs. Guest memory is mapped at its own VA, so the VA is the host pointer. */
static void record_guest_frames(host_stop *stop)
{
    host_fault_guest_ebp_fn reader = atomic_load_explicit(&guest_ebp_reader, memory_order_acquire);
    if (reader == NULL) {
        return;
    }
    uint32_t ebp = reader();
    stop->fault_guest_ebp = ebp;
    while (stop->fault_guest_frame_count < HOST_FAULT_GUEST_FRAMES && ebp >= 0x1000u &&
           ebp <= 0xFFFFFFF0u) {
        uint32_t pair[2];
        if (!safe_read((uintptr_t)ebp, pair, sizeof(pair)) || pair[1] == 0u) {
            break;
        }
        stop->fault_guest_frames[stop->fault_guest_frame_count++] = pair[1];
        if (pair[0] <= ebp) {
            break;
        }
        ebp = pair[0];
    }
}

#if defined(__linux__) && defined(__x86_64__)
static _Thread_local sigjmp_buf t_unwind_jmp;
static _Thread_local volatile bool t_unwinding;

/* Host frames: the lifted code is built at -O0, so every lifted call has an `rbp` frame and
 * the chain is one entry per guest call. Frames of a frameless libc leaf are skipped by the
 * chain, which is what `fault_stack_top` is for. */
static void record_host_frames(host_stop *stop, const ucontext_t *context)
{
    uintptr_t rbp = (uintptr_t)context->uc_mcontext.gregs[REG_RBP];
    stop->fault_rip = (uintptr_t)context->uc_mcontext.gregs[REG_RIP];
    uintptr_t top;
    if (safe_read((uintptr_t)context->uc_mcontext.gregs[REG_RSP], &top, sizeof(top))) {
        stop->fault_stack_top = top;
    }
    while (stop->fault_host_frame_count < HOST_FAULT_HOST_FRAMES && rbp >= 0x1000u &&
           (rbp & (sizeof(uintptr_t) - 1u)) == 0u) {
        uintptr_t pair[2];
        if (!safe_read(rbp, pair, sizeof(pair)) || pair[1] == 0u) {
            break;
        }
        stop->fault_host_frames[stop->fault_host_frame_count++] = pair[1];
        if (pair[0] <= rbp) {
            break;
        }
        rbp = pair[0];
    }
    /* Rust ports are built without frame pointers, so a fault inside one (MappedGuest::read32,
     * say) leaves the rbp chain empty and the report named only the leaf. Fall back to the
     * DWARF unwinder through the signal frame; libgcc is preloaded in host_run_arm so this
     * does not allocate here. Frames above the faulting RIP (this handler) are dropped. */
    if (stop->fault_host_frame_count == 0u) {
        void *frames[HOST_FAULT_HOST_FRAMES + 8u];
        int count = 0;
        /* A corrupt stack can fault the unwinder itself: the nested fault lands back here
         * (fault_handler checks t_unwinding) and the fallback is abandoned, never recorded. */
        if (sigsetjmp(t_unwind_jmp, 0) == 0) {
            t_unwinding = true;
            count = backtrace(frames, (int)(sizeof(frames) / sizeof(frames[0])));
        }
        t_unwinding = false;
        int first = 0;
        for (int i = 0; i < count; i++) {
            if ((uintptr_t)frames[i] == stop->fault_rip) {
                first = i + 1;
                break;
            }
        }
        for (int i = first; i < count && stop->fault_host_frame_count < HOST_FAULT_HOST_FRAMES; i++) {
            stop->fault_host_frames[stop->fault_host_frame_count++] = (uintptr_t)frames[i];
        }
    }
}
#else
static void record_host_frames(host_stop *stop, const ucontext_t *context)
{
    (void)stop;
    (void)context;
}
#endif

/*
 * Async-signal-safe by construction, and now actually so.
 *
 * It writes plain scalars into THREAD-LOCAL storage and jumps. No allocation, no
 * stdio, no locking -- which matters more than it used to: a fault can land while
 * this thread holds an HLE lock, and taking any lock here would deadlock inside a
 * signal handler.
 *
 * T505 adds the faulting RIP, the host `rbp` chain and the guest `ebp` chain to the stop
 * record, still as plain stores: every read of memory that might be unmapped goes through
 * `process_vm_readv`, and turning the addresses into names waits for host_report, outside
 * the handler. The signal itself is the one thing that cannot be recovered afterwards.
 *
 * A fault on a thread that never armed is not ours to swallow. Putting the default
 * disposition back and returning lets the faulting instruction re-execute and kill
 * the process with the real signal, which is a true report; jumping would be a lie
 * and ignoring it would spin forever. `signal()` is on POSIX's async-signal-safe
 * list, `sigaction()` is not, which is why it is used here and nowhere else.
 */
static host_fault_hook fault_hook;

void host_run_set_fault_hook(host_fault_hook hook)
{
    fault_hook = hook;
}

static void fault_handler(int signal_number, siginfo_t *info, void *context)
{
    const host_fault_hook hook = fault_hook;
    if (hook != NULL && hook(signal_number, info, context)) {
        return; /* T1741: the guest write watch served and single steps a protected page */
    }
#if defined(__linux__) && defined(__x86_64__)
    if (t_unwinding) {
        siglongjmp(t_unwind_jmp, 1); /* the unwinder fallback faulted: drop it, keep the first fault */
    }
#endif
    if (!t_armed) {
        host_unarmed_fault(signal_number, info, context);
        return;
    }
    const int saved_errno = errno;
    t_stop.reason = HOST_STOP_FAULT;
    t_stop.signal_number = signal_number;
    t_stop.fault_address = info ? (uintptr_t)info->si_addr : 0u;
    t_stop.guest_address = 0u;
    t_stop.ordinal = 0u;
    t_stop.detail = "faulted inside lifted code";
    clear_fault_evidence(&t_stop);
    if (context != NULL) {
        record_host_frames(&t_stop, (const ucontext_t *)context);
    }
    record_guest_frames(&t_stop);
    errno = saved_errno;
    jump_active();
}

void host_run_arm(void)
{
    if(t_armed || t_depth!=0u)abort();
    memset(&t_stop, 0, sizeof(t_stop));
    t_stop.detail = "";

    pthread_mutex_lock(&install_lock);
#if defined(__linux__) && defined(__x86_64__)
    if (install_count == 0u) {
        void *warm[1]; /* load the unwinder now, the fault handler must not dlopen it */
        (void)backtrace(warm, 1);
    }
#endif
    if (install_count == 0u) {
        struct sigaction action;
        memset(&action, 0, sizeof(action));
        action.sa_sigaction = fault_handler;
        action.sa_flags = SA_SIGINFO | SA_NODEFER;
        sigemptyset(&action.sa_mask);
        for (size_t i = 0; i < FAULT_SIGNAL_COUNT; i++) {
            (void)sigaction(FAULT_SIGNALS[i], &action, &previous[i]);
        }
    }
    install_count++;
    pthread_mutex_unlock(&install_lock);

    /* Set last. A fault between installing the handler and arming this thread
     * would otherwise be swallowed by the not-armed branch above. */
    t_armed = true;
}

void host_run_disarm(void)
{
    if(t_depth!=0u)abort();
    if (!t_armed) {
        return;
    }
    t_armed = false;

    pthread_mutex_lock(&install_lock);
    if (install_count > 0u) {
        install_count--;
    }
    if (install_count == 0u) {
        for (size_t i = 0; i < FAULT_SIGNAL_COUNT; i++) {
            (void)sigaction(FAULT_SIGNALS[i], &previous[i], NULL);
        }
    }
    pthread_mutex_unlock(&install_lock);
}

void host_run_stop(host_stop_reason reason, uint32_t guest_address, unsigned ordinal,
                   const char *detail)
{
    t_stop.reason = reason;
    t_stop.guest_address = guest_address;
    t_stop.ordinal = ordinal;
    t_stop.fault_address = 0u;
    t_stop.signal_number = 0;
    clear_fault_evidence(&t_stop);
    t_stop.detail = detail ? detail : "";
    /* Abort rather than return if THIS THREAD never armed: returning would resume
     * lifted code that has already been told it cannot continue, which is the
     * silent-wrong-answer outcome this whole mechanism exists to prevent.
     *
     * Note the per-thread test. With a shared flag, one thread finishing its run
     * would make every later stop on another thread abort the process instead of
     * reporting -- losing the stop reason, the trace and the backlog. */
    if (!t_armed) {
        /* T2145: this used to abort with no word at all, which is how a stop on a non-guest thread looked like a crash. */
        fprintf(stderr, "host: stop on a thread that never armed (%s, guest address %#x, ordinal %u): %s\n",
                host_stop_reason_str(reason), guest_address, ordinal, t_stop.detail);
        abort();
    }
    jump_active();
}

/* ---- T2145 crash report ---------------------------------------------------------------------------------------------- */
#include <execinfo.h>
#include <sys/prctl.h>
#include <sys/syscall.h>
static host_crash_extra_fn crash_extra;
static volatile sig_atomic_t crash_active;
static void crash_puts(const char *text)
{
    size_t length = 0u;
    while (text[length] != '\0') length++;
    ssize_t ignored = write(2, text, length);
    (void)ignored;
}
static void crash_hex(unsigned long value)
{
    char buffer[19];
    int at = 18;
    buffer[at] = '\0';
    do { buffer[--at] = "0123456789abcdef"[value & 15u]; value >>= 4; } while (value != 0u && at > 2);
    buffer[--at] = 'x';
    buffer[--at] = '0';
    crash_puts(buffer + at);
}
extern bool live_census_write(int reason) __attribute__((weak)); /* T3824, src/host/live_census.c */
static void crash_handler(int signal_number, siginfo_t *info, void *context)
{
    (void)context;
    if (crash_active) _exit(128 + signal_number);
    crash_active = 1;
    char name[32] = "?";
    (void)prctl(PR_GET_NAME, (unsigned long)name, 0, 0, 0);
    crash_puts("\n=== HOST CRASH (T2145) ===\nsignal ");
    crash_hex((unsigned long)signal_number);
    crash_puts(signal_number == SIGABRT ? " (SIGABRT)" : " (trap)");
    crash_puts(" thread ");
    crash_puts(name);
    crash_puts(" tid ");
    crash_hex((unsigned long)syscall(SYS_gettid));
    crash_puts(" fault_address ");
    crash_hex(info != NULL ? (unsigned long)info->si_addr : 0ul);
    crash_puts(t_armed ? " armed\n" : " not-armed\n");
    void *frames[48];
    const int depth = backtrace(frames, 48);
    crash_puts("host backtrace (innermost first):\n");
    backtrace_symbols_fd(frames, depth, 2);
    if (crash_extra != NULL) crash_extra(2);
    if (live_census_write != NULL) (void)live_census_write(1); /* T3824: async-signal-safe, LIVE_CENSUS_REASON_SIGNAL */
    crash_puts("=== END HOST CRASH ===\n");
    (void)signal(signal_number, SIG_DFL);
    raise(signal_number);
}
void host_crash_report_install(host_crash_extra_fn extra)
{
    static char *stack;
    host_core_policy_apply(); /* T3745: no core dumps on the root disk (first thing every host does) */
    (void)host_rss_watchdog_start(); /* T3745: stop before the PC swaps */
    if (extra != NULL) crash_extra = extra;
    if (stack != NULL) return;
    void *probe[1];
    (void)backtrace(probe, 1); /* load libgcc now, not inside the handler */
    const size_t size = 256u * 1024u;
    stack = malloc(size);
    if (stack == NULL) return;
    stack_t alt = {.ss_sp = stack, .ss_size = size, .ss_flags = 0};
    (void)sigaltstack(&alt, NULL);
    struct sigaction action;
    memset(&action, 0, sizeof action);
    action.sa_sigaction = crash_handler;
    action.sa_flags = SA_SIGINFO | SA_ONSTACK | SA_NODEFER;
    sigemptyset(&action.sa_mask);
    (void)sigaction(SIGABRT, &action, NULL);
    /* Dispositions are process-wide: cover SDL and worker threads even when no
     * guest thread is armed; host_run_arm temporarily layers its handler on top. */
    action.sa_sigaction = host_unarmed_fault;
    action.sa_flags = SA_SIGINFO | SA_NODEFER;
    for (size_t i = 0; i < FAULT_SIGNAL_COUNT; i++)
        (void)sigaction(FAULT_SIGNALS[i], &action, NULL);
}
