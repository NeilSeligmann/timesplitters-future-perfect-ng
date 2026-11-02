/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "dsound_stream.h"
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "dsound_hle.h"
#include "dsound_audio_runtime.h"
#include "dsound_completion.h"
#include "dsound_stream_frequency.h"
#include "kernel_clock.h"
#include "guest_mem.h"
#include "dsound_audio_guest_heap.h"
#include "kernel_call.h"
#include "dsound_stream_routing.h"
#if defined(TSFP_RUST_AUDIO_HOST) || defined(TSFP_RUST_AUDIO_STREAM_FREQUENCY)
#define TSFP_RUST_STREAM_FREQUENCY_STATE 1
#include "dsound_stream_frequency_ffi.h"
_Static_assert(sizeof(dsound_stream_frequency_state)==12u,"Rust stream-frequency ABI layout changed");
#endif
#ifdef TSFP_RUST_AUDIO_HOST
#include "dsound_audio_rust_frames.h"
#include "dsound_stream_lease.h"
_Static_assert(sizeof(dsound_stream_snapshot)==440u &&
    offsetof(dsound_stream_snapshot,cache_mask)==144u &&
    offsetof(dsound_stream_snapshot,pause_seen)==204u &&
    offsetof(dsound_stream_snapshot,format)==252u &&
    offsetof(dsound_stream_snapshot,routing)==272u &&
    offsetof(dsound_stream_snapshot,position_bits)==424u,"stream snapshot ABI drift");
extern void tsfp_audio_stream_snapshot_project_rust(dsound_stream_snapshot *,
    const dsound_stream_format_state *,const dsound_stream_volume_state *,
    const dsound_stream_spatial_state *,const dsound_stream_control_state *);
extern bool tsfp_audio_stream_set_frequency_rust(uint32_t stream,uint64_t serial,
    uint32_t scope_flags,uint32_t scope_channels,uint32_t scope_rate,uint32_t format_sets,
    const uint8_t *format_bytes,uint32_t cache_mask,uint32_t flags,uint32_t hertz,
    uint32_t control_word,void *context,
    bool (*apply)(void *,uint32_t,uint64_t,uint32_t,uint32_t),const char **error);
extern bool tsfp_audio_stream_observe_rust(uint32_t stream,dsound_stream_snapshot *out,
    const char **error);
extern size_t tsfp_audio_stream_routing_register_rust(void);
extern size_t tsfp_audio_stream_spatial_register_rust(void);
extern size_t tsfp_audio_stream_i3dl2_register_rust(void);
extern size_t tsfp_audio_stream_minimum_register_rust(void);
extern size_t tsfp_audio_stream_rolloff_register_rust(void);
extern bool tsfp_audio_stream_frequency_admit_rust(uint32_t stereo,uint32_t spatial,
    uint32_t completion,uint32_t frequency_handler,uint32_t volume_seen,
    uint32_t discontinuity_seen);
extern bool tsfp_audio_stream_lease_identity_rust(uint32_t stream_address, dsound_device_lease *identity_out);
extern bool tsfp_audio_stream_status_admit_rust(uint32_t flags,uint32_t mask,uint32_t rate,
    uint32_t channels,uint32_t block,uint32_t average,uint32_t packets,uint32_t volume_seen,
    int32_t volume,uint32_t pause,uint32_t flush,uint32_t discontinuity);
extern bool tsfp_audio_stream_lease_heap_rust(uint32_t stream_address, uint32_t *heap_out);
extern bool tsfp_audio_stream_create_abort_rust(uint32_t stream_address, uint32_t stream_heap,
    uint32_t publication_address, bool lease_reserved, bool routing_reserved);
extern bool tsfp_audio_stream_reset_rust(void);
extern bool tsfp_audio_stream_create_tables_stopped_rust(int32_t host_pid);
extern bool tsfp_audio_stream_lease_publication_address_rust(uint32_t stream_address, uint32_t *output);
extern bool tsfp_audio_stream_lease_max_distance_rust(uint32_t stream_address,
    const dsound_device_lease *lease, uint32_t apply, uint32_t distance_bits);
extern uint32_t tsfp_audio_stream_lease_startup_spatial_rust(uint32_t stream_address,
    const dsound_device_lease *lease, uint32_t action, uint32_t argument, uint32_t count,
    const uint32_t *i3dl2_words);
extern size_t tsfp_audio_stream_control_register_rust(void);
extern size_t tsfp_audio_stream_create_register_rust(void);
extern const char *tsfp_audio_stream_create_policy_error_rust(
    uint32_t enabled, uint32_t irql_known, uint32_t irql, int32_t host_pid);
extern uint32_t tsfp_audio_stream_create_outcome_rust(
    uint32_t lease_status, uint32_t create_status, uint32_t has_error);
extern void tsfp_audio_stream_create_reset_announcement_rust(void);
extern uint32_t tsfp_audio_stream_create_cleanup_untracked_rust(void);
extern void tsfp_audio_stream_create_release_request_rust(void *request);
extern bool tsfp_audio_stream_create_untracked_overlaps_rust(uint32_t address,uint32_t bytes);
extern void tsfp_audio_stream_create_finalize_callback_rust(
    const dsound_device_lease *lease, void *userdata);
extern void tsfp_audio_stream_create_set_enabled_rust(bool enabled);
extern bool tsfp_audio_stream_create_enabled_rust(void);
extern bool tsfp_audio_stream_device_singleton_rust(int32_t host_pid, uint32_t *output);
extern bool tsfp_audio_stream_create_probe_output_rust(int32_t host_pid, uint32_t address);
extern bool tsfp_audio_stream_create_write_header_rust(int32_t host_pid, uint32_t address,
                                                        uint32_t internal_address);
extern bool tsfp_audio_stream_create_reserve_rust(uint32_t stream_address,uint32_t stream_heap,
    uint32_t publication_address,uint32_t internal_address,const dsound_stream_scope *scope);
extern bool tsfp_audio_stream_create_fixed_overlap_rust(uint32_t address,uint32_t bytes,
    uint32_t internal,bool include_curve);
extern bool tsfp_audio_stream_create_output_overlap_rust(uint32_t output,uint32_t internal,
                                                         uint32_t descriptor,uint32_t format);
extern bool tsfp_audio_stream_create_allocation_overlap_rust(uint32_t address,uint32_t output,
                                                              uint32_t descriptor,uint32_t format);
typedef struct tsfp_audio_stream_create_prepare_result {
    uint32_t code,stream_heap,stream_address,lease_reserved,routing_reserved;
} tsfp_audio_stream_create_prepare_result;
extern uint32_t tsfp_audio_stream_create_prepare_rust(int32_t host_pid,
    const dsound_device_lease *candidate,const void *scope,uint32_t output,
    tsfp_audio_stream_create_prepare_result *result);
extern bool tsfp_audio_stream_create_prepare_callback_rust(
    const dsound_device_lease *candidate,void *userdata,dsound_device_lease_child *child);
extern void tsfp_audio_stream_create_cleanup_aborted_rust(void *userdata);
extern const char *tsfp_audio_stream_create_policy_error_host_c(void);
const char *tsfp_audio_stream_create_policy_error_snapshot_host_c(void);
void tsfp_audio_stream_create_acquire_host_c(uint32_t device,void *request,
    uint32_t *status,dsound_device_lease *lease);
void tsfp_audio_stream_create_cleanup_host_c(void *request);
void tsfp_audio_stream_create_report_host_c(const void *request,
    const dsound_device_lease *lease,uint32_t status);
void tsfp_audio_stream_create_refuse_host_c(const char *reason);
uint32_t tsfp_audio_stream_reset_release_host_c(uint32_t address,
    const dsound_device_lease *identity);
bool tsfp_audio_stream_reset_validate_host_c(uint32_t address,
    const dsound_device_lease *identity);
bool tsfp_audio_stream_reset_heap_destroy_host_c(uint32_t heap);
void tsfp_audio_stream_reset_log_host_c(uint32_t code,uint32_t address,uint64_t serial);
extern size_t tsfp_audio_stream_format_register_rust(void);
extern uint32_t tsfp_audio_stream_format_admit_rust(uint32_t scope_flags,
    uint32_t cache_mask,uint32_t volume_seen,uint32_t discontinuity_seen);
extern size_t tsfp_audio_stream_frequency_register_rust(void);
extern size_t tsfp_audio_stream_volume_register_rust(void);
extern uint32_t tsfp_audio_stream_routing_scope_admit_rust(uint32_t stream,
    const dsound_device_lease *lease,uint32_t completion_available);
_Static_assert(sizeof(dsound_stream_scope)==72u,"Rust stream-scope ABI layout changed");
_Static_assert(sizeof(dsound_stream_format_state)==24u,"Rust stream-format ABI layout changed");
_Static_assert(sizeof(dsound_stream_volume_state)==8u,"Rust stream-volume ABI layout changed");
_Static_assert(sizeof(dsound_stream_control_state)==32u,"Rust stream-control ABI layout changed");
_Static_assert(sizeof(dsound_stream_control_request)==24u,"Rust stream-control request ABI layout changed");
_Static_assert(sizeof(dsound_stream_spatial_state)==84u,"Rust stream-spatial ABI layout changed");
#endif
#define CREATE 0x0040967Cu
#define I3DL2 0x00408637u
#define MINIMUM 0x004085F1u
#define MAX_DISTANCE 0x004085D9u
#define SET_POSITION 0x00408609u
#define ROLLOFF 0x00408632u
#define STATUS 0x004073D3u
#define VOLUME 0x00407B14u
#define PAUSE 0x00407B23u
#define SET_FORMAT 0x00408C2Du
#define HEADROOM 0x00407B19u
#define MIX_VOLUMES 0x00407B1Eu
#define MIX_BINS 0x004085D4u
#define FLUSH_EX 0x00407B28u
#define DISCONTINUITY 0x0040733Bu
#define SET_FREQUENCY 0x004085CFu
#define SINGLETON 0x00412B30u
#define GLOBAL_STATE 0x004124A8u
#define VTABLE 0x004A1D00u
#define SECONDARY 0x004A1CF0u
#define CURVE 0x004B914Cu
#define CURVE_NEAR 0x004B915Cu
#define CURVE_FAR 0x004B916Cu
#define STREAM_BYTES 40u
#define OOM 0x8007000Eu
#ifdef TSFP_RUST_AUDIO_HOST
#define CREATE_OUTCOME_READY 0u
#define CREATE_OUTCOME_OOM 1u
#endif
#ifdef TSFP_RUST_AUDIO_HOST
extern void tsfp_audio_stream_mutex_lock_rust(void);
extern void tsfp_audio_stream_mutex_unlock_rust(void);
extern void tsfp_audio_stream_reset_mutex_lock_rust(void);
extern void tsfp_audio_stream_reset_mutex_unlock_rust(void);
static unsigned char rust_stream_lock_marker;
static unsigned char rust_reset_lock_marker;
static void stream_mutex_lock_dispatch(void *mutex)
{
    if(mutex==(void *)&rust_stream_lock_marker)tsfp_audio_stream_mutex_lock_rust();
    else if(mutex==(void *)&rust_reset_lock_marker)tsfp_audio_stream_reset_mutex_lock_rust();
    else (pthread_mutex_lock)((pthread_mutex_t *)mutex);
}
static void stream_mutex_unlock_dispatch(void *mutex)
{
    if(mutex==(void *)&rust_stream_lock_marker)tsfp_audio_stream_mutex_unlock_rust();
    else if(mutex==(void *)&rust_reset_lock_marker)tsfp_audio_stream_reset_mutex_unlock_rust();
    else (pthread_mutex_unlock)((pthread_mutex_t *)mutex);
}
#define pthread_mutex_lock(m) stream_mutex_lock_dispatch((void *)(m))
#define pthread_mutex_unlock(m) stream_mutex_unlock_dispatch((void *)(m))
#define lock rust_stream_lock_marker
#define reset_lock rust_reset_lock_marker
#else
static pthread_mutex_t lock=PTHREAD_MUTEX_INITIALIZER;
static pthread_mutex_t reset_lock=PTHREAD_MUTEX_INITIALIZER;
#endif
static bool completion;
#ifndef TSFP_RUST_AUDIO_HOST
static bool announced;
#endif
#ifndef TSFP_RUST_AUDIO_HOST
static bool enabled;
#endif
static uint64_t accepted_frequency_sets;
static dsound_stream_pause_note_fn pause_note;
static dsound_stream_routing_note_fn routing_note;
static dsound_stream_frequency_note_fn frequency_note;
static dsound_stream_format_note_fn format_note;
static dsound_stream_control_word_fn frequency_control_word;
static dsound_stream_irql_provider irql_provider;
static dsound_stream_fatal_fn fatal_handler;
static dsound_stream_public_route_fn public_route;
static dsound_stream_reset_route_fn reset_route;
typedef struct stream_node {
#ifdef TSFP_RUST_AUDIO_HOST
    /* Rust owns rollback membership; this ABI node carries only identity and heap. */
    struct {
        uint32_t stream_address,stream_heap;
    } value;
#else
    dsound_stream_snapshot value;
#endif
    struct stream_node *next;
} stream_node;
#ifdef TSFP_RUST_AUDIO_HOST
_Static_assert(sizeof(stream_node)==sizeof(void *)+8u,
    "Rust stream-create rollback node ABI layout changed");
