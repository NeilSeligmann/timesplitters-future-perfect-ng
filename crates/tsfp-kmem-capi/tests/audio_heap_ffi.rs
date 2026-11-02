// SPDX-License-Identifier: GPL-3.0-or-later

#[path = "support/witness_stubs.rs"]
mod witness_stubs;
use tsfp_kmem_capi::{
    MODE_RUST, tsfp_kmem_audio_heap_active, tsfp_kmem_audio_heap_alloc,
    tsfp_kmem_audio_heap_block_size, tsfp_kmem_audio_heap_create, tsfp_kmem_audio_heap_destroy,
    tsfp_kmem_audio_heap_valid, tsfp_kmem_init, tsfp_kmem_op,
};

#[test]
fn audio_heap_ffi_uses_the_shared_rust_guest_regions_state() {
    assert_eq!(tsfp_kmem_init(MODE_RUST, None, None), MODE_RUST);
    assert!(tsfp_kmem_audio_heap_active());

    let heap = tsfp_kmem_audio_heap_create(0, 0x1_0000, 0);
    assert_ne!(heap, 0);
    assert!(tsfp_kmem_audio_heap_valid(heap));

    let block = tsfp_kmem_audio_heap_alloc(heap, 8080);
    assert_ne!(block, 0);
    let mut requested = 0;
    assert!(unsafe { tsfp_kmem_audio_heap_block_size(heap, block, &mut requested) });
    assert_eq!(requested, 8080);

    // The host operation API and direct audio ABI must address the same heap.
    let inputs = [heap, block];
    let mut from_host = 0;
    let found = unsafe {
        tsfp_kmem_op(
            tsfp_kmem_capi::ops::OP_HEAP_BLOCK_SIZE,
            inputs.as_ptr(),
            inputs.len() as u32,
            &mut from_host,
            1,
        )
    };
    assert_eq!((found, from_host), (1, 8080));
    let freed = unsafe {
        tsfp_kmem_op(
            tsfp_kmem_capi::ops::OP_HEAP_FREE,
            inputs.as_ptr(),
            inputs.len() as u32,
            std::ptr::null_mut(),
            0,
        )
    };
    assert_eq!(freed, 1);
    assert!(!unsafe { tsfp_kmem_audio_heap_block_size(heap, block, &mut requested) });

    assert!(tsfp_kmem_audio_heap_destroy(heap));
    assert!(!tsfp_kmem_audio_heap_valid(heap));
    assert!(tsfp_kmem_audio_heap_active());
}
