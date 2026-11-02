/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "recomp_cooperative.h"
#include "host_runtime.h"
#include <stdlib.h>
static recomp_cooperative_provider configured_provider;
static void *configured_userdata;
static _Thread_local bool active;
static recomp_direct_call_interceptor configured_interceptor;
static void *interceptor_userdata;
#define RECOMP_DIRECT_CALL_DEPTH_MAX 64u
static _Thread_local uint32_t intercept_chain[RECOMP_DIRECT_CALL_DEPTH_MAX];
static _Thread_local unsigned intercept_depth;
extern int recomp_has_cooperative_calls(void) __attribute__((weak));
bool recomp_cooperative_ready(void)
{return recomp_has_cooperative_calls!=NULL && recomp_has_cooperative_calls()==1;}
bool recomp_cooperative_configure(recomp_cooperative_provider provider,void *userdata)
{
    if(active || intercept_depth != 0u)return false;
    configured_provider=provider;configured_userdata=userdata;return true;
}
bool recomp_direct_call_configure(recomp_direct_call_interceptor interceptor,void *userdata)
{
    if(active || intercept_depth != 0u)return false;
    configured_interceptor=interceptor;interceptor_userdata=userdata;return true;
}
/* T3740 fidelity: the capture-based Tier 2 offline runner (rust_ports.c t2o_run_lifted) replays every
 * nested call of the lifted control from the capture. Set only around that run, on its thread. */
_Thread_local bool (*recomp_t2o_direct_call)(uint32_t callee);
bool recomp_direct_call_intercept(uint32_t callee,uint32_t *eax)
{
    bool (*t2o)(uint32_t)=recomp_t2o_direct_call;
    if(__builtin_expect(t2o!=NULL,0))return t2o(callee);
    recomp_direct_call_interceptor interceptor=configured_interceptor;
    if(interceptor==NULL || eax==NULL || intercept_depth >= RECOMP_DIRECT_CALL_DEPTH_MAX)return false;
    for(unsigned i=0u;i<intercept_depth;i++)if(intercept_chain[i]==callee)return false;
    const unsigned previous_depth=intercept_depth;
    if(!host_run_armed())abort();
    volatile host_run_scope scope=HOST_RUN_SCOPE_INITIALIZER;
    if(!host_run_scope_init(&scope))abort();
    if(sigsetjmp(*host_run_scope_jmp(&scope),0)==0) {
        if(!host_run_scope_push(&scope)) {
            host_run_stop(HOST_STOP_UNIMPLEMENTED,callee,0u,"direct-call interception scope unavailable");
            abort();
        }
        intercept_chain[previous_depth]=callee;
        intercept_depth=previous_depth+1u;
        const uint32_t original_eax=*eax;
        const bool handled=interceptor(callee,eax,interceptor_userdata);
        if(!handled)*eax=original_eax;
        intercept_depth=previous_depth;
        if(!host_run_scope_pop(&scope))abort();
        return handled;
    }
    intercept_depth=previous_depth;
    if(!host_run_scope_pop(&scope))abort();
    host_run_rethrow(host_run_result());
}
void recomp_call_safepoint(uint32_t callee)
{
    recomp_cooperative_provider provider=configured_provider;
    if(provider==NULL || active)return;
    if(!host_run_armed())abort();
    volatile host_run_scope scope=HOST_RUN_SCOPE_INITIALIZER;
    if(!host_run_scope_init(&scope))abort();
    if(sigsetjmp(*host_run_scope_jmp(&scope),0)==0) {
        if(!host_run_scope_push(&scope)) {
            host_run_stop(HOST_STOP_UNIMPLEMENTED,callee,0u,"cooperative stop scope unavailable");
            abort();
        }
        active=true;provider(callee,configured_userdata);
        active=false;if(!host_run_scope_pop(&scope))abort();
    } else {
        active=false;if(!host_run_scope_pop(&scope))abort();
        host_run_rethrow(host_run_result());
    }
}
