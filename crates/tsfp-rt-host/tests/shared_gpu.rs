// SPDX-License-Identifier: GPL-3.0-or-later
use tsfp_rt_host::ffi::*;
use tsfp_wgpu::context::{GpuContext, Preference};

#[test]
fn shared_ffi_adopts_cloned_handles_and_rejects_live_replacement() {
    let gpu = GpuContext::new(&Preference::Software).unwrap();
    let mut layout = [0; 6];
    unsafe {
        tsfp_rt_host_share_layout(layout.as_mut_ptr());
    }
    assert_eq!(layout[1], size_of::<wgpu::Device>() as u64);
    assert_eq!(layout[5], 1);
    assert_eq!(
        unsafe { tsfp_rt_host_share_gpu(&gpu.device, &gpu.queue, c"test software".as_ptr()) },
        0
    );
    drop(gpu);
    let runtime =
        unsafe { tsfp_rt_host_create(c"--publish --opt pipelined_readback=1 --no-skip".as_ptr()) };
    assert!(!runtime.is_null());
    let replacement = GpuContext::new(&Preference::Software).unwrap();
    assert_eq!(
        unsafe {
            tsfp_rt_host_share_gpu(&replacement.device, &replacement.queue, std::ptr::null())
        },
        -1
    );
    let mut frame = TsfpSharedFrame {
        texture: std::ptr::null(),
        width: 0,
        height: 0,
        present: 0,
        slot: 0,
    };
    assert_eq!(
        unsafe { tsfp_rt_host_shared_acquire(runtime, &mut frame) },
        0
    );
    let bytes = b"# present 1\npresent\n";
    unsafe {
        tsfp_rt_host_write(runtime, bytes.as_ptr(), bytes.len());
    }
    // Flush producer bytes, then await the render thread without needing game data.
    unsafe {
        tsfp_rt_host_flush(runtime);
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if unsafe { tsfp_rt_host_shared_acquire(runtime, &mut frame) } == 1 {
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(frame.present, 1);
    assert!(!frame.texture.is_null());
    assert_eq!(
        unsafe { tsfp_rt_host_shared_acquire(runtime, &mut frame) },
        0
    );
    let stats = unsafe { &*runtime }.stats();
    assert_eq!(stats.readback_waits, 0);
    assert!(unsafe { &*runtime }.take_frame().is_none());
    unsafe {
        tsfp_rt_host_shared_release(runtime, frame.slot);
        tsfp_rt_host_destroy(runtime);
    }
}
