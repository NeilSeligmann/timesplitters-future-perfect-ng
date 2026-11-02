/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * T3442: deterministic guest thread scheduling for replays (TSFP_DETSCHED=1, needs TSFP_FAST_FORWARD).
 *
 * Guest threads run as host threads. Without this mode they run CONCURRENTLY, so the interleaving of
 * the owner, loader and sound threads (and with it async read submission and completion) follows host
 * scheduling and load, and off/off replays diverge (T3443, MEASURED). The Xbox CPU is uniprocessor:
 * exactly one guest thread runs at a time. This mode restores that with a global BATON:
 *
 * - only the baton holder executes guest code; every other participant waits on its own condition;
 * - the baton moves only at well-defined points: a blocking HLE wait (KeWaitForSingleObject,
 *   NtWaitForSingleObject on a thread, a contended RtlEnterCriticalSection, a virtual-time sleep from
 *   KeDelayExecutionThread or the pseudo pause), NtYieldExecution, thread start and exit, a host hold
 *   loop that waits for another guest thread, and quantum preemption at the cooperative safepoint
 *   (every lifted call) after a fixed number of the holder's own safepoints at PASSIVE IRQL;
 * - the next holder is chosen round robin by participant id (boot thread 0, guest thread slot + 1);
 *   a blocked participant receives the baton only to re-test its wait condition;
 * - when every participant re-tested and is still blocked, virtual time jumps to the earliest pending
 *   timed wait (replacing the wall-clock idle detection of kernel_vtime.c); if nothing is pending, the
 *   watchdog ends the run with the named stop "T3442 detsched deadlock".
 *
 * Scheduling policy is INFERRED (round robin, quantum FABRICATED); the Xbox kernel schedules by
 * priority with a time quantum, which this does not model. What it guarantees is that the same
 * inputs give the same interleaving. Unset, nothing here changes behaviour.
 *
 * KNOWN LIMIT (MEASURED, docs/t3442-deterministic-scheduling.md): a guest loop with no lifted call (the
 * loading bar gate sub_00156840 spinning on the worker's progress) never reaches a hand-off point, so
 * the mode stalls there with the stuck-holder stop. Opt-in diagnostic until loops get a preemption point.
 */
#ifndef TSFP_KERNEL_DETSCHED_H
#define TSFP_KERNEL_DETSCHED_H

#include <stdbool.h>
#include <stdint.h>

/* Participants: the boot thread plus every guest thread slot. */
#define KERNEL_DETSCHED_MAX 65u
#define KERNEL_DETSCHED_BOOT_ID 0u
/* Own safepoints between preemptions (TSFP_DETSCHED_QUANTUM overrides). FABRICATED. */
#define KERNEL_DETSCHED_DEFAULT_QUANTUM 4096u
/* Wall milliseconds of no progress before the named stop (TSFP_DETSCHED_WATCHDOG_MS overrides). */
#define KERNEL_DETSCHED_DEFAULT_WATCHDOG_MS 60000u

/* TSFP_DETSCHED set and not "0", and the virtual time base on. Read once. */
bool kernel_detsched_enabled(void);
/* True when the calling host thread is a participant (holds or waits for the baton). */
bool kernel_detsched_participant(void);

/* The creator (holding the baton) registers a participant before its host thread starts. The first
 * registration of the run takes the baton. */
void kernel_detsched_register(unsigned id);
/* A registered participant whose host thread will never run (creation failed). */
void kernel_detsched_unregister(unsigned id);
/* The participant's own host thread: bind and wait for the baton. */
void kernel_detsched_attach(unsigned id);
/* Leave for good (thread exit, boot entry returned) and pass the baton on. */
void kernel_detsched_detach(void);

/* Voluntary hand-off (NtYieldExecution). True when another participant ran. */
bool kernel_detsched_yield(void);
/* The caller's wait condition does not hold: give the baton away and return when it comes back, so the
 * caller re-tests. Call with no host lock held. Non-participants sleep briefly instead. */
void kernel_detsched_block(void);
/* Guest progress outside safepoints (an HLE dispatch returned): the all-blocked test needs it. */
void kernel_detsched_note_progress(void);
/* One cooperative safepoint of the calling thread (quantum preemption at PASSIVE IRQL). */
void kernel_detsched_safepoint(void);

/* Named stop for a deadlock or a stuck holder; NULL restores the default (message and abort). */
void kernel_detsched_set_stop_sink(void (*sink)(const char *detail));

typedef struct kernel_detsched_stats {
    uint64_t handoffs;      /* baton moves */
    uint64_t preemptions;   /* quantum expiries that moved the baton */
    uint64_t yields;        /* NtYieldExecution hand-offs */
    uint64_t blocks;        /* blocked re-test hand-offs */
    uint64_t idle_jumps;    /* virtual-time jumps when every participant was blocked */
    uint64_t external_waits; /* all blocked, nothing pending: short wall sleeps (host hold) */
    unsigned participants_max;
} kernel_detsched_stats;
kernel_detsched_stats kernel_detsched_get_stats(void);

/* Tests only. */
void kernel_detsched_test_reset(bool enabled, unsigned quantum);

#endif
