/* SPDX-License-Identifier: GPL-3.0-or-later
 * Synthetic bounded mapping and dispatcher only. All service answers come from
 * the real repository kernel_*.c files compiled by build.rs. No disc required. */
#include "kernel_call.h"
#include "kernel_hle.h"
#include "kernel_hal.h"
#include "kernel_config.h"
#include "kernel_crypto.h"
#include "kernel_identity.h"
#include "kernel_disk_identity.h"
#include "kernel_av.h"
#include "kernel_dbg.h"
#include "kernel_fpstate.h"
#include <string.h>
#include <stdarg.h>

static _Alignas(4) uint8_t arena[8192];
static kernel_fn handlers[371];
static const uint32_t *arguments;
static unsigned argument_count;
static uint32_t fatal_ordinal, firmware_routine, firmware_pending;
static char printed[1024];
static unsigned readable_limit = sizeof(arena), writable_limit = sizeof(arena);

bool kernel_guest_range_readable(uint32_t addr, size_t len)
{ return addr != 0u && (uint64_t)addr + len <= readable_limit; }
void *kernel_guest_at(uint32_t addr, size_t len)
{ return kernel_guest_range_readable(addr,len) ? arena + addr : NULL; }
const void *kernel_guest_at_ro(uint32_t addr, size_t len) { return kernel_guest_at(addr, len); }
bool kernel_guest_read_bytes(uint32_t addr, void *out, size_t len)
{ if (!out || !kernel_guest_range_readable(addr,len)) return false; memcpy(out,arena+addr,len); return true; }
bool kernel_guest_write_bytes(uint32_t addr, const void *input, size_t len)
{
    if (!input || addr == 0u || (uint64_t)addr+len > sizeof(arena)) return false;
    size_t available = addr < writable_limit ? writable_limit-addr : 0u;
    size_t written = len < available ? len : available;
    memcpy(arena+addr,input,written); return written == len;
}
bool kernel_guest_read_u32(uint32_t addr, uint32_t *out)
{ uint8_t b[4]; if (!kernel_guest_read_bytes(addr,b,4)) return false; *out = (uint32_t)b[0] | ((uint32_t)b[1]<<8) | ((uint32_t)b[2]<<16) | ((uint32_t)b[3]<<24); return true; }
bool kernel_guest_write_u32(uint32_t addr, uint32_t value)
{ const uint8_t b[] = {(uint8_t)value,(uint8_t)(value>>8),(uint8_t)(value>>16),(uint8_t)(value>>24)}; return kernel_guest_write_bytes(addr,b,4); }
bool kernel_guest_read_u8(uint32_t addr, uint8_t *out) { return kernel_guest_read_bytes(addr,out,1); }
bool kernel_guest_write_u8(uint32_t addr, uint8_t value) { return kernel_guest_write_bytes(addr,&value,1); }
uint32_t kernel_guest_add(uint32_t base, uint32_t offset)
{ return base && (uint64_t)base+offset <= UINT32_MAX ? base+offset : 0u; }
bool kernel_frame_arg(const kernel_call_frame *frame, unsigned index, uint32_t *out)
{ if (!frame || index >= argument_count) return false; *out = arguments[index]; return true; }
bool kernel_hle_register(unsigned ordinal, kernel_fn fn)
{ if (ordinal >= 371u) return false; handlers[ordinal] = fn; return true; }
static int silent_log(const char *format, ...) {
    /* Only preserve debug text for comparison, never print source/identity/key data. */
    if (strcmp(format,"DbgPrint: %s\n") == 0) {
        va_list ap; va_start(ap,format); const char *s = va_arg(ap,const char *);
        size_t n = strlen(s); if (n >= sizeof(printed)) n = sizeof(printed)-1;
        memcpy(printed,s,n); printed[n] = 0; va_end(ap);
    }
    return 0;
}
kernel_log_fn kernel_hle_log(void) { return silent_log; }
void kernel_hle_fatal(unsigned ordinal, const char *format, ...)
{ (void)format; fatal_ordinal = ordinal; }
static void firmware(uint32_t routine, unsigned pending)
{ firmware_routine = routine; firmware_pending = pending; }
void oracle_reset(void)
{
    kernel_hal_reset(); kernel_crypto_reset(); kernel_config_reset(); kernel_av_set_widescreen(false);
    kernel_av_reset(); kernel_dbg_reset(); kernel_fpstate_reset();
    kernel_hal_set_firmware_sink(firmware);
    kernel_hal_register(); kernel_config_register(); kernel_crypto_register();
    kernel_av_register(); kernel_dbg_register(); kernel_fpstate_register();
    readable_limit = writable_limit = sizeof(arena); memset(arena,0,sizeof(arena));
    fatal_ordinal = firmware_routine = firmware_pending = 0; printed[0] = 0;
}
void oracle_memory(const uint8_t *bytes) { memcpy(arena,bytes,sizeof(arena)); }
void oracle_snapshot(uint8_t *bytes) { memcpy(bytes,arena,sizeof(arena)); }
void oracle_limits(unsigned read, unsigned write) { readable_limit = read; writable_limit = write; }
uint32_t oracle_call(unsigned ordinal, const uint32_t *args, unsigned count)
{
    arguments = args; argument_count = count;
    kernel_call_frame frame = {0};
    return handlers[ordinal](&frame);
}
unsigned oracle_config_set(uint32_t index, const uint8_t *bytes, unsigned count)
{ return kernel_config_set_setting(index,bytes,count); }
void oracle_config_policy(unsigned policy) { kernel_config_set_unknown_policy((kernel_config_unknown_policy)policy); }
unsigned oracle_eeprom(const uint8_t *bytes, const uint8_t *key)
{ return key ? kernel_config_set_eeprom_keyed(bytes,256,key,16) : kernel_config_set_eeprom(bytes,256); }
unsigned oracle_av(unsigned pack, unsigned region, unsigned widescreen)
{ kernel_av_set_widescreen(widescreen != 0); return kernel_av_configure((kernel_av_pack)pack,region); }
unsigned oracle_av_install(void) { return kernel_av_install_settings(); }
unsigned oracle_identity(unsigned hardware, unsigned version) { return kernel_identity_publish(hardware,version); }
unsigned oracle_disk(unsigned model, unsigned serial, unsigned storage, const uint8_t *m, unsigned nm, const uint8_t *s, unsigned ns)
{
    kernel_disk_identity_snapshot id = {0};
    if (nm > 40 || ns > 20) return 0;
    memcpy(id.model,m,nm); memcpy(id.serial,s,ns); id.model_length = (uint16_t)nm; id.serial_length = (uint16_t)ns;
    return kernel_disk_identity_publish(model,serial,storage,&id);
}
unsigned oracle_counter(unsigned id)
{
    switch (id) {
    case 0: return kernel_hal_shutdown_count();
    case 1: return kernel_hal_shutdown_duplicate_count();
    case 2: return kernel_hal_shutdown_unmatched_count();
    case 3: return kernel_hal_shutdown_overflow_count();
    case 4: return kernel_fpstate_total_depth();
    case 5: return kernel_fpstate_anomaly_count();
    case 6: return kernel_config_served_count();
    case 7: return kernel_config_fabricated_count();
    case 8: return kernel_config_refused_count();
    case 9: return kernel_av_fabricated_count();
    case 10: return kernel_av_refused_count();
    case 11: return kernel_av_ignored_option_count();
    case 12: return kernel_dbg_print_count();
    case 13: return kernel_dbg_refused_count();
    case 14: return fatal_ordinal;
    case 15: return firmware_routine;
    case 16: return firmware_pending;
    case 17: return kernel_av_capabilities();
    case 18: return kernel_av_smc_video_mode();
    case 19: return kernel_crypto_sha_init_count();
    case 20: return kernel_crypto_sha_update_count();
    case 21: return kernel_crypto_sha_final_count();
    case 22: return kernel_crypto_hmac_count();
    case 23: return kernel_crypto_des_parity_count();
    case 24: return kernel_crypto_stale_context_count();
    case 25: return kernel_crypto_hmac_oversize_key_count();
    case 26: return kernel_config_queried_indices(NULL,0);
    }
    return UINT32_MAX;
}
unsigned oracle_debug(uint8_t *bytes) { size_t n = strlen(printed); memcpy(bytes,printed,n); return (unsigned)n; }

uint64_t oracle_absorbed(void) { return kernel_crypto_sha_bytes_absorbed(); }
unsigned oracle_queried(uint32_t *out) { return kernel_config_queried_indices(out,64); }
unsigned oracle_disk_read(unsigned model, unsigned serial, uint8_t *bytes) {
    kernel_disk_identity_snapshot id = {0};
    if (!kernel_disk_identity_read(model,serial,&id)) return 0;
    memcpy(bytes,&id,sizeof(id)); return 1;
}
unsigned oracle_av_image(unsigned pack, unsigned base) { return kernel_av_configure_from_image((kernel_av_pack)pack,base); }
unsigned oracle_key(unsigned ordinal, uint8_t *output) { return kernel_config_eeprom_key(ordinal,output); }

uint32_t oracle_stored(const uint32_t *args, unsigned count) {
    arguments = args; argument_count = count; kernel_call_frame frame = {0};
    return kernel_config_query_stored(&frame);
}