/* Rust owns committed lifecycle records and failed-create rollback membership. */
#else
static stream_node *streams,*detached;
#endif
#ifndef TSFP_RUST_AUDIO_HOST
static stream_node *rollback;
#endif
static bool overlap(uint32_t a,uint32_t an,uint32_t b,uint32_t bn)
{return (uint64_t)a<(uint64_t)b+bn && (uint64_t)b<(uint64_t)a+an;}
static void refuse(uint32_t entry,const char *reason) __attribute__((noreturn));
static void refuse(uint32_t entry,const char *reason)
{
    pthread_mutex_lock(&lock);dsound_stream_fatal_fn fatal=fatal_handler;pthread_mutex_unlock(&lock);
    dsound_hle_log()("dsound passive stream %#x refused: %s\n",entry,reason);
    if(fatal!=NULL)fatal(entry,reason);abort();
}
static bool stream_enabled_locked(void)
{
#ifdef TSFP_RUST_AUDIO_HOST
    return tsfp_audio_stream_create_enabled_rust();
#else
    return enabled;
#endif
}
void dsound_stream_set_enabled(bool value)
{
    pthread_mutex_lock(&lock);
#ifdef TSFP_RUST_AUDIO_HOST
    tsfp_audio_stream_create_set_enabled_rust(value);
#else
    enabled=value;
#endif
    pthread_mutex_unlock(&lock);
}
void dsound_stream_set_completion(bool value,dsound_stream_pause_note_fn note)
{pthread_mutex_lock(&lock);completion=value;pause_note=value?note:NULL;pthread_mutex_unlock(&lock);}
void dsound_stream_set_routing_note(dsound_stream_routing_note_fn note)
{pthread_mutex_lock(&lock);routing_note=note;pthread_mutex_unlock(&lock);}
uint64_t dsound_stream_frequency_set_count(void)
{pthread_mutex_lock(&lock);const uint64_t count=accepted_frequency_sets;pthread_mutex_unlock(&lock);return count;}
void dsound_stream_set_format_note(dsound_stream_format_note_fn note)
{pthread_mutex_lock(&lock);format_note=note;pthread_mutex_unlock(&lock);}
void dsound_stream_set_frequency_note(dsound_stream_frequency_note_fn note,
    dsound_stream_control_word_fn control_word)
{
    pthread_mutex_lock(&lock);frequency_note=note;frequency_control_word=control_word;
    pthread_mutex_unlock(&lock);
}
void dsound_stream_set_irql_provider(dsound_stream_irql_provider provider)
{pthread_mutex_lock(&lock);irql_provider=provider;pthread_mutex_unlock(&lock);}
void dsound_stream_set_fatal(dsound_stream_fatal_fn fatal)
{pthread_mutex_lock(&lock);fatal_handler=fatal;pthread_mutex_unlock(&lock);}
void dsound_stream_set_extension(dsound_stream_public_route_fn route,dsound_stream_reset_route_fn reset)
{pthread_mutex_lock(&lock);public_route=route;reset_route=reset;pthread_mutex_unlock(&lock);}
/* Called with stream lock held. Provider must only report current IRQL, never
 * reenter this adapter. Kernel TLS observation does not enter the device lock. */
static const char *policy_error(void)
{
#ifdef TSFP_RUST_AUDIO_HOST
    uint8_t irql=0u;
    const bool active=stream_enabled_locked();
    bool irql_known=false;
    int32_t host_pid=0;
    /* Preserve the oracle's short-circuit observation order: disabled skips IRQL,
     * and unsupported IRQL skips the guest global read. Rust owns the decision. */
    if(active) {
        irql_known=irql_provider!=NULL&&irql_provider(&irql);
        if(irql_known && irql==0u)host_pid=(int32_t)kernel_host_pid();
    }
    return tsfp_audio_stream_create_policy_error_rust(active?1u:0u,
        irql_known?1u:0u,irql,(int32_t)host_pid);
#else
    uint32_t state;uint8_t irql;
    if(!stream_enabled_locked())return "explicit headless-streams policy is disabled";
    if(irql_provider==NULL || !irql_provider(&irql) || irql!=0u)return "only known IRQL0 is supported";
    if(!kernel_guest_read_u32(GLOBAL_STATE,&state) || state!=0u)return "original global audio state must be zero";
    return NULL;
#endif
}
#ifdef TSFP_RUST_AUDIO_HOST
const char *tsfp_audio_stream_create_policy_error_host_c(void)
{ return policy_error(); }
const char *tsfp_audio_stream_create_policy_error_snapshot_host_c(void)
{
    pthread_mutex_lock(&lock);const char *error=policy_error();pthread_mutex_unlock(&lock);
    return error;
}
#endif
#ifndef TSFP_RUST_AUDIO_HOST
static stream_node *find(uint32_t address)
{
    for(stream_node *n=streams;n!=NULL;n=n->next)
        if(n->value.stream_address==address)return n;
    return NULL;
}
#endif
static bool token_equal(const dsound_device_lease *a,const dsound_device_lease *b)
{
#ifdef TSFP_RUST_AUDIO_HOST
    return tsfp_audio_stream_lease_identity_matches_rust(a,b);
#else
    return a->device_heap==b->device_heap && a->internal_address==b->internal_address && a->serial==b->serial;
#endif
}
#ifdef TSFP_RUST_AUDIO_HOST
/* The Rust lease validator calls GuestRegions directly when active; this callback is the C-heap fallback. */
static bool stream_lease_probe(void *context,uint32_t stage,uint32_t heap,uint32_t address,
                               uint32_t *block_bytes)
{
    (void)context;
    switch(stage) {
    case DSOUND_STREAM_LEASE_DEVICE_HEAP:
        return kmem_heap_valid(heap);
    case DSOUND_STREAM_LEASE_STREAM_HEAP:
        return kmem_heap_valid(heap);
    case DSOUND_STREAM_LEASE_BLOCK_SIZE:
        return block_bytes!=NULL && kmem_heap_block_size(heap,address,block_bytes);
    default:
        return false;
    }
}
#endif
static bool valid_node(const stream_node *node,const dsound_device_lease *identity,uint32_t internal)
{
#ifdef TSFP_RUST_AUDIO_HOST
    if(node==NULL)return false;
    return tsfp_audio_stream_lease_validate_record_rust(node->value.stream_address,identity,internal,
        NULL,stream_lease_probe);
#else
    uint32_t actual[10],bytes;
    return node!=NULL && token_equal(&node->value.lease,identity) && identity->internal_address==internal &&
        dsound_audio_heap_valid(identity->device_heap) &&
        kernel_guest_read_bytes(node->value.stream_address,actual,sizeof(actual)) &&
        memcmp(actual,node->value.header,sizeof(actual))==0 && dsound_audio_heap_valid(node->value.stream_heap) &&
        dsound_audio_heap_block_size(node->value.stream_heap,node->value.stream_address,&bytes) && bytes==STREAM_BYTES;
#endif
}
#ifndef TSFP_RUST_AUDIO_HOST
static bool commit_routing(uint32_t stream,uint64_t serial,dsound_stream_routing *state,
                           const dsound_stream_routing *candidate,bool apply_pcm)
{
    if(apply_pcm && (routing_note==NULL || !routing_note(stream,serial,candidate)))return false;
    *state=*candidate;
    return true;
}
#endif
#ifdef TSFP_RUST_AUDIO_HOST
static bool commit_volume_routing(uint32_t stream,uint64_t serial,uint32_t volume,bool apply_pcm)
{
    dsound_stream_routing_note_fn note=routing_note;
    dsound_stream_routing_apply_fn apply=NULL;
    void *context=NULL;
    if(apply_pcm) {
        if(note==NULL)return false;
        apply=tsfp_audio_stream_routing_apply_note_rust;
        context=&note;
    }
    return tsfp_audio_stream_routing_volume_commit_rust(stream,serial,volume,context,apply);
}
#endif
#ifdef TSFP_RUST_STREAM_FREQUENCY_STATE
static bool frequency_apply_rust(void *context,uint32_t stream,uint64_t serial,
                                 uint32_t old_hz,uint32_t new_hz)
{
    if(context==NULL)return false;
    dsound_stream_frequency_note_fn note=*(dsound_stream_frequency_note_fn *)context;
    return note!=NULL && note(stream,serial,old_hz,new_hz);
}
static bool frequency_format_apply_rust(void *context,uint32_t stream,uint64_t serial,
                                        uint32_t old_hz,uint32_t new_hz,uint32_t format_address)
{
    if(context==NULL)return false;
    dsound_stream_format_note_fn note=*(dsound_stream_format_note_fn *)context;
    return note!=NULL && note(stream,serial,old_hz,new_hz,format_address);
}
#endif
#ifndef TSFP_RUST_AUDIO_HOST
static bool load_routing(uint32_t stream,uint64_t serial,
                         const dsound_stream_routing *state,dsound_stream_routing *output)
{
    (void)stream;
    (void)serial;
    if(state==NULL || output==NULL)return false;
    *output=*state;return true;
}
#endif
#ifndef TSFP_RUST_AUDIO_HOST
static bool aliases_state(uint32_t address,uint32_t bytes,uint32_t internal)
{
#ifdef TSFP_RUST_AUDIO_HOST
    if(tsfp_audio_stream_create_fixed_overlap_rust(address,bytes,internal,true))return true;
#else
    if(overlap(address,bytes,internal,44u) || overlap(address,bytes,SINGLETON,4u) ||
       overlap(address,bytes,GLOBAL_STATE,4u) || overlap(address,bytes,SECONDARY,60u) ||
       overlap(address,bytes,CURVE,16u))return true;
#endif
#ifdef TSFP_RUST_AUDIO_HOST
    if(tsfp_audio_stream_lease_overlaps_guest_span_rust(address,bytes))return true;
#endif
#ifdef TSFP_RUST_AUDIO_HOST
    if(tsfp_audio_stream_lease_overlaps_i3dl2_span_rust(address,bytes))return true;
#else
    for(stream_node *n=streams;n!=NULL;n=n->next)
        if(overlap(address,bytes,n->value.stream_address,STREAM_BYTES) ||
           overlap(address,bytes,n->value.publication_address,4u) ||
           overlap(address,bytes,n->value.scope.descriptor_address,24u) ||
           overlap(address,bytes,n->value.scope.format_address,20u) ||
           (n->value.i3dl2_address!=0u && overlap(address,bytes,n->value.i3dl2_address,36u)))return true;
    for(stream_node *n=detached;n!=NULL;n=n->next)
        if(overlap(address,bytes,n->value.stream_address,STREAM_BYTES))return true;
#endif
#ifdef TSFP_RUST_AUDIO_HOST
    if(tsfp_audio_stream_create_untracked_overlaps_rust(address,bytes))return true;
#else
    for(stream_node *n=rollback;n!=NULL;n=n->next)
        if(n->value.stream_address!=0u && overlap(address,bytes,n->value.stream_address,STREAM_BYTES))return true;
#endif
    return false;
}
#endif
/* Input buffers are historical stack snapshots, not guest-owned state. The
 * original setter reuses/overlaps these locals after Create has consumed them. */
static bool aliases_headers(uint32_t address,uint32_t bytes,uint32_t internal)
{
#ifdef TSFP_RUST_AUDIO_HOST
    if(tsfp_audio_stream_create_fixed_overlap_rust(address,bytes,internal,false))return true;
#else
    if(overlap(address,bytes,internal,44u) || overlap(address,bytes,SINGLETON,4u) ||
       overlap(address,bytes,GLOBAL_STATE,4u) || overlap(address,bytes,SECONDARY,60u))return true;
#endif
#ifdef TSFP_RUST_AUDIO_HOST
    if(tsfp_audio_stream_lease_overlaps_guest_span_rust(address,bytes))return true;
#endif
#ifndef TSFP_RUST_AUDIO_HOST
    for(stream_node *n=streams;n!=NULL;n=n->next)
        if(overlap(address,bytes,n->value.stream_address,STREAM_BYTES) ||
           overlap(address,bytes,n->value.publication_address,4u))return true;
    for(stream_node *n=detached;n!=NULL;n=n->next)
        if(overlap(address,bytes,n->value.stream_address,STREAM_BYTES))return true;
#endif
#ifdef TSFP_RUST_AUDIO_HOST
    if(tsfp_audio_stream_create_untracked_overlaps_rust(address,bytes))return true;
#else
    for(stream_node *n=rollback;n!=NULL;n=n->next)
        if(n->value.stream_address!=0u && overlap(address,bytes,n->value.stream_address,STREAM_BYTES))return true;
#endif
    return false;
}
/* Rust owns protected-stream lifecycle membership and span projection. The C adapter
 * supplies only fixed global spans, while this call runs under the stream lock. */
#if defined(TSFP_RUST_AUDIO_HOST)
static bool routing_guest_spans_snapshot(uint32_t internal,dsound_stream_guest_span *spans,
                                         uint32_t capacity,uint32_t *count_out)
{
    if(count_out==NULL)return false;
    uint32_t stream_count=0u;
    if(!tsfp_audio_stream_lease_guest_spans_rust(NULL,0u,&stream_count) ||
       stream_count>UINT32_MAX-4u)return false;
    const uint32_t count=stream_count+4u;
    *count_out=count;
    if(spans==NULL)return capacity==0u;
    if(capacity<count)return false;
    spans[0]=(dsound_stream_guest_span){internal,44u};
    spans[1]=(dsound_stream_guest_span){SINGLETON,4u};
    spans[2]=(dsound_stream_guest_span){GLOBAL_STATE,4u};
    spans[3]=(dsound_stream_guest_span){SECONDARY,60u};
    uint32_t written=0u;
    return tsfp_audio_stream_lease_guest_spans_rust(spans+4u,capacity-4u,&written) &&
        written==stream_count && written+4u==count;
}
#endif
/* Unordered safety facts, NOT a copy of original ordered vtable bytes. Every
 * member has a verified unconditional T161 boundary in the embedding host. */
