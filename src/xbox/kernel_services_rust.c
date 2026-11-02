/* SPDX-License-Identifier: GPL-3.0-or-later
 * T2208: serialized dual replay of safe Rust services and retained C oracle.
 * Explicit ordinal lists only; selection freezes on first dispatch. C owns host effects.
 */
#include "kernel_services_rust.h"
#ifdef TSFP_RUST_KSERVICES /* empty unit when the Rust lib is not linked (globbed test builds) */
#include "kernel_call.h"
#include <pthread.h>
#include <stdlib.h>
#include <string.h>

static const unsigned ordinals[] = {24,335,336,337,338,339,340,346,47,49,252,253,1,2,3,4,8,95,139,142};
static const unsigned arities[] = {5,1,3,2,3,3,7,2,2,1,1,2,0,4,6,1,257,1,1,1};
static unsigned modes[371]; /* 0 C, 1 Rust guarded by oracle, 2 compare */
static uint64_t calls[371], mismatches[371];
static bool enabled;
static pthread_once_t once = PTHREAD_ONCE_INIT;
static pthread_mutex_t lock;
void *tsfp_services_prepare(uint32_t, const uint32_t *, size_t, uint64_t,
    bool (*)(uint32_t,void *,size_t), bool (*)(uint32_t,const void *,size_t), bool (*)(uint32_t,size_t));
uint32_t tsfp_services_finish(void *, uint32_t, bool, bool (*)(uint32_t,void *,size_t),
    bool (*)(uint32_t,const void *,size_t), bool *);
bool tsfp_services_restored(const void *);
void tsfp_services_discard(void *);
static bool supported(unsigned ordinal) {
    for (size_t i=0; i<sizeof(ordinals)/sizeof(ordinals[0]); ++i) if (ordinals[i]==ordinal) return true;
    return false;
}
static bool parse(const char *env, unsigned mode, unsigned *out) {
    if (!env || !*env || strcmp(env,"0")==0) return true;
    while (*env) {
        if (*env < '0' || *env > '9') return false;
        char *end; unsigned long ordinal = strtoul(env,&end,10);
        if (ordinal >= 371 || !supported((unsigned)ordinal) || (*end && *end!=',')) return false;
        out[ordinal]=mode;
        if (!*end) return true;
        env=end+1; if (!*env) return false;
    }
    return true;
}
static void initialize(void) {
    pthread_mutexattr_t attr; pthread_mutexattr_init(&attr);
    pthread_mutexattr_settype(&attr,PTHREAD_MUTEX_RECURSIVE);
    pthread_mutex_init(&lock,&attr); pthread_mutexattr_destroy(&attr);
    unsigned parsed[371]={0};
    if (!parse(getenv("TSFP_RUST_KSERVICES"),1,parsed) || !parse(getenv("TSFP_KSERVICES_COMPARE"),2,parsed)) {
        kernel_hle_log()("kernel-services: invalid ordinal list; all selections OFF\n"); return;
    }
    memcpy(modes,parsed,sizeof(modes));
    for (unsigned i=0;i<371;++i) enabled |= modes[i]!=0;
}
uint32_t kernel_services_dispatch(unsigned ordinal, kernel_fn oracle, void *context) {
    pthread_once(&once,initialize);
    if (!enabled || !supported(ordinal)) return oracle(context);
    pthread_mutex_lock(&lock);
    unsigned arity=0;
    for (size_t i=0;i<sizeof(ordinals)/sizeof(ordinals[0]);++i) if (ordinals[i]==ordinal) arity=arities[i];
    uint32_t args[257]; unsigned count=0;
    while (count<arity && kernel_frame_arg(context,count,&args[count])) ++count;
    /* The stable actual host thread identifies FP ownership, matching the C model. */
    uint64_t thread=0; pthread_t self=pthread_self();
    _Static_assert(sizeof(self)<=sizeof(thread),"pthread identity fits u64 on supported host");
    memcpy(&thread,&self,sizeof(self));
    void *pending=tsfp_services_prepare(ordinal,args,count,thread,kernel_guest_read_bytes,
        kernel_guest_write_bytes,kernel_guest_range_readable);
    if (!tsfp_services_restored(pending)) {
        tsfp_services_discard(pending);
        pthread_mutex_unlock(&lock);
        kernel_hle_fatal(ordinal,"kernel-services replay could not restore guest writes; oracle not run");
        return 0xc0000005u;
    }
    /* Consume replay evidence and release every lock before a terminal C hook.
     * The production hook may longjmp instead of returning. No Rust allocation,
     * Rust mutex or bridge mutex may survive that boundary. */
    if (ordinal==49 || ordinal==95) {
        bool mismatch=false;
        uint32_t expected=tsfp_services_finish(pending,0,modes[ordinal]==1,
            kernel_guest_read_bytes,kernel_guest_write_bytes,&mismatch);
        if (modes[ordinal]) {
            ++calls[ordinal];
            if (mismatch) ++mismatches[ordinal];
        }
        pthread_mutex_unlock(&lock);
        uint32_t result=oracle(context);
        if (result!=expected && modes[ordinal]) {
            pthread_mutex_lock(&lock); ++mismatches[ordinal]; pthread_mutex_unlock(&lock);
            kernel_hle_log()("kernel-services: ordinal %u terminal return mismatch; C retained\n",ordinal);
        }
        return result==expected && !mismatch && modes[ordinal]==1 ? expected : result;
    }
    /* C delivers diagnostic and display effects once, after Rust returns. */
    uint32_t result=oracle(context); bool mismatch=false;
    result=tsfp_services_finish(pending,result,modes[ordinal]==1,kernel_guest_read_bytes,
        kernel_guest_write_bytes,&mismatch);
    if (modes[ordinal]) {
        ++calls[ordinal];
        if (mismatch) { ++mismatches[ordinal]; kernel_hle_log()("kernel-services: ordinal %u mismatch; C retained\n",ordinal); }
    }
    pthread_mutex_unlock(&lock); return result;
}

uint64_t kernel_services_mismatches(unsigned ordinal) {
    pthread_once(&once,initialize); pthread_mutex_lock(&lock);
    uint64_t n=ordinal<371?mismatches[ordinal]:0; pthread_mutex_unlock(&lock); return n;
}
uint64_t kernel_services_calls(unsigned ordinal) {
    pthread_once(&once,initialize); pthread_mutex_lock(&lock);
    uint64_t n=ordinal<371?calls[ordinal]:0; pthread_mutex_unlock(&lock); return n;
}

unsigned kernel_services_mode(unsigned ordinal) {
    pthread_once(&once,initialize); return ordinal<371?modes[ordinal]:0;
}
#endif /* TSFP_RUST_KSERVICES */
