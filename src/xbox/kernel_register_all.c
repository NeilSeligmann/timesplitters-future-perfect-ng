/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * See kernel_register_all.h. Add a new module's register call HERE and nowhere else.
 */

#include "kernel_register_all.h"

/* Keep the replaceable file registration call behind the Rust-owned ABI. */
extern unsigned tsfp_rfio_kernel_file_register(void);

#include "kernel_av.h"
#include "kernel_clock.h"
#include "kernel_config.h"
#include "kernel_critsec.h"
#include "kernel_crypto.h"
#include "kernel_dbg.h"
#include "kernel_event.h"
#include "kernel_event_handle.h"
#include "kernel_file_object.h"
#include "kernel_fpstate.h"
#include "kernel_hal.h"
#include "kernel_io.h"
#include "kernel_memory.h"
#include "kernel_object.h"
#include "kmem_abi.h"
#include "kernel_rtl.h"
#include "kernel_rtl_string.h"
#include "kernel_sync.h"
#include "kernel_thread.h"
#include "kernel_xe.h"

size_t kernel_register_all(void)
{
    size_t bound = kmem_register_ordinals();
    bound += kernel_sync_register();
    bound += kernel_thread_register();
    bound += kernel_object_register();
    bound += kernel_critsec_register();
    bound += kernel_rtl_register();
    bound += kernel_rtl_string_register();
    bound += kernel_event_register();
    bound += kernel_event_handle_register();
    bound += kernel_hal_register();
    bound += kernel_crypto_register();
    bound += kernel_config_register();
    bound += tsfp_rfio_kernel_file_register();
    bound += kernel_file_object_register();
    bound += kernel_io_register();
    bound += kernel_xe_register();
    bound += kernel_av_register();
    bound += kernel_clock_register();
    bound += kernel_dbg_register();
    bound += kernel_fpstate_register();
    return bound;
}