#ifndef TSFP_RUST_AUDIO_HOST
static bool safe_tables(const uint32_t table[15])
{
    static const uint32_t stopped[14]={0x00384859u,0x0040686Cu,0x00406879u,0x0040688Au,
        0x0040723Fu,0x00407286u,0x004072D4u,0x0040733Bu,0x00407388u,0x004073D3u,
        0x00407424u,0x00407883u,0x0040788Du,0x004093ADu};
    for(unsigned i=0u;i<15u;i++) {
        bool found=false;for(unsigned j=0u;j<14u;j++)if(table[i]==stopped[j])found=true;
        if(!found)return false;
    }
    return true;
}
#endif
bool dsound_stream_original_tables_stopped(void)
{
#ifdef TSFP_RUST_AUDIO_HOST
    /* Rust owns guest reads and table validation on the Rust audio host. */
    return tsfp_audio_stream_create_tables_stopped_rust(kernel_host_pid());
#else
    uint32_t table[15];
    return kernel_guest_read_bytes(SECONDARY,table,sizeof(table)) && safe_tables(table);
#endif
}
typedef struct create_finalize_context {
    int32_t host_pid;
    uint32_t stream_address,output_address,routing_reserved,failures,announce;
} create_finalize_context;
_Static_assert(sizeof(create_finalize_context)==24u,"Rust stream-create finalize ABI layout changed");
typedef struct create_request {
#ifdef TSFP_RUST_AUDIO_HOST
    create_finalize_context finalize;
#endif
    dsound_stream_scope scope;
    uint32_t output,status;
#ifndef TSFP_RUST_AUDIO_HOST
#endif
    stream_node *node;
    const char *error;
    bool announce;
    bool routing_reserved;
#ifdef TSFP_RUST_AUDIO_HOST
    bool lease_reserved;
#endif
} create_request;
#ifdef TSFP_RUST_AUDIO_HOST
_Static_assert(sizeof(create_request)==128u,"Rust stream-create prepare callback ABI layout changed");
#endif
#ifndef TSFP_RUST_AUDIO_HOST
static bool create_prepare(const dsound_device_lease *candidate,void *userdata,
                           dsound_device_lease_child *child)
{
    create_request *r=userdata;pthread_mutex_lock(&lock);
    r->error=policy_error();if(r->error!=NULL)return false;
#ifdef TSFP_RUST_AUDIO_HOST
    r->node=calloc(1u,sizeof(*r->node));
    if(r->node==NULL){r->status=OOM;return false;}
    tsfp_audio_stream_create_prepare_result result={0};
    const uint32_t outcome=tsfp_audio_stream_create_prepare_rust(
        kernel_host_pid(),candidate,&r->scope,r->output,&result);
    r->node->value.stream_heap=result.stream_heap;
    r->node->value.stream_address=result.stream_address;
    r->lease_reserved=result.lease_reserved!=0u;
    r->routing_reserved=result.routing_reserved!=0u;
    r->finalize.host_pid=kernel_host_pid();
    r->finalize.stream_address=result.stream_address;
    r->finalize.output_address=r->output;
    r->finalize.routing_reserved=r->routing_reserved?1u:0u;
    if(outcome!=0u) {
        if(outcome==4u)r->status=OOM;
        switch(outcome) {
            case 1u:r->error="original vtable unreadable or contains an unprotected target";break;
            case 2u:r->error="unproven output/input alias with owned state";break;
            case 3u:r->error="output is not writable";break;
            case 4u:break;
            case 5u:r->error="new private stream allocation/alias is invalid";break;
            case 6u:r->error="new private stream allocation/alias is invalid";break;
            case 7u:r->error="Rust stream lease lifecycle reservation failed";break;
            case 8u:r->error="Rust routing sidecar reservation failed";break;
            default:r->error="Rust stream-create preparation refused";break;
        }
        return false;
    }
    *child=(dsound_device_lease_child){result.stream_heap,result.stream_address,STREAM_BYTES};
    return true;
#endif
#ifdef TSFP_RUST_AUDIO_HOST
    if(!tsfp_audio_stream_create_tables_stopped_rust(kernel_host_pid())) {
        r->error="original vtable unreadable or contains an unprotected target";return false;
    }
#else
    uint32_t table[15];
    if(!kernel_guest_read_bytes(SECONDARY,table,sizeof(table))) {
        r->error="original vtable unreadable";return false;
    }
    if(!safe_tables(table)){r->error="original vtable contains an unprotected target";return false;}
#endif
    bool output_alias;
#ifdef TSFP_RUST_AUDIO_HOST
    output_alias=tsfp_audio_stream_create_output_overlap_rust(r->output,candidate->internal_address,
        r->scope.descriptor_address,r->scope.format_address);
#else
    output_alias=overlap(r->output,4u,r->scope.descriptor_address,24u) ||
        overlap(r->output,4u,r->scope.format_address,20u) ||
        overlap(r->scope.descriptor_address,24u,candidate->internal_address,44u) ||
        overlap(r->scope.format_address,20u,candidate->internal_address,44u);
#endif
    if(aliases_state(r->output,4u,candidate->internal_address) || output_alias) {
        r->error="unproven output/input alias with owned state";return false;
    }
    /* Same-byte probe preserves contents, but is explicitly a guest write. */
#ifdef TSFP_RUST_AUDIO_HOST
    if(!tsfp_audio_stream_create_probe_output_rust(kernel_host_pid(),r->output)) {
#else
    uint32_t old;
    if(!kernel_guest_read_u32(r->output,&old) || !kernel_guest_write_u32(r->output,old) ||
       kernel_guest_at_ro(r->output,4u)==NULL) {
#endif
        r->error="output is not writable";return false;
    }
    r->node=calloc(1u,sizeof(*r->node));if(r->node==NULL){r->status=OOM;return false;}
#ifndef TSFP_RUST_AUDIO_HOST
    r->node->value.publication_address=r->output;
#endif
#ifndef TSFP_RUST_AUDIO_HOST
    r->node->value.scope=r->scope;
    r->node->value.source_rate_hz=r->scope.sample_rate;
    dsound_stream_routing_init(&r->node->value.routing);
#endif
#ifdef TSFP_RUST_AUDIO_HOST
    uint32_t heap=0u,address=0u;
    if(!tsfp_audio_stream_heap_allocate_rust(STREAM_BYTES,&heap,&address)) {
        r->status=OOM;return false;
    }
#else
    uint32_t heap=dsound_audio_heap_create(0u,GUEST_HEAP_CHUNK_MIN,0u);
    uint32_t address=heap==0u?0u:dsound_audio_heap_alloc(heap,STREAM_BYTES);
#endif
    r->node->value.stream_heap=heap;
    if(heap==0u){r->status=OOM;return false;}
    r->node->value.stream_address=address;
    if(address==0u){r->status=OOM;return false;}
#ifdef TSFP_RUST_AUDIO_HOST
#else
    uint32_t *header=r->node->value.header;
    header[0]=VTABLE;header[1]=SECONDARY;header[2]=1u;header[3]=candidate->internal_address;
#endif
    bool invalid_allocation=aliases_state(address,STREAM_BYTES,candidate->internal_address);
#ifdef TSFP_RUST_AUDIO_HOST
    invalid_allocation=invalid_allocation ||
        tsfp_audio_stream_create_allocation_overlap_rust(address,r->output,
            r->scope.descriptor_address,r->scope.format_address);
#else
    invalid_allocation=invalid_allocation || overlap(address,STREAM_BYTES,r->output,4u) ||
       overlap(address,STREAM_BYTES,r->scope.descriptor_address,24u) ||
       overlap(address,STREAM_BYTES,r->scope.format_address,20u);
#endif
    if(invalid_allocation) {
        r->error="new private stream allocation/alias is invalid";return false;
    }
#ifdef TSFP_RUST_AUDIO_HOST
    const bool header_written=tsfp_audio_stream_create_write_header_rust(
        kernel_host_pid(),address,candidate->internal_address);
#else
    const bool header_written=kernel_guest_write_bytes(address,header,STREAM_BYTES);
#endif
    if(!header_written) {
        r->error="new private stream allocation/alias is invalid";return false;
    }
#ifdef TSFP_RUST_AUDIO_HOST
    if(!tsfp_audio_stream_create_reserve_rust(address,heap,r->output,
            candidate->internal_address,&r->scope)) {
        r->error="Rust stream lease lifecycle reservation failed";return false;
    }
    r->lease_reserved=true;
#endif
#ifdef TSFP_RUST_STREAM_FREQUENCY_STATE
    if(!tsfp_audio_stream_routing_prepare_rust(address)) {
        r->error="Rust routing sidecar reservation failed";return false;
    }
    r->routing_reserved=true;
#endif
    *child=(dsound_device_lease_child){heap,address,STREAM_BYTES};return true;
}
#endif
#ifdef TSFP_RUST_AUDIO_HOST
extern void tsfp_audio_stream_create_abort_unlock_rust(void *userdata);
#define CREATE_PREPARE_CALLBACK tsfp_audio_stream_create_prepare_callback_rust
#define CREATE_ABORT_CALLBACK tsfp_audio_stream_create_abort_unlock_rust
#define CREATE_FINALIZE_CALLBACK tsfp_audio_stream_create_finalize_callback_rust
#else
#define CREATE_PREPARE_CALLBACK create_prepare
static void create_abort(void *userdata)
{
    (void)userdata;
    /* Device lease callbacks run under the device lock. Retain the unpublished
     * candidate until acquire_lease returns; cleanup and fatal must run afterward. */
    pthread_mutex_unlock(&lock);
}
#define CREATE_ABORT_CALLBACK create_abort
#define CREATE_FINALIZE_CALLBACK create_finalize
#endif
#ifdef TSFP_RUST_AUDIO_HOST
static bool stream_heap_destroy(uint32_t heap)
{ return tsfp_audio_stream_heap_destroy_rust(heap); }
#else
static bool stream_heap_valid(uint32_t heap)
{ return dsound_audio_heap_valid(heap); }
static bool stream_heap_destroy(uint32_t heap)
{ return dsound_audio_heap_destroy(heap); }
#endif
static void cleanup_aborted_create(create_request *r)
{
    if(r->node==NULL)return;
#ifdef TSFP_RUST_AUDIO_HOST
    tsfp_audio_stream_create_cleanup_aborted_rust(r);
#else
    pthread_mutex_lock(&lock);
    const uint32_t heap=r->node->value.stream_heap;
    const bool cleaned=!stream_heap_valid(heap) || stream_heap_destroy(heap);
    if(cleaned) {
        free(r->node);
    } else {
        /* No committed lease/token exists. The generation-bound exclusive
         * private heap may have no header, or a partially written header. */
        r->node->next=rollback;rollback=r->node;
        r->error="private allocation rollback failed; unleased cleanup retained";
    }
    r->node=NULL;pthread_mutex_unlock(&lock);
#endif
}
#ifndef TSFP_RUST_AUDIO_HOST
static void create_finalize(const dsound_device_lease *committed,void *userdata)
{
    create_request *r=userdata;
    /* acquire_lease copies its caller output token AFTER this callback. Store
     * the COMMITTED token here before making the sidecar or guest pointer visible. */
#ifdef TSFP_RUST_STREAM_FREQUENCY_STATE
    if(!tsfp_audio_stream_routing_bind_rust(r->node->value.stream_address,committed->serial))
        dsound_hle_log()("dsound: Rust stream routing lease bind invariant failed stream=%#x serial=%llu\n",
            r->node->value.stream_address,(unsigned long long)committed->serial);
#endif
    r->node->value.lease=*committed;
    r->node->next=streams;streams=r->node;
    r->announce=!announced;announced=true;
    (void)kernel_guest_write_u32(r->output,r->node->value.stream_address);
    pthread_mutex_unlock(&lock);
}
#endif
static const dsound_device_lease_ops create_ops={CREATE_PREPARE_CALLBACK,CREATE_ABORT_CALLBACK,CREATE_FINALIZE_CALLBACK};
#ifdef TSFP_RUST_AUDIO_HOST
/* Host-only edges for the Rust-owned frame-to-lease transaction. The callbacks
 * remain synchronous so their stack request and lock lifetime match the oracle. */
void tsfp_audio_stream_create_acquire_host_c(uint32_t device,void *request,
    uint32_t *status,dsound_device_lease *lease)
{
    if(status==NULL || lease==NULL)return;
    *status=(uint32_t)dsound_device_acquire_lease(device,&create_ops,request,lease);
}
void tsfp_audio_stream_create_cleanup_host_c(void *request)
{cleanup_aborted_create((create_request *)request);}
void tsfp_audio_stream_create_report_host_c(const void *request,
    const dsound_device_lease *lease,uint32_t status)
{
    const create_request *r=(const create_request *)request;
    if(r==NULL || lease==NULL)return;
    if((r->finalize.failures&1u)!=0u)
        dsound_hle_log()("dsound: Rust stream lease bind invariant failed stream=%#x serial=%llu\n",
            r->finalize.stream_address,(unsigned long long)lease->serial);
    if((r->finalize.failures&2u)!=0u)
        dsound_hle_log()("dsound: Rust stream routing lease bind invariant failed stream=%#x serial=%llu\n",
            r->finalize.stream_address,(unsigned long long)lease->serial);
    if((r->finalize.failures&4u)!=0u)
        dsound_hle_log()("dsound: Rust stream output publication failed after lease commit stream=%#x\n",
            r->finalize.stream_address);
    if(status==(uint32_t)DSOUND_LEASE_OK && r->announce)
        dsound_hle_log()("dsound: explicit HEADLESS streams; passive40-byte public headers and "
            "host startup setter caches; omitted settings/hardware/list/packet objects, APU/DSP/FP/critical-section "
            "operations; no audio output or packet completion\n");
}
void tsfp_audio_stream_create_refuse_host_c(const char *reason)
{refuse(CREATE,reason!=NULL?reason:"device lease transaction refused");}
#endif
uint32_t dsound_stream_create(uint32_t descriptor,uint32_t output)
{
    create_request r={0};r.output=output;
#ifdef TSFP_RUST_AUDIO_HOST
    r.finalize.host_pid=(int32_t)kernel_host_pid();
#endif
    pthread_mutex_lock(&lock);const char *error=policy_error();pthread_mutex_unlock(&lock);
    if(error!=NULL)refuse(CREATE,error);
    if(!dsound_stream_scope_snapshot(descriptor,&r.scope))refuse(CREATE,"descriptor/format outside measured startup scope");
    uint32_t device;
#ifdef TSFP_RUST_AUDIO_HOST
    if(!tsfp_audio_stream_device_singleton_rust(kernel_host_pid(),&device) ||
       device==0u || device>UINT32_MAX-8u)
#else
    if(!kernel_guest_read_u32(SINGLETON,&device) || device==0u || device>UINT32_MAX-8u)
#endif
        refuse(CREATE,"no owned SILENT device");
    dsound_device_lease lease;
    dsound_device_lease_status status=dsound_device_acquire_lease(device+8u,&create_ops,&r,&lease);
#ifdef TSFP_RUST_AUDIO_HOST
    r.announce=r.finalize.announce!=0u;
    if((r.finalize.failures&1u)!=0u)
        dsound_hle_log()("dsound: Rust stream lease bind invariant failed stream=%#x serial=%llu\n",
            r.finalize.stream_address,(unsigned long long)lease.serial);
    if((r.finalize.failures&2u)!=0u)
        dsound_hle_log()("dsound: Rust stream routing lease bind invariant failed stream=%#x serial=%llu\n",
            r.finalize.stream_address,(unsigned long long)lease.serial);
    if((r.finalize.failures&4u)!=0u)
        dsound_hle_log()("dsound: Rust stream output publication failed after lease commit stream=%#x\n",
            r.finalize.stream_address);
#endif
    if(status!=DSOUND_LEASE_OK)cleanup_aborted_create(&r);
#ifdef TSFP_RUST_AUDIO_HOST
    /* The committed lifecycle, address, and heap now live only in Rust. */
    tsfp_audio_stream_create_release_request_rust(r.node);r.node=NULL;
    const uint32_t outcome=tsfp_audio_stream_create_outcome_rust(
        (uint32_t)status,r.status,r.error!=NULL?1u:0u);
    if(outcome==CREATE_OUTCOME_OOM)return OOM;
    if(outcome!=CREATE_OUTCOME_READY)
        refuse(CREATE,r.error!=NULL?r.error:"device lease transaction refused");
#else
    if(status==DSOUND_LEASE_OUT_OF_MEMORY || (status==DSOUND_LEASE_PREPARE_FAILED && r.status==OOM && r.error==NULL))return OOM;
    if(status!=DSOUND_LEASE_OK)refuse(CREATE,r.error!=NULL?r.error:"device lease transaction refused");
#endif
    if(r.announce)dsound_hle_log()("dsound: explicit HEADLESS streams; passive40-byte public headers and "
        "host startup setter caches; omitted settings/hardware/list/packet objects, APU/DSP/FP/critical-section "
        "operations; no audio output or packet completion\n");
    return 0u;
}
typedef struct access_request {
    uint32_t address,entry,argument,apply,count,position_z;
    dsound_device_lease identity;
    dsound_stream_snapshot *snapshot;
    const char *error;
    bool observer;
} access_request;
static bool identify(uint32_t address,dsound_device_lease *identity)
{
#ifdef TSFP_RUST_AUDIO_HOST
    return tsfp_audio_stream_lease_identity_rust(address,identity);
#else
    pthread_mutex_lock(&lock);stream_node *n=find(address);if(n!=NULL)*identity=n->value.lease;
    pthread_mutex_unlock(&lock);return n!=NULL;
#endif
}
static void access_owned(uint32_t internal,void *userdata,uint32_t *result)
{
    access_request *r=userdata;pthread_mutex_lock(&lock);
#ifdef TSFP_RUST_AUDIO_HOST
    stream_node key={0};key.value.stream_address=r->address;stream_node *n=&key;
#else
    stream_node *n=find(r->address);
#endif
    if(!valid_node(n,&r->identity,internal)){r->error="stream ownership/header/generation changed";goto done;}
#ifdef TSFP_RUST_AUDIO_HOST
    if(r->observer){
        if(!tsfp_audio_stream_observe_rust(r->address,r->snapshot,&r->error))goto done;
        *result=0u;goto done;
    }
#endif
#ifdef TSFP_RUST_AUDIO_HOST
    dsound_device_lease rust_lease;
    if(!tsfp_audio_stream_lease_identity_rust(r->address,&rust_lease)) {
        r->error="Rust stream lease identity is unavailable";goto done;
    }
    const dsound_device_lease *active_lease=&rust_lease;
#else
    const dsound_device_lease *active_lease=&n->value.lease;
#endif
    dsound_stream_scope scope_storage={0};
#ifdef TSFP_RUST_AUDIO_HOST
    if(!tsfp_audio_stream_lease_scope_rust(r->address,active_lease,&scope_storage)) {
        r->error="Rust stream scope is unavailable for this lease";goto done;
    }
#else
    scope_storage=n->value.scope;
#endif
    const dsound_stream_scope *scope=&scope_storage;
#ifdef TSFP_RUST_AUDIO_HOST
    dsound_stream_format_state format_state={0};
    if(!tsfp_audio_stream_lease_format_state_rust(r->address,active_lease,&format_state)) {
        r->error="Rust stream format sidecar is unavailable for this lease";goto done;
    }
#else
    const uint32_t format_sets=n->value.format_sets;
#endif
#ifdef TSFP_RUST_STREAM_FREQUENCY_STATE
    uint32_t format_rate=scope->sample_rate;
#ifdef TSFP_RUST_AUDIO_HOST
    if(format_state.sets!=0u)memcpy(&format_rate,format_state.bytes+4u,sizeof(format_rate));
#else
    if(format_sets!=0u)memcpy(&format_rate,n->value.format+4u,sizeof(format_rate));
#endif
#endif
#ifdef TSFP_RUST_AUDIO_HOST
    dsound_stream_volume_state volume_state={0};
    if(!tsfp_audio_stream_lease_volume_state_rust(r->address,active_lease,&volume_state)) {
        r->error="Rust stream volume sidecar is unavailable for this lease";goto done;
    }
    const bool volume_seen=volume_state.seen!=0u;
    const int32_t volume_value=volume_state.value;
    dsound_stream_spatial_state spatial_state={0};
    if(!tsfp_audio_stream_lease_spatial_state_rust(r->address,active_lease,&spatial_state)) {
        r->error="Rust stream spatial sidecar is unavailable for this lease";goto done;
    }
    dsound_stream_control_state control_state={0};
    if(!tsfp_audio_stream_lease_control_state_rust(r->address,active_lease,&control_state)) {
        r->error="Rust stream control sidecar is unavailable for this lease";goto done;
    }
    uint32_t cache_mask=spatial_state.startup_cache_mask;
    const bool pause_seen=control_state.pause_seen!=0u;
    const bool flush_seen=control_state.flush_seen!=0u;
    const bool discontinuity_seen=control_state.discontinuity_seen!=0u;
#else
    const bool volume_seen=n->value.volume_seen;
    const int32_t volume_value=n->value.volume;
    uint32_t cache_mask=n->value.cache_mask;
    const bool pause_seen=n->value.pause_seen;
    const uint32_t pause_mode=n->value.pause_mode;
    const bool flush_seen=n->value.flush_seen;
    const uint32_t flush_time_low=n->value.flush_time_low;
    const uint32_t flush_time_high=n->value.flush_time_high;
    const uint32_t flush_flags=n->value.flush_flags;
    const bool discontinuity_seen=n->value.discontinuity_seen;
#endif
#ifndef TSFP_RUST_AUDIO_HOST
    if(r->observer){
#ifdef TSFP_RUST_AUDIO_HOST
        memset(r->snapshot,0,sizeof(*r->snapshot));
        r->snapshot->lease=*active_lease;
        r->snapshot->stream_address=n->value.stream_address;
        r->snapshot->stream_heap=n->value.stream_heap;
        if(!tsfp_audio_stream_lease_header_rust(
               r->address,active_lease,r->snapshot->header)) {
            r->error="Rust stream header snapshot is unavailable for this lease";goto done;
        }
        if(!tsfp_audio_stream_lease_publication_address_rust(
               r->address,&r->snapshot->publication_address)) {
            r->error="Rust stream publication address is unavailable";goto done;
        }
#else
        *r->snapshot=n->value;
#endif
        r->snapshot->scope=scope_storage;
#ifdef TSFP_RUST_AUDIO_HOST
        tsfp_audio_stream_snapshot_project_rust(r->snapshot,&format_state,&volume_state,
                                                &spatial_state,&control_state);
#endif
#ifdef TSFP_RUST_STREAM_FREQUENCY_STATE
        dsound_stream_frequency_state frequency={0};
        if(!tsfp_audio_stream_frequency_state_rust(r->address,active_lease->serial,&frequency)) {
            r->error="Rust frequency sidecar is unavailable for this stream generation";goto done;
        }
        r->snapshot->frequency_sets=frequency.accepted_sets;
        r->snapshot->source_rate_hz=frequency.accepted_sets==0u || frequency.current_hz==0u?
            format_rate:frequency.current_hz;
        r->snapshot->frequency_pitch=frequency.pitch;
#endif
#ifdef TSFP_RUST_AUDIO_HOST
        if(!tsfp_audio_stream_routing_load_rust(r->address,active_lease->serial,
                                                &r->snapshot->routing)) {
            r->error="Rust routing sidecar is unavailable for this lease";goto done;
        }
#endif
        *result=0u;goto done;
    }
#endif
    r->error=policy_error();if(r->error!=NULL)goto done;
    if(r->entry==SET_FREQUENCY) {
#ifdef TSFP_RUST_AUDIO_HOST
        dsound_stream_frequency_note_fn note=frequency_note;
        const uint32_t frequency_flags=(volume_seen?1u:0u)|(discontinuity_seen?2u:0u)|
            (completion?4u:0u)|(frequency_note!=NULL?8u:0u);
        if(!tsfp_audio_stream_set_frequency_rust(r->address,active_lease->serial,scope->flags,
               scope->channels,scope->sample_rate,format_state.sets,format_state.bytes,cache_mask,
               frequency_flags,r->argument,r->count,&note,frequency_apply_rust,&r->error))goto done;
        accepted_frequency_sets++;
        *result=0u;goto done;
#else
#ifdef TSFP_RUST_STREAM_FREQUENCY_STATE
        dsound_stream_frequency_state frequency={0};
        if(!tsfp_audio_stream_frequency_state_rust(r->address,active_lease->serial,&frequency)) {
            r->error="Rust frequency sidecar is unavailable for this lease";goto done;
        }
        const uint32_t old_hz=frequency.accepted_sets==0u || frequency.current_hz==0u?
            format_rate:frequency.current_hz;
#else
        const uint32_t old_hz=n->value.source_rate_hz;
#endif
        const bool stereo=scope->flags==0u && scope->channels==2u && cache_mask==0u;
        const bool spatial=scope->flags==0x10u && scope->channels==1u && cache_mask==7u;
#ifdef TSFP_RUST_AUDIO_HOST
        const bool frequency_admitted=tsfp_audio_stream_frequency_admit_rust(
            stereo?1u:0u,spatial?1u:0u,completion?1u:0u,frequency_note!=NULL?1u:0u,
            volume_seen?1u:0u,discontinuity_seen?1u:0u);
#else
        const bool frequency_admitted=completion && frequency_note!=NULL &&
            (stereo || spatial) && volume_seen && discontinuity_seen;
#endif
        if(!frequency_admitted) {
            r->error="frequency needs a started stereo or fully configured mono PCM stream";goto done;
        }
        uint32_t hertz=r->argument;
        if(hertz==0u) {
            hertz=scope->sample_rate;
#ifdef TSFP_RUST_AUDIO_HOST
            if(format_state.sets!=0u)memcpy(&hertz,format_state.bytes+4u,sizeof(hertz));
#else
            if(format_sets!=0u)memcpy(&hertz,n->value.format+4u,sizeof(hertz));
#endif
        }
        int32_t pitch;
#ifdef TSFP_RUST_STREAM_FREQUENCY_STATE
        const bool pitch_valid=tsfp_audio_stream_frequency_pitch_rust(hertz,(uint16_t)r->count,&pitch);
        if(!pitch_valid) {
            r->error="frequency or guest control word outside measured domain";goto done;
        }
        dsound_stream_frequency_note_fn note=frequency_note;
        const bool accepted=frequency.accepted_sets!=UINT32_MAX &&
            tsfp_audio_stream_frequency_commit_rust(r->address,active_lease->serial,
                frequency.accepted_sets,old_hz,hertz,(uint16_t)r->count,
                &note,frequency_apply_rust,&pitch);
#else
        const bool pitch_valid=dsound_stream_frequency_pitch(hertz,(uint16_t)r->count,&pitch);
        const bool accepted=pitch_valid && n->value.frequency_sets!=UINT32_MAX &&
            frequency_note(r->address,active_lease->serial,old_hz,hertz);
#endif
        if(!accepted) {
            if(!pitch_valid) {
                r->error="frequency or guest control word outside measured domain";goto done;
            }
            r->error="frequency PCM/completion transaction refused";goto done;
        }
#ifndef TSFP_RUST_STREAM_FREQUENCY_STATE
        n->value.source_rate_hz=hertz;
        n->value.frequency_pitch=pitch;
        n->value.frequency_sets++;
#endif
        accepted_frequency_sets++;
#endif
        *result=0u;goto done;
    }
    if (r->entry == HEADROOM || r->entry == MIX_BINS || r->entry == MIX_VOLUMES) {
#ifdef TSFP_RUST_AUDIO_HOST
        const uint32_t completion_available=completion && routing_note!=NULL?1u:0u;
        if(tsfp_audio_stream_routing_scope_admit_rust(
            r->address,active_lease,completion_available)!=0u) {
            r->error="real PCM routing requires admitted nonspatial stereo stream and completion policy";
            goto done;
        }
#else
        if (!completion || routing_note == NULL || scope->flags != 0u ||
            scope->channels != 2u) {
            r->error="real PCM routing requires admitted nonspatial stereo stream and completion policy";
            goto done;
        }
#endif
#ifdef TSFP_RUST_AUDIO_HOST
        dsound_stream_routing_note_fn note=routing_note;
        dsound_stream_routing_apply_fn apply=tsfp_audio_stream_routing_apply_note_rust;
        uint32_t operation=DSOUND_STREAM_ROUTING_UPDATE_HEADROOM;
        if(r->entry==MIX_BINS)operation=DSOUND_STREAM_ROUTING_UPDATE_BINS;
        else if(r->entry==MIX_VOLUMES)operation=DSOUND_STREAM_ROUTING_UPDATE_VOLUMES;
        const uint32_t route_status=tsfp_audio_stream_routing_update_rust(
            r->address,active_lease->serial,operation,r->argument,kernel_host_pid(),
            r->argument==0u?0u:r->argument,internal,&note,apply,routing_guest_spans_snapshot);
        if(route_status!=0u) {
            r->error=route_status==3u?
                "PCM routing rejected clock/history/lifetime or inactive renderer":
                route_status==2u?
                    "mix-bin state needs unsupported DSP/HRTF route or invalid index":
                    "mix-bin descriptor or pairs outside checked bounds or aliases owned state";
            goto done;
        }
        *result=0u;goto done;
#else
        dsound_stream_routing candidate;
        if(!load_routing(r->address,active_lease->serial,&n->value.routing,&candidate)) {
            r->error="stream routing sidecar is unavailable for this lease";goto done;
        }
        bool admitted=true;
        if (r->entry == HEADROOM) dsound_stream_routing_headroom(&candidate,r->argument);
        else if (r->entry == MIX_BINS && r->argument == 0u)
            admitted=dsound_stream_routing_bins(&candidate,NULL,0u,true);
        else {
#if defined(TSFP_RUST_AUDIO_HOST)
            const uint32_t maximum=r->entry==MIX_BINS?DSOUND_STREAM_ROUTE_SLOTS:DSOUND_STREAM_ROUTE_BINS;
            if (r->argument == 0u) {
                r->error="mix-bin descriptor is null";goto done;
            }
            const uint32_t route_status=tsfp_audio_stream_routing_guest_descriptor_rust(
                (int32_t)kernel_host_pid(),&candidate,r->argument,maximum,
                r->entry==MIX_VOLUMES,internal,routing_guest_spans_snapshot);
            if (route_status==1u) {
                r->error="mix-bin descriptor or pairs outside checked bounds or aliases owned state";goto done;
            }
            admitted=route_status==0u;
#else
            uint32_t descriptor[2]={0u,0u};
            if (r->argument == 0u || aliases_headers(r->argument,4u,internal) ||
                !kernel_guest_read_u32(r->argument,&descriptor[0]) ||
                (descriptor[0]!=0u &&
                 ((uint64_t)r->argument+8u>UINT64_C(0x100000000) ||
                  aliases_headers(r->argument+4u,4u,internal) ||
                  !kernel_guest_read_u32(r->argument+4u,&descriptor[1])))) {
                r->error="mix-bin descriptor inaccessible or aliases owned state";goto done;
            }
            const uint32_t maximum=r->entry==MIX_BINS?DSOUND_STREAM_ROUTE_SLOTS:DSOUND_STREAM_ROUTE_BINS;
            dsound_stream_route_pair pairs[DSOUND_STREAM_ROUTE_BINS];
            if (descriptor[0] > maximum || (descriptor[0] != 0u &&
                (descriptor[1]==0u || (uint64_t)descriptor[1]+descriptor[0]*8u>UINT64_C(0x100000000) ||
                 aliases_headers(descriptor[1],descriptor[0]*8u,internal) ||
                 !kernel_guest_read_bytes(descriptor[1],pairs,descriptor[0]*8u)))) {
                r->error="mix-bin pairs outside checked bounds or aliases owned state";goto done;
            }
            admitted=r->entry==MIX_BINS?
                dsound_stream_routing_bins(&candidate,pairs,descriptor[0],false):
                dsound_stream_routing_volumes(&candidate,pairs,descriptor[0]);
#endif
        }
        if (!admitted) {r->error="mix-bin state needs unsupported DSP/HRTF route or invalid index";goto done;}
        if (!commit_routing(r->address,active_lease->serial,&n->value.routing,&candidate,true)) {
            r->error="PCM routing rejected clock/history/lifetime or inactive renderer";goto done;
        }
        *result=0u;goto done;
#endif
    }
    if (r->entry == STATUS) {
        uint32_t old;
#ifdef TSFP_RUST_AUDIO_HOST
        bool admitted = tsfp_audio_stream_status_admit_rust(scope->flags,cache_mask,scope->sample_rate,
            scope->channels,scope->block_align,scope->average_bytes_per_second,scope->max_packets,
            volume_seen,volume_value,pause_seen,flush_seen,discontinuity_seen);
#else
        bool spatial = scope->flags == 0x10u && cache_mask == 7u;
        bool stereo = scope->flags == 0u && cache_mask == 0u &&
            scope->sample_rate == 44100u && scope->channels == 2u &&
            scope->block_align == 72u &&
            scope->average_bytes_per_second == 49612u;
        /* T605: the title's stream update (sub_00029CC0, 0x29CEA) and sub_00029D10 (0x29D28) also ask every started stream:
         * volume -10000, Pause1, FlushEx and Discontinuity recorded (each implied by the recorded Discontinuity), the
         * stereo stream with or without its SetFormat and the spatial one. The original answers 1 for all of them
         * (bit 0 only: the free packet list is non-empty, no packet, voice flags 0x15), see docs/audio-input-recovery.md. */
        bool started = discontinuity_seen;
        bool fresh = !pause_seen && !flush_seen && !discontinuity_seen;
        bool admitted = !((!spatial && !stereo) || scope->max_packets != 3u ||
            !volume_seen || volume_value != -10000 || (!fresh && !started));
#endif
        if (!admitted) {
            r->error = "only exact spatial or fresh stereo silence startup status or the status of a started stream is supported";
            goto done;
        }
        if (r->argument == 0u || (uint64_t)r->argument + 4u > UINT64_C(0x100000000) ||
            aliases_headers(r->argument,4u,internal) || overlap(r->argument,4u,CURVE,16u) ||
            !kernel_guest_read_u32(r->argument,&old) ||
            !kernel_guest_write_u32(r->argument,old) ||
            !kernel_guest_write_u32(r->argument,1u)) {
            r->error = "startup status output is inaccessible or aliases protected state";
            goto done;
        }
        /* Exact reached startup response only. Original derives bit0 from its
         * non-self B0 list; no native packet admission exists. This is not a
         * general playback/readiness query; future packet support must revisit. */
        *result = 0u;
        goto done;
    }
    if (r->entry == SET_FORMAT) {
        uint8_t words[DSOUND_STREAM_FORMAT_BYTES] = {0};
#ifdef TSFP_RUST_AUDIO_HOST
        const uint32_t format_shape=tsfp_audio_stream_format_admit_rust(
            scope->flags,cache_mask,volume_seen?1u:0u,discontinuity_seen?1u:0u);
        if(format_shape==0u) {
            r->error="only the stereo startup scope or a configured spatial stream can change format";
            goto done;
        }
        if(format_shape==3u) {
            r->error="only a fully started (volume, Pause1, FlushEx, Discontinuity) stream can change format";
            goto done;
        }
        const bool stereo=format_shape==1u;
#else
        /* Keep the C-host oracle predicate spelled as its mutation anchors expect. */
        const bool stereo = scope->flags == 0u;
        const bool spatial = scope->flags == 0x10u && cache_mask == 7u;
        bool started = volume_seen && discontinuity_seen;
        if(!stereo && !spatial) {
            r->error="only the stereo startup scope or a configured spatial stream can change format";
            goto done;
        }
        if(!started) {
            r->error="only a fully started (volume, Pause1, FlushEx, Discontinuity) stream can change format";
            goto done;
        }
#endif
        if (aliases_headers(r->argument, sizeof(words), internal) ||
            overlap(r->argument, sizeof(words), CURVE, 16u)) {
            r->error = "SetFormat input aliases protected state";
            goto done;
        }
        /* An unreadable input leaves the zero fill, which is never an XADPCM format.
         * T1159, T1181: the original validates nothing, so the family is policy. XADPCM is tag 0x69, 4 bits, cbSize 2, 64 samples per
         * block, 36 bytes per channel per block, average bytes per second = rate * block / 64 (floored), a rate in the 8000 to 48000 Hz
         * range of the Xbox stream mixer. The channel count is the stream's own (stereo 2, spatial 1): a different count also changes
         * the voice format words (+0xC, +0x14, +0xEC, T602 oracle) the host record does not model. */
        const uint32_t channels = stereo ? 2u : 1u, block = 36u * channels;
        uint32_t rate=0u;
#ifdef TSFP_RUST_AUDIO_HOST
        (void)block;
        const bool family=tsfp_audio_stream_format_decode_rust((int32_t)kernel_host_pid(),r->argument,
            channels,words,&rate);
#else
        const bool readable=kernel_guest_read_bytes(r->argument,words,sizeof(words));
        uint32_t average;
        memcpy(&rate, words + 4, 4);
        memcpy(&average, words + 8, 4);
        const bool family = words[0] == 0x69u && words[1] == 0u && words[2] == channels && words[3] == 0u &&
            rate >= 8000u && rate <= 48000u && average == (uint32_t)((uint64_t)rate * block / 64u) &&
            words[12] == block && words[13] == 0u && words[14] == 4u && words[15] == 0u &&
            words[16] == 2u && words[17] == 0u && words[18] == 64u && words[19] == 0u;
#endif
#ifdef TSFP_RUST_AUDIO_HOST
        if (!family) {
#else
        if (!readable || !family) {
#endif
            r->error = "only an ADPCM format of the stream's channel count (block 36 per channel, 64 samples, average rate*block/64, 8000 to 48000 Hz) at a readable address is supported";
            goto done;
        }
        /* Original copies settings, resets pitch and programs the APU. The
         * optional PCM/completion abort transaction changes source time;
         * guest settings/voice writes and hardware remain omitted. */
#ifdef TSFP_RUST_STREAM_FREQUENCY_STATE
        dsound_stream_frequency_state frequency={0};
        if(!tsfp_audio_stream_frequency_state_rust(r->address,active_lease->serial,&frequency)) {
            r->error="Rust frequency sidecar is unavailable for this lease";goto done;
        }
        const uint32_t old_hz=frequency.accepted_sets==0u || frequency.current_hz==0u?
            format_rate:frequency.current_hz;
        const uint32_t frequency_sets=frequency.accepted_sets;
        int32_t format_pitch=frequency.pitch;
        if(frequency_sets!=0u || (completion && frequency_control_word!=NULL)) {
            uint16_t cw=0u;
            const bool known=frequency_control_word!=NULL && frequency_control_word(&cw);
            const bool pitch_valid=known && tsfp_audio_stream_frequency_format_commit_rust(
                r->address,active_lease->serial,frequency_sets,old_hz,
                rate,cw,r->argument,format_note!=NULL?&format_note:NULL,
                format_note!=NULL?frequency_format_apply_rust:NULL,&format_pitch);
            const bool format_accepted=pitch_valid;
#else
        const uint32_t old_hz=n->value.source_rate_hz;
        const uint32_t frequency_sets=n->value.frequency_sets;
        int32_t format_pitch=n->value.frequency_pitch;
        if(frequency_sets!=0u || (completion && format_note!=NULL)) {
            uint16_t cw=0u;
            const bool known=format_note!=NULL && frequency_control_word!=NULL && frequency_control_word(&cw);
            const bool pitch_valid=known && dsound_stream_frequency_pitch(rate,cw,&format_pitch);
            const bool format_accepted=pitch_valid &&
                format_note(r->address,active_lease->serial,old_hz,rate,r->argument);
#endif
            if(!format_accepted) {
                dsound_hle_log()("dsound: format stream=%#x old=%u new=%u cw=%#x known=%u pitch-valid=%u\n",
                    r->address,old_hz,rate,(unsigned)cw,known?1u:0u,pitch_valid?1u:0u);
                r->error="SetFormat PCM/completion abort/reset rejected ownership, clock, words or guest CW";goto done;
            }
        }
#ifdef TSFP_RUST_AUDIO_HOST
        if(!tsfp_audio_stream_lease_format_commit_rust(
                r->address,active_lease,&format_state,words)) {
            r->error="Rust stream format sidecar commit invariant failed";goto done;
        }
#else
        memcpy(n->value.format, words, sizeof(words));
        n->value.frequency_pitch=format_pitch;
        n->value.source_rate_hz=rate;
        n->value.format_sets++;
#endif
        *result = 0u;
        goto done;
    }
    if (r->entry == VOLUME) {
#ifdef TSFP_RUST_AUDIO_HOST
        dsound_stream_volume_request request={r->argument,r->count,completion?1u:0u};
        dsound_stream_volume_state next_volume={0};
        if(tsfp_audio_stream_lease_volume_apply_rust(
                r->address,active_lease,&request,&next_volume)!=0u) {
            r->error="Rust stream SetVolume policy refused this request";goto done;
        }
        const int32_t new_volume=next_volume.value;
        if (!commit_volume_routing(r->address,active_lease->serial,(uint32_t)new_volume,
                                   routing_note != NULL)) {
            r->error="stream PCM volume/routing transaction rejected";goto done;
        }
#else
        bool spatial = scope->flags == 0x10u && cache_mask == 7u;
        bool stereo = scope->flags == 0u && cache_mask == 0u &&
            scope->max_packets == 3u && scope->sample_rate == 44100u &&
            scope->channels == 2u && scope->block_align == 72u &&
            scope->average_bytes_per_second == 49612u;
        bool fresh = !pause_seen && !flush_seen && !discontinuity_seen;
        bool paused = stereo && pause_seen && pause_mode == 1u &&
            flush_seen && flush_time_low == 0u && flush_time_high == 0u && flush_flags == 1u &&
            discontinuity_seen;
        /* T681: with the completion model on, the title's per frame stream update (sub_00029F30, caller 0x29F68)
         * sets the volume it computes (-1137 and every other value), so a started stream takes ANY value and the
         * model records it. Only a stream that never started keeps the startup silence rule. */
        const bool update = completion && (spatial || stereo) && discontinuity_seen && volume_seen;
        if (!update && ((!spatial && !stereo) || (!fresh && !paused) || r->argument != 0xFFFFD8F0u)) {
            r->error = "only fresh or fully paused/flushed/discontinued startup silence is supported";
            goto done;
        }
        if (r->count == 1u && format_sets == 0u) {
            r->error = "SetVolume after SetFormat needs a recorded SetFormat";
            goto done;
        }
        const int32_t new_volume=update?(int32_t)r->argument:-10000;
        dsound_stream_routing candidate;
        if(!load_routing(r->address,active_lease->serial,&n->value.routing,&candidate)) {
            r->error="stream routing sidecar is unavailable for this lease";goto done;
        }
        dsound_stream_routing_volume(&candidate,(uint32_t)new_volume);
        if (!commit_routing(r->address,active_lease->serial,&n->value.routing,&candidate,
                            routing_note != NULL && (stereo || spatial))) {
            r->error="stream PCM volume/routing transaction rejected";goto done;
        }
#endif
        /* Original settings values are represented in host-owned state; real
         * PCM uses them under the explicit hook, APU writes remain omitted. */
#ifdef TSFP_RUST_AUDIO_HOST
        /* The stream lock serializes this exact-generation commit with the accepted
         * routing transaction and every observer or subsequent handler. */
        if(!tsfp_audio_stream_lease_volume_commit_rust(
                r->address,active_lease,&volume_state,&next_volume)) {
            r->error="Rust stream volume state changed during the serialized SetVolume transaction";
            goto done;
        }
        volume_state=next_volume;
#else
        n->value.volume_seen = true;
        n->value.volume = new_volume;
#endif
        *result = 0u;
        goto done;
    }
    if (r->entry == DISCONTINUITY) {
#ifdef TSFP_RUST_AUDIO_HOST
        const dsound_stream_control_request request={DSOUND_STREAM_CONTROL_DISCONTINUITY,
            r->argument,r->count,r->apply,completion,(pause_note != NULL)};
        if(tsfp_audio_stream_lease_control_apply_rust(r->address,active_lease,&request)!=0u) {
            r->error="Rust stream control policy refused Discontinuity";goto done;
        }
#else
        bool stereo = scope->flags == 0u && cache_mask == 0u;
        bool spatial = scope->flags == 0x10u && cache_mask == 7u;
        if ((!stereo && !spatial) ||
            !pause_seen || pause_mode > 1u || !flush_seen || flush_time_low != 0u ||
            flush_time_high != 0u || flush_flags != 1u) {
            static char detail[192];
            snprintf(detail, sizeof detail, "only empty paused/flushed startup Discontinuity is supported (flags %#x mask %u pause %u/%u flush %u time %u/%u flags %u seen %u)",
                     scope->flags, cache_mask, pause_seen, pause_mode, flush_seen,
                     flush_time_low, flush_time_high, flush_flags, discontinuity_seen);
            r->error = detail;
            goto done;
        }
#endif
        /* T1198: measured on the original (spatial and stereo startup streams): Discontinuity is S_OK in pause mode 0 and 1, with or
         * without queued packets, writes no APU register and leaves GetStatus unchanged (only a first call with packets sets one
         * byte of an omitted packet object). Repeated calls are no-ops.
         * HOST request observation only. Original 0x40733B examines omitted
         * voice queue state; this facade has no packet admission. No queue,
         * voice discontinuity flag, callback or completion is synthesized. */
#ifndef TSFP_RUST_AUDIO_HOST
        n->value.discontinuity_seen = true;
#endif
        *result = 0u;
        goto done;
    }
    if (r->entry == FLUSH_EX) {
#ifdef TSFP_RUST_AUDIO_HOST
        const dsound_stream_control_request request={DSOUND_STREAM_CONTROL_FLUSH_EX,
            r->argument,r->count,r->apply,completion,(pause_note != NULL)};
        if(tsfp_audio_stream_lease_control_apply_rust(r->address,active_lease,&request)!=0u) {
            r->error="Rust stream control policy refused FlushEx";goto done;
        }
#else
        bool stereo = scope->flags == 0u && cache_mask == 0u;
        bool spatial = scope->flags == 0x10u && cache_mask == 7u;
        if ((!stereo && !spatial) ||
            !pause_seen || pause_mode != 1u ||
            r->argument != 0u || r->count != 0u || r->apply != 1u) {
            r->error = "only empty paused startup FlushEx time0/flags1 is supported";
            goto done;
        }
#endif
        /* HOST request metadata only. No Process or packet-admission method is
         * supported by this facade; adopting one requires revisiting this empty
         * scope before FlushEx can continue. No timing, callbacks, voice flags,
         * hardware flush or packet completion is synthesized here. */
#ifndef TSFP_RUST_AUDIO_HOST
        n->value.flush_seen = true;
        n->value.flush_time_low = r->argument;
        n->value.flush_time_high = r->count;
        n->value.flush_flags = r->apply;
#endif
        *result = 0u;
        goto done;
    }
    if (r->entry == PAUSE) {
#ifdef TSFP_RUST_AUDIO_HOST
        const dsound_stream_control_request request={DSOUND_STREAM_CONTROL_PAUSE,
            r->argument,r->count,r->apply,completion,(pause_note != NULL)};
        if(tsfp_audio_stream_lease_control_apply_rust(r->address,active_lease,&request)!=0u) {
            r->error="Rust stream control policy refused Pause";goto done;
        }
#else
        /* Original 29EA0 reaches 29ED1 with mode1 on the live stream.
         * Its additional caller admission requires the completion-owned started state. */
        if (r->count == 2u && (!completion || pause_note == NULL || !discontinuity_seen || r->argument != 1u)) {
            r->error = "runtime Pause caller needs completion transport, a started stream and mode1";
            goto done;
        }
        bool stereo = scope->flags == 0u && cache_mask == 0u;
        bool spatial = scope->flags == 0x10u && cache_mask == 7u;
        /* T681: a started stream (Discontinuity recorded) can also be paused and resumed (mode 0) by the title. */
        const bool resume_ok = completion && discontinuity_seen && (r->argument == 0u || r->argument == 1u);
        if ((!stereo && !spatial) || (r->argument != 1u && !resume_ok)) {
            r->error = "only measured stereo or fully configured spatial startup Pause mode1 is supported";
            goto done;
        }
        if (r->count == 1u && format_sets == 0u) {
            r->error = "Pause after SetFormat needs a recorded SetFormat";
            goto done;
        }
#endif
        /* Host request observation only. Original wrapper reads stream+0x24
         * and changes the omitted voice flags through 0x40BF18/0x40D09A.
         * No voice, hardware, packet or playback state is synthesized here. */
#ifndef TSFP_RUST_AUDIO_HOST
        n->value.pause_seen = true;
        n->value.pause_mode = r->argument;
#endif
        if (pause_note != NULL) pause_note(r->address, r->argument);
        *result = 0u;
        goto done;
    }
    if(r->entry==MAX_DISTANCE) {
#ifndef TSFP_RUST_AUDIO_HOST
        if(scope->flags!=0x10u || cache_mask!=7u) {
            r->error="only a fully configured spatial startup stream may record SetMaxDistance";goto done;
        }
#endif
        if(r->apply!=1u) {
            r->error="SetMaxDistance apply=0 enters the unmodeled settings commit helper";goto done;
        }
        /* The original writes the exact float word to omitted settings +0x3c, then sets
         * dirty byte +2. It leaves the guest stream/device headers unchanged and the
         * title's two admitted callers both pass apply=1. This is a host observation only. */
#ifdef TSFP_RUST_AUDIO_HOST
        if(!tsfp_audio_stream_lease_max_distance_rust(
            r->address,active_lease,r->apply,r->argument)) {
            r->error="Rust stream lease refused the accepted SetMaxDistance";goto done;
        }
#endif
#ifndef TSFP_RUST_AUDIO_HOST
        n->value.max_distance_seen=true;n->value.max_distance_bits=r->argument;
#endif
        *result=0u;goto done;
    }
    if(r->entry==SET_POSITION) {
        /* T1182: the title's three callers pass apply=1. The original writes only the omitted settings object and its dirty byte in that
         * mode, apply=0 enters the commit helper 0x406E90 and the APU chain. Raw host record on a fully configured spatial stream. */
        if(scope->flags!=0x10u || cache_mask!=7u) {
            r->error="only a fully configured spatial startup stream may record SetPosition";goto done;
        }
        if(r->apply!=1u) {
            r->error="SetPosition apply=0 enters the unmodeled settings commit helper";goto done;
        }
#ifdef TSFP_RUST_AUDIO_HOST
        dsound_stream_spatial_state next=spatial_state;
        next.position_bits[0]=r->argument;next.position_bits[1]=r->count;
        next.position_bits[2]=r->position_z;next.position_sets++;
        next.revision=spatial_state.revision+1u;
        if(!tsfp_audio_stream_lease_spatial_commit_rust(
            r->address,active_lease,&spatial_state,&next)) {
            r->error="Rust stream spatial sidecar refused the accepted SetPosition";goto done;
        }
#endif
#ifndef TSFP_RUST_AUDIO_HOST
        n->value.position_bits[0]=r->argument;n->value.position_bits[1]=r->count;n->value.position_bits[2]=r->position_z;
        n->value.position_sets++;
#endif
        *result=0u;goto done;
    }
    if(r->entry==ROLLOFF && scope->flags==0x10u && cache_mask==7u && r->apply==0u && r->count==4u &&
       (r->argument==CURVE || r->argument==CURVE_NEAR || r->argument==CURVE_FAR)) {
        /* T1199: the sound update sub_00029E60 (returns 0x29E81, 0x29E90) switches a configured spatial stream between the three
         * 4 point curves. Measured on the original (spatial startup and started streams, paused or resumed): S_OK, a raw store of
         * the curve pointer, the count and the dirty byte in the omitted settings object, no APU write, no kernel call but the
         * critical section pair, public headers untouched. Host record of the pointer only. */
#ifdef TSFP_RUST_AUDIO_HOST
        dsound_stream_spatial_state next=spatial_state;
        next.curve_address=r->argument;next.curve_count=r->count;
        next.revision=spatial_state.revision+1u;
        if(!tsfp_audio_stream_lease_spatial_commit_rust(
            r->address,active_lease,&spatial_state,&next)) {
            r->error="Rust stream spatial sidecar refused the accepted rolloff curve";goto done;
        }
#endif
#ifndef TSFP_RUST_AUDIO_HOST
        n->value.curve_address=r->argument;n->value.curve_count=r->count;
#endif
        *result=0u;goto done;
    }
    if(scope->flags!=0x10u || r->apply!=0u){r->error="only startup spatial setters/apply0 are supported";goto done;}
    if(r->entry==I3DL2) {
        uint32_t words[9],expected[9]={0u,0u,0xFFFFF448u,0u,0u,0u,0u,0u,0u};
#ifndef TSFP_RUST_AUDIO_HOST
        if(cache_mask!=0u || aliases_headers(r->argument,36u,internal) ||
#else
        if(aliases_headers(r->argument,36u,internal) ||
#endif
           !kernel_guest_read_bytes(r->argument,words,sizeof(words)) || memcmp(words,expected,sizeof(words))!=0) {
            r->error="only exact first startup I3DL2 cache is supported";goto done;
        }
#ifdef TSFP_RUST_AUDIO_HOST
        if(tsfp_audio_stream_lease_startup_spatial_rust(r->address,active_lease,
            1u,r->argument,0u,words)!=0u) {
            r->error="Rust stream spatial sidecar refused startup I3DL2 state";goto done;
        }
#else
        memcpy(n->value.i3dl2,words,sizeof(words));n->value.i3dl2_address=r->argument;n->value.cache_mask=1u;
#endif
    } else if(r->entry==MINIMUM) {
#ifdef TSFP_RUST_AUDIO_HOST
        if(tsfp_audio_stream_lease_startup_spatial_rust(r->address,active_lease,
            2u,r->argument,0u,NULL)!=0u) {
            r->error="Rust stream spatial sidecar refused startup minimum-distance state";goto done;
        }
#else
        if(cache_mask!=1u || r->argument!=0x3F800000u){r->error="only ordered startup minDistance1.0 is supported";goto done;}
        n->value.min_distance_bits=r->argument;n->value.cache_mask=3u;
#endif
    } else {
#ifdef TSFP_RUST_AUDIO_HOST
        if(tsfp_audio_stream_lease_startup_spatial_rust(r->address,active_lease,
            3u,r->argument,r->count,NULL)!=0u) {
            r->error="Rust stream spatial sidecar refused startup rolloff state";goto done;
        }
#else
        if(cache_mask!=3u || r->argument!=CURVE || r->count!=4u){r->error="only ordered startup curve VA/count4 is supported";goto done;}
        n->value.curve_address=r->argument;n->value.curve_count=r->count;
        n->value.cache_mask=7u;
#endif
    }
    *result=0u;
done:pthread_mutex_unlock(&lock);
}
/* Measured spatial setters check global failure before object/settings access. */
static uint32_t spatial_preflight(uint32_t entry)
{
    pthread_mutex_lock(&lock);
    const bool admitted=stream_enabled_locked();
    uint8_t irql=0u;const bool known=irql_provider!=NULL && irql_provider(&irql);
    pthread_mutex_unlock(&lock);
    if(!admitted || !known || irql!=0u)refuse(entry,"spatial setter needs explicit policy at known IRQL0");
    uint32_t state;
    if(!kernel_guest_read_u32(GLOBAL_STATE,&state))refuse(entry,"global audio state inaccessible");
    return state!=0u?0x80004005u:0u;
}
static uint32_t access_extended(uint32_t entry,uint32_t address,uint32_t argument,uint32_t count,uint32_t apply,uint32_t position_z)
{
    if(entry==SET_POSITION || entry==MAX_DISTANCE) {
        const uint32_t failure=spatial_preflight(entry);
        if(failure!=0u)return failure;
    }
    access_request r={0};r.address=address;r.entry=entry;r.argument=argument;r.count=count;r.apply=apply;r.position_z=position_z;
    if(!identify(address,&r.identity))refuse(entry,"stream is not owned by passive adapter");
    uint32_t result=0u;
    if(!dsound_device_with_owned_interface(r.identity.internal_address+8u,access_owned,&r,&result))
        refuse(entry,"parent SILENT device ownership/generation changed");
    if(r.error!=NULL){if(dsound_hle_degrade("stream",entry,r.error))return 0u;refuse(entry,r.error);}return result;
}
static uint32_t access(uint32_t entry,uint32_t address,uint32_t argument,uint32_t count,uint32_t apply)
{return access_extended(entry,address,argument,count,apply,0u);}
uint32_t dsound_stream_set_position(uint32_t stream,uint32_t x,uint32_t y,uint32_t z,uint32_t apply)
{return access_extended(SET_POSITION,stream,x,y,apply,z);}
uint32_t dsound_stream_cache_i3dl2(uint32_t stream,uint32_t parameters,uint32_t apply)
{return access(I3DL2,stream,parameters,0u,apply);}
uint32_t dsound_stream_cache_min_distance(uint32_t stream,uint32_t bits,uint32_t apply)
{return access(MINIMUM,stream,bits,0u,apply);}
uint32_t dsound_stream_cache_max_distance(uint32_t stream,uint32_t bits,uint32_t apply)
{return access(MAX_DISTANCE,stream,bits,0u,apply);}
uint32_t dsound_stream_cache_rolloff(uint32_t stream,uint32_t curve,uint32_t count,uint32_t apply)
{return access(ROLLOFF,stream,curve,count,apply);}
uint32_t dsound_stream_cache_pause(uint32_t stream, uint32_t mode)
{return access(PAUSE,stream,mode,0u,0u);}
uint32_t dsound_stream_cache_flush_ex(uint32_t stream, uint32_t time_low,
                                      uint32_t time_high, uint32_t flags)
{return access(FLUSH_EX,stream,time_low,time_high,flags);}
uint32_t dsound_stream_cache_discontinuity(uint32_t stream)
{return access(DISCONTINUITY,stream,0u,0u,0u);}
uint32_t dsound_stream_get_startup_status(uint32_t stream,uint32_t output)
{return access(STATUS,stream,output,0u,0u);}
/* The original global failure precedes dereferencing object/input arguments. */
static uint32_t routing_preflight(uint32_t entry)
{
    pthread_mutex_lock(&lock);
    const bool admitted=stream_enabled_locked() && completion && routing_note!=NULL;
    uint8_t irql=0u;
    const bool known=irql_provider!=NULL && irql_provider(&irql);
    pthread_mutex_unlock(&lock);
    if (!admitted || !known || irql!=0u)
        refuse(entry,"routing needs explicit PCM/completion policy at known IRQL0");
    uint32_t state;
    if (!kernel_guest_read_u32(GLOBAL_STATE,&state)) refuse(entry,"global audio state inaccessible");
    return state!=0u?0x80004005u:0u;
}
#ifdef TSFP_RUST_AUDIO_HOST
_Static_assert(offsetof(kernel_call_frame,stack_ptr)==0u,
    "Rust routing decoder expects stack_ptr at the kernel frame prefix");
_Static_assert(offsetof(kernel_call_frame,stack_limit)==sizeof(uint32_t),
    "Rust routing decoder expects stack_limit second in the kernel frame prefix");
bool dsound_stream_routing_hle_policy_snapshot_rust(uint32_t *admitted_out,
    uint32_t *completion_enabled_out,uint32_t *irql_known_out,uint32_t *irql_out)
{
    if(admitted_out==NULL||completion_enabled_out==NULL||irql_known_out==NULL||irql_out==NULL)return false;
    pthread_mutex_lock(&lock);
    const bool completion_on=completion;
    const bool admitted=stream_enabled_locked() && completion_on && routing_note!=NULL;
    uint8_t irql=0u;
    const bool known=irql_provider!=NULL && irql_provider(&irql);
    pthread_mutex_unlock(&lock);
    *admitted_out=admitted?1u:0u;
    *completion_enabled_out=completion_on?1u:0u;
    *irql_known_out=known?1u:0u;
    *irql_out=(uint32_t)irql;
    return true;
}
int32_t dsound_stream_routing_hle_guest_pid_rust(void)
{return (int32_t)kernel_host_pid();}
uint32_t dsound_stream_routing_hle_apply_rust(uint32_t entry,uint32_t stream,uint32_t argument)
{return access(entry,stream,argument,0u,0u);}
_Noreturn void dsound_stream_routing_hle_refuse_rust(uint32_t entry,uint32_t reason)
{
    const char *message="unknown Rust stream-routing refusal";
    switch(reason) {
    case 1u: message="routing policy snapshot unavailable";break;
    case 2u: message="routing needs explicit PCM/completion policy at known IRQL0";break;
    case 3u: message="global audio state inaccessible";break;
    case 4u: message="Rust stream-call decoder refused frame";break;
    }
    refuse(entry,message);
}
bool dsound_stream_volume_hle_extension_rust(void *context,uint32_t *result_out)
{
    if(context==NULL||result_out==NULL)return false;
    const kernel_call_frame *frame=context;
    pthread_mutex_lock(&lock);dsound_stream_public_route_fn route=public_route;pthread_mutex_unlock(&lock);
    uint32_t caller=0u;
    return route!=NULL && kernel_guest_read_u32(frame->stack_ptr,&caller) &&
        route(VOLUME,frame,caller,result_out);
}
bool dsound_stream_create_hle_extension_rust(void *context,uint32_t caller,uint32_t *result_out)
{
    if(context==NULL||result_out==NULL)return false;
    const kernel_call_frame *frame=context;
    pthread_mutex_lock(&lock);dsound_stream_public_route_fn route=public_route;pthread_mutex_unlock(&lock);
    return route!=NULL && route(CREATE,frame,caller,result_out);
}
bool dsound_stream_volume_hle_completion_enabled_rust(void)
{pthread_mutex_lock(&lock);const bool value=completion;pthread_mutex_unlock(&lock);return value;}
int32_t dsound_stream_volume_hle_guest_pid_rust(void)
{return (int32_t)kernel_host_pid();}
uint32_t dsound_stream_volume_hle_apply_rust(uint32_t stream,uint32_t millibels,bool after_format)
{return access(VOLUME,stream,millibels,after_format?1u:0u,0u);}
_Noreturn void dsound_stream_volume_hle_refuse_rust(uint32_t reason)
{
    const char *message=reason==1u?"Rust stream-call decoder refused frame":"unknown Rust stream-volume refusal";
    refuse(VOLUME,message);
}
bool dsound_stream_spatial_hle_completion_enabled_rust(void)
{pthread_mutex_lock(&lock);const bool value=completion;pthread_mutex_unlock(&lock);return value;}
int32_t dsound_stream_spatial_hle_guest_pid_rust(void)
{return (int32_t)kernel_host_pid();}
uint32_t dsound_stream_spatial_hle_apply_rust(uint32_t entry,const uint32_t *args)
{
    if(args==NULL)refuse(entry,"Rust spatial handler supplied no decoded arguments");
    if(entry==I3DL2)return dsound_stream_cache_i3dl2(args[0],args[1],args[2]);
    if(entry==MINIMUM)return dsound_stream_cache_min_distance(args[0],args[1],args[2]);
    if(entry==MAX_DISTANCE)return dsound_stream_cache_max_distance(args[0],args[1],args[2]);
    if(entry==SET_POSITION) {
        const uint32_t stream=args[0],x=args[1],y=args[2],z=args[3],apply=args[4];
        return dsound_stream_set_position(stream,x,y,z,apply);
    }
    if(entry==ROLLOFF)return dsound_stream_cache_rolloff(args[0],args[1],args[2],args[3]);
    refuse(entry,"Rust spatial handler selected an unmeasured entry");
}
bool dsound_stream_control_hle_prepare_rust(uint32_t entry,const kernel_call_frame *frame,
    bool *completion_out,uint32_t *caller_out,uint32_t *result_out)
{
    if(completion_out==NULL||caller_out==NULL||result_out==NULL)
        refuse(entry,"Rust stream-control callback supplied no prepare outputs");
    pthread_mutex_lock(&lock);
    dsound_stream_public_route_fn route=public_route;
    *completion_out=completion;
    pthread_mutex_unlock(&lock);
    *caller_out=0u;*result_out=0u;
    uint32_t actual;
    if(route!=NULL&&frame!=NULL&&kernel_guest_read_u32(frame->stack_ptr,&actual)) {
        *caller_out=actual;
        uint32_t extension;
        if(route(entry,frame,actual,&extension)) {*result_out=extension;return true;}
    }
    return false;
}
int32_t dsound_stream_control_hle_guest_pid_rust(void)
{return (int32_t)kernel_host_pid();}
uint32_t dsound_stream_control_hle_apply_rust(uint32_t entry,uint32_t caller,const uint32_t *args)
{
    if(args==NULL)refuse(entry,"Rust stream-control callback supplied no decoded arguments");
    if(entry==PAUSE) {
        const uint32_t path=caller==0x29A51u?1u:(caller==0x29ED1u?2u:0u);
        return access(PAUSE,args[0],args[1],path,0u);
    }
    if(entry==FLUSH_EX)return dsound_stream_cache_flush_ex(args[0],args[1],args[2],args[3]);
    if(entry==SET_FORMAT)return dsound_stream_cache_set_format(args[0],args[1]);
    refuse(entry,"Rust stream-control callback selected an unmeasured entry");
}
_Noreturn void dsound_stream_control_hle_refuse_rust(uint32_t entry,uint32_t reason)
{
    const char *message=reason==1u?"Rust stream-call decoder refused frame":
        (reason==2u?"Rust stream caller changed during decode":
        (reason==3u?"guest x87 control word unavailable":
        (reason==4u?"frequency needs explicit PCM/completion policy at known IRQL0":
        (reason==5u?"global audio state inaccessible":"unknown Rust stream-control refusal"))));
    refuse(entry,message);
}
_Noreturn void dsound_stream_spatial_hle_refuse_rust(uint32_t entry,uint32_t reason)
{
    const char *message=reason==1u?"Rust stream-call decoder refused frame":"unknown Rust stream-spatial refusal";
    refuse(entry,message);
}
#endif
static uint32_t frequency_preflight(void)
{
    pthread_mutex_lock(&lock);
    const bool admitted=stream_enabled_locked() && completion && frequency_note!=NULL;
    uint8_t irql=0u;const bool known=irql_provider!=NULL && irql_provider(&irql);
    pthread_mutex_unlock(&lock);
    if(!admitted || !known || irql!=0u)refuse(SET_FREQUENCY,"frequency needs explicit PCM/completion policy at IRQL0");
    uint32_t state;
    if(!kernel_guest_read_u32(GLOBAL_STATE,&state))refuse(SET_FREQUENCY,"global audio state inaccessible");
    return state!=0u?0x80004005u:0u;
}
#ifdef TSFP_RUST_AUDIO_HOST
bool dsound_stream_frequency_hle_policy_inputs_rust(
    dsound_stream_frequency_policy_inputs *output)
{
    if(output==NULL)return false;
    pthread_mutex_lock(&lock);
    output->enabled=stream_enabled_locked()?1u:0u;
    output->completion=completion?1u:0u;
    output->handler_available=frequency_note!=NULL?1u:0u;
    uint8_t irql=0u;
    output->irql_known=irql_provider!=NULL&&irql_provider(&irql)?1u:0u;
    output->irql=(uint32_t)irql;
    pthread_mutex_unlock(&lock);
    return true;
}
int32_t dsound_stream_frequency_hle_guest_pid_rust(void)
{return kernel_host_pid();}
bool dsound_stream_frequency_hle_control_word_rust(uint16_t *output)
{
    if(output==NULL)return false;
    pthread_mutex_lock(&lock);
    dsound_stream_control_word_fn read_word=frequency_control_word;
    const bool known=read_word!=NULL&&read_word(output);
    pthread_mutex_unlock(&lock);
    return known;
}
uint32_t dsound_stream_frequency_hle_apply_rust(uint32_t stream,uint32_t hertz,uint16_t control_word)
{return access(SET_FREQUENCY,stream,hertz,control_word,0u);}
#endif
uint32_t dsound_stream_set_frequency(uint32_t stream,uint32_t hertz,uint16_t control_word)
{
    const uint32_t failure=frequency_preflight();
    return failure!=0u?failure:access(SET_FREQUENCY,stream,hertz,control_word,0u);
}
uint32_t dsound_stream_set_headroom(uint32_t stream,uint32_t headroom)
{uint32_t failure=routing_preflight(HEADROOM);return failure!=0u?failure:access(HEADROOM,stream,headroom,0u,0u);}
uint32_t dsound_stream_set_mix_bins(uint32_t stream,uint32_t bins)
{uint32_t failure=routing_preflight(MIX_BINS);return failure!=0u?failure:access(MIX_BINS,stream,bins,0u,0u);}
uint32_t dsound_stream_set_mix_bin_volumes(uint32_t stream,uint32_t bins)
{uint32_t failure=routing_preflight(MIX_VOLUMES);return failure!=0u?failure:access(MIX_VOLUMES,stream,bins,0u,0u);}
uint32_t dsound_stream_cache_set_format(uint32_t stream,uint32_t format)
{return access(SET_FORMAT,stream,format,0u,0u);}
uint32_t dsound_stream_cache_volume(uint32_t stream, int32_t volume)
{return access(VOLUME,stream,(uint32_t)volume,0u,0u);}
bool dsound_stream_get_snapshot(uint32_t stream,dsound_stream_snapshot *output)
{
    if(output==NULL)return false;access_request r={0};dsound_stream_snapshot value;
    r.address=stream;r.observer=true;r.snapshot=&value;
    if(!identify(stream,&r.identity))return false;uint32_t result=0u;
    if(!dsound_device_with_owned_interface(r.identity.internal_address+8u,access_owned,&r,&result) || r.error!=NULL)return false;
    *output=value;return true;
}
typedef struct reset_request {
    uint32_t address;
    dsound_device_lease identity;
    stream_node *node;
#ifdef TSFP_RUST_AUDIO_HOST
    stream_node key;
#endif
    bool held;
} reset_request;
static bool reset_prepare(const dsound_device_lease *candidate,void *userdata,dsound_device_lease_child *child)
{
    reset_request *r=userdata;pthread_mutex_lock(&lock);r->held=true;
#ifdef TSFP_RUST_AUDIO_HOST
    r->key.value.stream_address=r->address;r->node=&r->key;
#else
    r->node=find(r->address);
#endif
    if(!token_equal(candidate,&r->identity) || !valid_node(r->node,&r->identity,candidate->internal_address))return false;
#ifdef TSFP_RUST_AUDIO_HOST
    uint32_t heap=0u;
    if(!tsfp_audio_stream_lease_heap_rust(r->address,&heap) || heap==0u)return false;
    *child=(dsound_device_lease_child){heap,r->address,STREAM_BYTES};
#else
    *child=(dsound_device_lease_child){r->node->value.stream_heap,r->address,STREAM_BYTES};
#endif
    return true;
}
static void reset_abort(void *userdata)
{reset_request *r=userdata;r->held=false;pthread_mutex_unlock(&lock);}
static void reset_finalize(const dsound_device_lease *committed,void *userdata)
{
    reset_request *r=userdata;(void)committed;
#ifdef TSFP_RUST_AUDIO_HOST
    if(!tsfp_audio_stream_lease_detach_rust(r->address,&r->identity))
        dsound_hle_log()("dsound: Rust stream lease detach invariant failed stream=%#x serial=%llu\n",
            r->address,(unsigned long long)r->identity.serial);
    (void)tsfp_audio_stream_routing_retire_rust(r->address,r->identity.serial);
#else
    stream_node **p=&streams;
    while(*p!=r->node)p=&(*p)->next;*p=r->node->next;r->node->next=detached;detached=r->node;
#ifdef TSFP_RUST_STREAM_FREQUENCY_STATE
    if(!tsfp_audio_stream_routing_retire_rust(r->address,r->identity.serial))
        dsound_hle_log()("dsound: Rust frequency sidecar retire invariant failed stream=%#x serial=%llu\n",
            r->address,(unsigned long long)r->identity.serial);
#endif
#endif
    r->held=false;pthread_mutex_unlock(&lock);
}
static const dsound_device_lease_ops reset_ops={reset_prepare,reset_abort,reset_finalize};
#ifdef TSFP_RUST_AUDIO_HOST
uint32_t tsfp_audio_stream_reset_release_host_c(uint32_t address,
    const dsound_device_lease *identity)
{
    if(address==0u || identity==NULL)return (uint32_t)DSOUND_LEASE_INVALID;
    reset_request request={0};request.address=address;request.identity=*identity;
    return (uint32_t)dsound_device_release_lease(identity,&reset_ops,&request);
}
bool tsfp_audio_stream_reset_validate_host_c(uint32_t address,
    const dsound_device_lease *identity)
{
    if(address==0u || identity==NULL)return false;
    reset_request request={0};request.address=address;request.identity=*identity;
    request.key.value.stream_address=address;request.node=&request.key;
    return valid_node(request.node,identity,identity->internal_address);
}
bool tsfp_audio_stream_reset_heap_destroy_host_c(uint32_t heap)
{return heap!=0u && stream_heap_destroy(heap);}
void tsfp_audio_stream_reset_log_host_c(uint32_t code,uint32_t address,uint64_t serial)
{
    if(code==1u)
        dsound_hle_log()("dsound: Rust stream lease retire invariant failed stream=%#x serial=%llu\n",
            address,(unsigned long long)serial);
    else if(code==2u)
        dsound_hle_log()("dsound: Rust stream lease rollback retire invariant failed\n");
}
static bool reset_streams(void)
{
    return tsfp_audio_stream_reset_rust();
}
#else
static bool reset_streams(void)
{
    for(;;) {
        reset_request r={0};pthread_mutex_lock(&lock);
        if(streams!=NULL){r.address=streams->value.stream_address;r.identity=streams->value.lease;}
        pthread_mutex_unlock(&lock);if(r.address==0u)break;
        if(dsound_device_release_lease(&r.identity,&reset_ops,&r)!=DSOUND_LEASE_OK)return false;
        /* Detached nodes remain tracked if checked cleanup refuses. No fallible
         * guest cleanup runs under device lock or in finalize. */
        pthread_mutex_lock(&lock);stream_node **p=&detached;
        while(*p!=r.node)p=&(*p)->next;
        if(!valid_node(r.node,&r.identity,r.identity.internal_address)){
            pthread_mutex_unlock(&lock);return false;
        }
        if(!dsound_audio_heap_destroy(r.node->value.stream_heap)){
            pthread_mutex_unlock(&lock);return false;
        }
        *p=r.node->next;free(r.node);pthread_mutex_unlock(&lock);
    }
    pthread_mutex_lock(&lock);
    while(detached!=NULL) {
        stream_node *n=detached;
        if(!valid_node(n,&n->value.lease,n->value.lease.internal_address)){
            pthread_mutex_unlock(&lock);return false;
        }
        if(!dsound_audio_heap_destroy(n->value.stream_heap)){
            pthread_mutex_unlock(&lock);return false;
        }
        detached=n->next;free(n);
    }
    while(rollback!=NULL) {
        stream_node *n=rollback;
        /* Unleased rollback ownership is the exclusive private heap generation,
         * not a device reference. Never release a lease or destroy a replacement. */
        if(!dsound_audio_heap_valid(n->value.stream_heap)) {
            pthread_mutex_unlock(&lock);return false;
        }
        if(!dsound_audio_heap_destroy(n->value.stream_heap)) {
            pthread_mutex_unlock(&lock);return false;
        }
        rollback=n->next;free(n);
    }
#ifdef TSFP_RUST_AUDIO_HOST
    tsfp_audio_stream_create_reset_announcement_rust();
#else
    announced=false;
#endif
    pthread_mutex_unlock(&lock);return true;
}
#endif
bool dsound_stream_reset_checked(void)
{
    /* Extension streams (T392 movie streams) lease the same device, so they go first. */
    pthread_mutex_lock(&lock);dsound_stream_reset_route_fn extension=reset_route;pthread_mutex_unlock(&lock);
    const bool movies=extension==NULL || extension();
    pthread_mutex_lock(&reset_lock);bool result=reset_streams();
    pthread_mutex_unlock(&reset_lock);return movies && result;
}
void dsound_stream_reset(void){(void)dsound_stream_reset_checked();}
#ifndef TSFP_RUST_AUDIO_HOST
/* `second` is a further measured caller inside sub_000299C0 (T602, the sound start re-formats a started stream), 0 for none.
 * `third` (T743) is a caller admitted ONLY with the completion model on: the stream update sub_00029F30 -> sub_00029EE0 starts
 * the stream (Pause(0) of the started stream after its first Process, return address 0x29F13). The original 0x407B23 never
 * reads its caller, the model needs the started state it already checks. 0 for none. */
static uint32_t frame_handler(void *context,uint32_t entry,uint32_t caller,uint32_t second,uint32_t third,unsigned count)
{
    const kernel_call_frame *frame=context;uint32_t actual,args[4];
    pthread_mutex_lock(&lock);dsound_stream_public_route_fn route=public_route;const bool third_admitted=completion;pthread_mutex_unlock(&lock);
    if(route!=NULL && frame!=NULL && kernel_guest_read_u32(frame->stack_ptr,&actual)){
        uint32_t extension;
        /* T392: an installed extension (the movie stream) may answer its own streams first. */
        if(route(entry,frame,actual,&extension))return extension;
    }
    if(frame==NULL || !kernel_guest_read_u32(frame->stack_ptr,&actual) ||
       (actual!=caller && (second==0u || actual!=second) && (!third_admitted || third==0u || actual!=third) &&
        !(third_admitted && entry==PAUSE && actual==0x29ED1u))) {
        /* The actual return address is named (T743), the text keeps its measured prefix. */
        static char reason[96];
        const bool have=frame!=NULL && kernel_guest_read_u32(frame->stack_ptr,&actual);
        (void)snprintf(reason,sizeof reason,"only measured startup caller is supported, got caller %#x",have?actual:0u);
        if(dsound_hle_degrade("stream",entry,reason))return 0u;
        refuse(entry,reason);
    }
    for(unsigned i=0u;i<count;i++)if(!kernel_frame_arg(frame,i,&args[i]))refuse(entry,"unreadable argument");
    if(entry==CREATE)return dsound_stream_create(args[0],args[1]);
    if(entry==I3DL2)return dsound_stream_cache_i3dl2(args[0],args[1],args[2]);
    if(entry==MINIMUM)return dsound_stream_cache_min_distance(args[0],args[1],args[2]);
    if(entry==MAX_DISTANCE)return dsound_stream_cache_max_distance(args[0],args[1],args[2]);
    if(entry==SET_FORMAT)return dsound_stream_cache_set_format(args[0],args[1]);
    /* The second SetVolume caller is the one after Pause and Discontinuity of the re-format, so it needs a recorded SetFormat. */
    if(entry==VOLUME)return access(VOLUME,args[0],args[1],actual==second?1u:0u,0u);
    /* The second Pause caller is the one right after SetFormat, so it needs a recorded SetFormat. */
    if(entry==PAUSE)return access(PAUSE,args[0],args[1],actual==second?1u:(actual==0x29ED1u?2u:0u),0u);
    if(entry==FLUSH_EX)return dsound_stream_cache_flush_ex(args[0],args[1],args[2],args[3]);
    return dsound_stream_cache_rolloff(args[0],args[1],args[2],args[3]);
}
#endif
#ifndef TSFP_RUST_AUDIO_HOST
static uint32_t create_handler(void *c){return frame_handler(c,CREATE,0x29943u,0u,0u,2u);}
#endif
#ifndef TSFP_RUST_AUDIO_HOST
static uint32_t i3dl2_handler(void *c){return frame_handler(c,I3DL2,0x2998Au,0u,0u,3u);}
#endif
#ifndef TSFP_RUST_AUDIO_HOST
static uint32_t minimum_handler(void *c){return frame_handler(c,MINIMUM,0x29998u,0u,0u,3u);}
static uint32_t max_distance_handler(void *context)
{
    const kernel_call_frame *frame=context;uint32_t actual,args[3];
    if(frame==NULL || !kernel_guest_read_u32(frame->stack_ptr,&actual) ||
       (actual!=0x29ADAu && actual!=0x29D86u))
        refuse(MAX_DISTANCE,"only the two measured spatial startup/update callers are supported");
    for(unsigned i=0u;i<3u;i++)if(!kernel_frame_arg(frame,i,&args[i]))
        refuse(MAX_DISTANCE,"unreadable SetMaxDistance argument");
    (void)actual;
    return dsound_stream_cache_max_distance(args[0],args[1],args[2]);
}
#endif
#ifndef TSFP_RUST_AUDIO_HOST
static uint32_t set_position_handler(void *context)
{
    const kernel_call_frame *frame=context;uint32_t actual,args[5];
    if(frame==NULL || !kernel_guest_read_u32(frame->stack_ptr,&actual) ||
       (actual!=0x29ACAu && actual!=0x29E2Bu && actual!=0x29FADu))
        refuse(SET_POSITION,"only the three measured spatial stream SetPosition callers are supported");
    for(unsigned i=0u;i<5u;i++)if(!kernel_frame_arg(frame,i,&args[i]))
        refuse(SET_POSITION,"unreadable SetPosition argument");
    (void)actual;
    return dsound_stream_set_position(args[0],args[1],args[2],args[3],args[4]);
}
#endif
#ifndef TSFP_RUST_AUDIO_HOST
static uint32_t rolloff_handler(void *context)
{
    const kernel_call_frame *frame=context;uint32_t actual,args[4];
    if(frame==NULL || !kernel_guest_read_u32(frame->stack_ptr,&actual) ||
       (actual!=0x299A8u && actual!=0x29E81u && actual!=0x29E90u)) {
        static char reason[112];
        const bool have=frame!=NULL && kernel_guest_read_u32(frame->stack_ptr,&actual);
        (void)snprintf(reason,sizeof reason,"only measured startup caller is supported, got caller %#x",have?actual:0u);
        refuse(ROLLOFF,reason);
    }
    for(unsigned i=0u;i<4u;i++)if(!kernel_frame_arg(frame,i,&args[i]))refuse(ROLLOFF,"unreadable SetRolloffCurve argument");
    (void)actual;
    return dsound_stream_cache_rolloff(args[0],args[1],args[2],args[3]);
}
#endif
#ifndef TSFP_RUST_AUDIO_HOST
static uint32_t pause_handler(void *c){return frame_handler(c,PAUSE,0x29B5Au,0x29A51u,0x29F13u,2u);}
static uint32_t flush_ex_handler(void *c){return frame_handler(c,FLUSH_EX,0x29B69u,0u,0u,4u);}
#endif
#ifndef TSFP_RUST_AUDIO_HOST
static uint32_t set_format_handler(void *c){return frame_handler(c,SET_FORMAT,0x299FAu,0u,0u,2u);}
#endif
#ifndef TSFP_RUST_AUDIO_HOST
static uint32_t set_frequency_handler(void *context)
{
    const uint32_t failure=frequency_preflight();if(failure!=0u)return failure;
    const kernel_call_frame *frame=context;uint32_t actual,stream,hertz;uint16_t cw;
    if(frame==NULL || !kernel_guest_read_u32(frame->stack_ptr,&actual) || actual!=0x29B35u)
        refuse(SET_FREQUENCY,"only measured return address 0x29B35 is supported");
    if(!kernel_frame_arg(frame,0u,&stream) || !kernel_frame_arg(frame,1u,&hertz))
        refuse(SET_FREQUENCY,"unreadable frequency arguments");
    (void)actual;
    pthread_mutex_lock(&lock);
    const bool known=frequency_control_word!=NULL && frequency_control_word(&cw);
    pthread_mutex_unlock(&lock);
    if(!known)refuse(SET_FREQUENCY,"guest x87 control word unavailable");
    return access(SET_FREQUENCY,stream,hertz,cw,0u);
}
#endif
#ifndef TSFP_RUST_AUDIO_HOST
static uint32_t volume_handler(void *c){return frame_handler(c,VOLUME,0x29F68u,0x29A67u,0u,2u);}
#endif
#ifndef TSFP_RUST_AUDIO_HOST
/* Original routing wrappers are STDCALL2/RET8 and have no caller restriction;
 * the checked adapter instead proves live ownership and its PCM capability. */
static uint32_t routing_handler(void *context,uint32_t entry)
{
    const kernel_call_frame *frame=context;uint32_t stream,argument;
    const uint32_t failure=routing_preflight(entry);if (failure!=0u) return failure;
    if (!kernel_frame_arg(frame,0u,&stream) || !kernel_frame_arg(frame,1u,&argument))
        refuse(entry,"unreadable routing arguments");
    return access(entry,stream,argument,0u,0u);
}
static uint32_t headroom_handler(void *c){return routing_handler(c,HEADROOM);}
static uint32_t mix_bins_handler(void *c){return routing_handler(c,MIX_BINS);}
static uint32_t mix_volumes_handler(void *c){return routing_handler(c,MIX_VOLUMES);}
#endif
size_t dsound_stream_register(void)
{
    size_t count=0u;
#ifdef TSFP_RUST_AUDIO_HOST
    count+=tsfp_audio_stream_create_register_rust();
#else
    count+=dsound_hle_register(CREATE,create_handler)?1u:0u;
#endif
#ifdef TSFP_RUST_AUDIO_HOST
    count+=tsfp_audio_stream_i3dl2_register_rust();
#else
    count+=dsound_hle_register(I3DL2,i3dl2_handler)?1u:0u;
#endif
#ifdef TSFP_RUST_AUDIO_HOST
    count+=tsfp_audio_stream_minimum_register_rust();
#else
    count+=dsound_hle_register(MINIMUM,minimum_handler)?1u:0u;
#endif
#ifdef TSFP_RUST_AUDIO_HOST
    count+=tsfp_audio_stream_rolloff_register_rust();
#else
    count+=dsound_hle_register(ROLLOFF,rolloff_handler)?1u:0u;
#endif
#ifdef TSFP_RUST_AUDIO_HOST
    count+=tsfp_audio_stream_control_register_rust();
#else
    count+=dsound_hle_register(PAUSE,pause_handler)?1u:0u;
    count+=dsound_hle_register(FLUSH_EX,flush_ex_handler)?1u:0u;
#endif
    /* Only with the policy on, so a flags-off boot keeps its exact registry (T602). */
    pthread_mutex_lock(&lock);const bool policy=stream_enabled_locked();pthread_mutex_unlock(&lock);
    if(policy) {
#ifdef TSFP_RUST_AUDIO_HOST
        count+=tsfp_audio_stream_format_register_rust();
#else
        count+=dsound_hle_register(SET_FORMAT,set_format_handler)?1u:0u;
#endif
    }
#ifdef TSFP_RUST_AUDIO_HOST
    if(policy)count+=tsfp_audio_stream_spatial_register_rust();
#else
    if(policy)count+=dsound_hle_register(MAX_DISTANCE,max_distance_handler)?1u:0u;
    if(policy)count+=dsound_hle_register(SET_POSITION,set_position_handler)?1u:0u;
#endif
    pthread_mutex_lock(&lock);const bool real_frequency=policy && frequency_note!=NULL && frequency_control_word!=NULL;pthread_mutex_unlock(&lock);
    if(real_frequency) {
#ifdef TSFP_RUST_AUDIO_HOST
        count+=tsfp_audio_stream_frequency_register_rust();
#else
        count+=dsound_hle_register(SET_FREQUENCY,set_frequency_handler)?1u:0u;
#endif
    }
    pthread_mutex_lock(&lock);const bool real_routing=policy && routing_note!=NULL;pthread_mutex_unlock(&lock);
    if (real_routing) {
#ifdef TSFP_RUST_AUDIO_HOST
        count+=tsfp_audio_stream_routing_register_rust();
#else
        count+=dsound_hle_register(HEADROOM,headroom_handler)?1u:0u;
        count+=dsound_hle_register(MIX_BINS,mix_bins_handler)?1u:0u;
        count+=dsound_hle_register(MIX_VOLUMES,mix_volumes_handler)?1u:0u;
#endif
    }
#ifdef TSFP_RUST_AUDIO_HOST
    count+=tsfp_audio_stream_volume_register_rust();
#else
    count+=dsound_hle_register(VOLUME,volume_handler)?1u:0u;return count;
#endif
    return count;
}
