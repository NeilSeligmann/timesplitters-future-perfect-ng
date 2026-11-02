// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: wraps=tsfp-engine-render/draw
// portreg: wraps=tsfp-engine-render/effect
// portreg: wraps=tsfp-engine-render/effect_setup
// portreg: wraps=tsfp-engine-render/emitter
// portreg: wraps=tsfp-engine-render/lighting
// portreg: wraps=tsfp-engine-render/overlay
// portreg: wraps=tsfp-engine-render/particle
// portreg: wraps=tsfp-engine-render/scene
// portreg: wraps=tsfp-engine-render/state
// portreg: wraps=tsfp-engine-render/vertex
// portreg: mirrors=tsfp-ui/text_entry
//! T1976: lane adapters (Tier 2 through `tools.rust_trace`) for the 62 hand written render roots of T1898
//! (docs/t-render-slice.md). The pure render code is `crates/tsfp-engine-render/src/*.rs`, compiled here at the crate root by
//! `#[path]` (see `lib.rs`). Each adapter maps the entry register file and cdecl stack arguments of a root onto a `Renderer`
//! method, exactly as the T1898 differential shim (`tests/rust/render_diff_shim.rs`) does, and backs the two attachment
//! traits with the retained lifted code:
//!
//! * `RenderDevice` (the D3D8 library, `0x3D....`, stdcall or fastcall with ECX/EDX): `tsfp_lane_guest_call` with the call frame
//!   built here (the callee may pop its own arguments, so the strict `tsfp_lane_x_call` contract does not apply);
//! * `RenderHost` (the not yet ported game neighbours, cdecl): `tsfp_lane_x_call`, which also marshals x87/XMM and applies the
//!   lifted `RECOMP_ICALL` not-code rule for the function pointers read from the effect rows.
//!
//! Every root is cdecl with a plain `ret`, the port leaves ESP as entered (the host adapter pops the return slot). Only EAX (and
//! ST0 for the two float returning roots) is a result; other registers keep their entry values.
use core::cell::RefCell;

use crate::{
    Renderer,
    abi::{ExtRegisters, GuestCall},
    device::{HostArg, HostFunction, RenderDevice, RenderHost, ScissorRect},
    gp_bridge::SharedMemory,
    map::{DEVICE_DIRTY_MASK, DEVICE_STATE_SHADOW},
    memory::GuestMemory,
    particle, render_trace,
    state::{defer_state_in_guest, defer_texture_stage_word_in_guest, state_handler_entry},
};
use tsfp_port_scene_t2_div_scene_1::draw_model_node_tree;

#[cfg(all(not(test), not(feature = "std")))]
mod heap {
    //! The render modules allocate small `Vec`s (staged arguments). The `no_std` host archive has no allocator of its own,
    //! the host links libc, so `malloc` backs it.
    use core::alloc::{GlobalAlloc, Layout};
    unsafe extern "C" {
        fn malloc(size: usize) -> *mut u8;
        fn calloc(count: usize, size: usize) -> *mut u8;
        fn realloc(pointer: *mut u8, size: usize) -> *mut u8;
        fn free(pointer: *mut u8);
        fn aligned_alloc(alignment: usize, size: usize) -> *mut u8;
    }
    struct Libc;
    unsafe impl GlobalAlloc for Libc {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            if layout.align() <= 16 {
                unsafe { malloc(layout.size()) }
            } else {
                unsafe { aligned_alloc(layout.align(), layout.size()) }
            }
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            if layout.align() <= 16 {
                unsafe { calloc(1, layout.size()) }
            } else {
                let pointer = unsafe { aligned_alloc(layout.align(), layout.size()) };
                if !pointer.is_null() {
                    unsafe { core::ptr::write_bytes(pointer, 0, layout.size()) }
                }
                pointer
            }
        }
        unsafe fn dealloc(&self, pointer: *mut u8, _: Layout) {
            unsafe { free(pointer) }
        }
        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            if layout.align() <= 16 {
                unsafe { realloc(pointer, size) }
            } else {
                let new =
                    unsafe { self.alloc(Layout::from_size_align_unchecked(size, layout.align())) };
                if !new.is_null() {
                    unsafe {
                        core::ptr::copy_nonoverlapping(pointer, new, layout.size().min(size));
                        free(pointer)
                    }
                }
                new
            }
        }
    }
    #[global_allocator]
    static ALLOCATOR: Libc = Libc;
}

unsafe extern "C" {
    /// Runs the lifted callee of `target`; `regs` is eax, ecx, edx, ebx, esi, edi, ebp, esp in and out (src/host/rust_lane_call.c).
    fn tsfp_lane_guest_call(target: u32, regs: *mut u32);
    fn tsfp_lane_x_fp_load(regs: *mut ExtRegisters);
    fn tsfp_lane_x_fp_store(regs: *const ExtRegisters);
    /// cdecl callee with the whole `ExtRegisters` record; requires the callee to return with `esp == entry + 4`.
    fn tsfp_lane_x_call(va: u32, regs: *mut ExtRegisters);
}

/// Scratch below the entry ESP: vector staging, host buffers, then the call frames.
const DEVICE_STAGE: u32 = 0xE00;
const HOST_STAGE: u32 = 0xB00;
const FRAME_DEPTH: u32 = 0x1000;

/// Register and x87/XMM state shared by the device and the host (they call the same lifted code).
struct Calls {
    regs: ExtRegisters,
    entry_esp: u32,
}

impl Calls {
    /// Writes `args` below the entry frame and returns the address of the first.
    fn frame<M: GuestMemory>(&self, mem: &mut M, args: &[u32]) -> u32 {
        let block = self
            .entry_esp
            .wrapping_sub(FRAME_DEPTH)
            .wrapping_sub(4 * args.len() as u32);
        for (index, arg) in args.iter().enumerate() {
            mem.write32(block.wrapping_add(4 * index as u32), *arg);
        }
        block
    }

    /// cdecl call through the strict x87/XMM bridge; the callee-saved registers keep their entry values.
    fn cdecl<M: GuestMemory>(
        &mut self,
        mem: &mut M,
        target: u32,
        esi: Option<u32>,
        args: &[u32],
    ) -> u32 {
        let block = self.frame(mem, args);
        let saved = (self.regs.ebx, self.regs.ebp, self.regs.esi, self.regs.edi);
        self.regs.esp = block;
        if let Some(value) = esi {
            self.regs.esi = value;
        }
        unsafe { tsfp_lane_x_call(target, &mut self.regs) };
        let result = self.regs.eax;
        (self.regs.ebx, self.regs.ebp, self.regs.esi, self.regs.edi) = saved;
        result
    }

    /// Call whose callee may pop its own arguments (stdcall) or take ECX/EDX (fastcall): the frame is built here and ESP after
    /// the call is not checked.
    fn raw<M: GuestMemory>(
        &mut self,
        mem: &mut M,
        target: u32,
        ecx: u32,
        edx: u32,
        args: &[u32],
    ) -> u32 {
        let block = self.frame(mem, args);
        let esp = block.wrapping_sub(4);
        mem.write32(esp, 0);
        let mut file = [
            self.regs.eax,
            ecx,
            edx,
            self.regs.ebx,
            self.regs.esi,
            self.regs.edi,
            self.regs.ebp,
            esp,
        ];
        unsafe {
            tsfp_lane_x_fp_store(&self.regs);
            tsfp_lane_guest_call(target, file.as_mut_ptr());
            tsfp_lane_x_fp_load(&mut self.regs);
        }
        file[0]
    }
}

/// `RenderDevice` over the lifted D3D8 library.
struct Device<'a, M: GuestMemory> {
    mem: SharedMemory<'a, M>,
    calls: &'a RefCell<Calls>,
    scratch: u32,
}

impl<M: GuestMemory> Device<'_, M> {
    fn invoke(&mut self, va: u32, ecx: u32, edx: u32, args: &[u32]) -> u32 {
        self.calls
            .borrow_mut()
            .raw(&mut self.mem, va, ecx, edx, args)
    }
    /// Copies `words` to guest scratch space below the stack and returns its address.
    fn stage(&mut self, words: &[u32]) -> u32 {
        for (index, word) in words.iter().enumerate() {
            self.mem
                .write32(self.scratch.wrapping_add(4 * index as u32), *word);
        }
        self.scratch
    }
}

impl<M: GuestMemory> RenderDevice for Device<'_, M> {
    fn push_method(&mut self, header: u32, value: u32) {
        let tracing = render_trace::enabled();
        if tracing {
            render_trace::call("PushMethod", &[header, value]);
        }
        let native_enabled = tracing && render_trace::native_push_method_enabled();
        if native_push_method_bypass(tracing, native_enabled) {
            return;
        }
        self.invoke(0x3D6C90, header, value, &[]);
    }
    fn defer_state(&mut self, index: u32, value: u32) {
        if render_trace::enabled() {
            render_trace::call("DeferState", &[index, value]);
        }
        defer_state_in_guest(&mut self.mem, index, value);
    }
    fn state_handler(&mut self, index: u32, value: u32) {
        let tracing = render_trace::enabled();
        if tracing {
            render_trace::call("StateHandler", &[index, value]);
        }
        let native_enabled = tracing && render_trace::native_state_handler_enabled(index);
        if tsfp_port_engine_render_d3d::native_state::handler_bypass(tracing, native_enabled, index)
        {
            let front_fill = self.mem.read32(DEVICE_STATE_SHADOW.wrapping_add(0x8B * 4));
            let back_fill = self.mem.read32(DEVICE_STATE_SHADOW.wrapping_add(0x8C * 4));
            let two_sided = self.mem.read32(DEVICE_STATE_SHADOW.wrapping_add(0x8D * 4));
            let state_shadow = DEVICE_STATE_SHADOW.wrapping_add(index.wrapping_mul(4));
            tsfp_port_engine_render_d3d::native_state::apply_native_state_handler(
                index,
                value,
                front_fill,
                back_fill,
                two_sided,
                &mut self.mem,
                DEVICE_DIRTY_MASK,
                state_shadow,
                DEVICE_STATE_SHADOW.wrapping_add(0x8B * 4),
                |header, method_value| render_trace::call("PushMethod", &[header, method_value]),
            );
            return;
        }
        if let Some(entry) = state_handler_entry(index) {
            self.invoke(entry, 0, 0, &[value]);
        }
    }
    fn defer_texture_stage_word(&mut self, stage: u32, word: u32, value: u32) {
        if render_trace::enabled() {
            render_trace::call("DeferTextureStageWord", &[stage, word, value]);
        }
        defer_texture_stage_word_in_guest(&mut self.mem, stage, word, value);
    }
    fn set_texture(&mut self, stage: u32, texture: u32) {
        if render_trace::enabled() {
            render_trace::texture_record(&self.mem, texture);
            render_trace::call("SetTexture", &[stage, texture]);
        }
        self.invoke(0x3D4070, 0, 0, &[stage, texture]);
    }
    fn set_vertex_constants(&mut self, first_register: u32, words: &[u32]) {
        if render_trace::enabled() {
            let mut all = alloc::vec![first_register];
            all.extend_from_slice(words);
            render_trace::call("SetVertexConstants", &all);
        }
        let source = self.stage(words);
        self.invoke(0x3D58B0, first_register, source, &[words.len() as u32]);
    }
    fn set_vertex_constants_uncached(&mut self, first_register: u32, words: &[u32]) {
        if render_trace::enabled() {
            let mut all = alloc::vec![first_register];
            all.extend_from_slice(words);
            render_trace::call("SetVertexConstantsUncached", &all);
        }
        let source = self.stage(words);
        self.invoke(0x3D57D0, first_register, source, &[words.len() as u32]);
    }
    fn set_vertex_constant_one_vector(&mut self, first_register: u32, words: &[u32]) {
        if render_trace::enabled() {
            let mut all = alloc::vec![first_register];
            all.extend_from_slice(words);
            render_trace::call("SetVertexConstantOneVector", &all);
        }
        let source = self.stage(words);
        self.invoke(0x3D5670, first_register, source, &[]);
    }
    fn set_vertex_constant_four_vectors(&mut self, first_register: u32, words: &[u32]) {
        if render_trace::enabled() {
            let mut all = alloc::vec![first_register];
            all.extend_from_slice(words);
            render_trace::call("SetVertexConstantFourVectors", &all);
        }
        let source = self.stage(words);
        self.invoke(0x3D5720, first_register, source, &[]);
    }
    fn load_vertex_program(&mut self, program: u32, address: u32) {
        if render_trace::enabled() {
            render_trace::program(&self.mem, program);
            render_trace::call("LoadVertexProgram", &[program, address]);
        }
        self.invoke(0x3D59E0, 0, 0, &[program, address]);
    }
    fn select_vertex_declaration(&mut self, declaration: u32, address: u32) {
        if render_trace::enabled() {
            render_trace::declaration(&self.mem, declaration);
            render_trace::call("SelectVertexDeclaration", &[declaration, address]);
        }
        self.invoke(0x3D5630, 0, 0, &[declaration, address]);
    }
    fn set_pixel_shader(&mut self, program: u32) {
        if render_trace::enabled() {
            render_trace::pixel_shader(&self.mem, program);
            render_trace::call("SetPixelShader", &[program]);
        }
        self.invoke(0x3D92E0, 0, 0, &[program]);
    }
    fn draw_vertices(&mut self, primitive: u32, start: u32, count: u32) {
        if render_trace::enabled() && !render_trace::dropping() {
            render_trace::draw_vertices(&self.mem, start, count);
            render_trace::call("DrawVertices", &[primitive, start, count]);
        }
        self.invoke(0x3D4FB0, 0, 0, &[primitive, start, count]);
    }
    fn draw_indexed_vertices(&mut self, primitive: u32, count: u32, indices: u32) {
        if render_trace::enabled() && !render_trace::dropping() {
            render_trace::draw_indexed(&self.mem, count, indices);
            render_trace::call("DrawIndexedVertices", &[primitive, count, indices]);
        }
        self.invoke(0x3D5050, 0, 0, &[primitive, count, indices]);
    }
    fn set_scissor(&mut self, count: u32, flags: u32, rects: &[ScissorRect]) {
        if render_trace::enabled() {
            render_trace::scissor(count, flags, rects);
        }
        let words: alloc::vec::Vec<u32> =
            rects.iter().flatten().map(|value| *value as u32).collect();
        let source = self.stage(&words);
        self.invoke(0x3D4470, 0, 0, &[count, flags, source]);
    }
    fn set_vertex_bindings(&mut self, formats: [u32; 16], offsets: [u32; 16]) {
        let tracing = render_trace::enabled();
        let native_enabled = tracing && render_trace::native_vertex_bindings_enabled();
        if native_vertex_bindings_bypass(tracing, native_enabled) {
            // BeginPush flushes the D3D8 deferred state before the engine writes its two array
            // runs. Keep that flush while the typed operation replaces the raw 34-dword packet.
            let cursor = self.begin_push(0);
            self.end_push(cursor);
            render_trace::native_vertex_bindings(formats, offsets);
            return;
        }

        // C remains the default oracle path: preserve the original pair of auto-incrementing
        // NV2A method runs in the guest push-buffer reservation.
        let cursor = self.begin_push(0x22);
        self.mem.write32(cursor, 0x40_1760);
        for (index, format) in formats.iter().enumerate() {
            self.mem
                .write32(cursor.wrapping_add(4 + 4 * index as u32), *format);
        }
        self.mem.write32(cursor.wrapping_add(0x44), 0x40_1720);
        for (index, offset) in offsets.iter().enumerate() {
            self.mem
                .write32(cursor.wrapping_add(0x48 + 4 * index as u32), *offset);
        }
        self.end_push(cursor.wrapping_add(0x88));
    }
    fn insert_fence(&mut self) -> u32 {
        if render_trace::enabled() {
            render_trace::call("InsertFence", &[]);
        }
        self.invoke(0x3D3630, 0, 0, &[])
    }
    fn block_until_fence(&mut self, fence: u32) {
        if render_trace::enabled() {
            render_trace::call("BlockUntilFence", &[fence]);
        }
        self.invoke(0x3D34B0, 0, 0, &[fence]);
    }
    fn lock_vertex_buffer(&mut self, buffer: u32, flags: u32) -> u32 {
        if render_trace::enabled() {
            render_trace::call("LockVertexBuffer", &[buffer, flags]);
        }
        let data = self.invoke(0x3D4F30, 0, 0, &[buffer, flags]);
        if render_trace::enabled() {
            render_trace::lock_result(buffer, data);
        }
        data
    }
    fn set_pixel_constants(&mut self, index: u32, vectors: &[[f32; 4]]) {
        if render_trace::enabled() {
            let mut all = alloc::vec![index];
            all.extend(render_trace::float_words(vectors));
            render_trace::call("SetPixelConstants", &all);
        }
        let words: alloc::vec::Vec<u32> = vectors
            .iter()
            .flatten()
            .map(|value| value.to_bits())
            .collect();
        let source = self.stage(&words);
        self.invoke(0x3D9520, 0, 0, &[index, source, vectors.len() as u32]);
    }
    fn begin_push(&mut self, dwords: u32) -> u32 {
        if render_trace::enabled() {
            render_trace::call("BeginPush", &[dwords]);
        }
        let cursor = self.invoke(0x3D6660, 0, 0, &[dwords]);
        if render_trace::enabled() {
            render_trace::push_begin(cursor);
        }
        cursor
    }
    fn end_push(&mut self, cursor: u32) {
        if render_trace::enabled() {
            render_trace::call("EndPush", &[cursor]);
            render_trace::push_end(&self.mem, cursor);
        }
        self.invoke(0x3D6680, 0, 0, &[cursor]);
    }
}

fn native_push_method_bypass(trace_enabled: bool, native_enabled: bool) -> bool {
    trace_enabled && native_enabled
}

fn native_vertex_bindings_bypass(trace_enabled: bool, native_enabled: bool) -> bool {
    trace_enabled && native_enabled
}

#[cfg(test)]
mod native_push_method_tests {
    use super::{native_push_method_bypass, native_vertex_bindings_bypass};

    #[test]
    fn native_push_method_requires_the_binary_trace_path_and_opt_in() {
        assert!(!native_push_method_bypass(false, false));
        assert!(!native_push_method_bypass(false, true));
        assert!(!native_push_method_bypass(true, false));
        assert!(native_push_method_bypass(true, true));
    }

    #[test]
    fn native_vertex_bindings_require_the_binary_trace_path_and_opt_in() {
        assert!(!native_vertex_bindings_bypass(false, false));
        assert!(!native_vertex_bindings_bypass(false, true));
        assert!(!native_vertex_bindings_bypass(true, false));
        assert!(native_vertex_bindings_bypass(true, true));
    }
}

/// `RenderHost` over the lifted neighbours of other systems.
struct Host<'a, M: GuestMemory> {
    mem: SharedMemory<'a, M>,
    calls: &'a RefCell<Calls>,
    scratch: u32,
}

impl<M: GuestMemory> RenderHost for Host<'_, M> {
    fn call(&mut self, function: HostFunction, args: &[u32]) -> u32 {
        if render_trace::enabled() {
            if render_trace::fast_hooks() {
                // Borrow once: tracing through SharedMemory otherwise repeats a RefCell
                // borrow per byte and hides MappedGuest's aligned dword access.
                let memory = self.mem.0.borrow();
                render_trace::host(&**memory, function as u32, args);
            } else {
                render_trace::host(&self.mem, function as u32, args);
            }
        }
        let (esi, args) = match function {
            HostFunction::ResolveModelField => (Some(args[0]), &[][..]),
            _ => (None, args),
        };
        self.calls
            .borrow_mut()
            .cdecl(&mut self.mem, function as u32, esi, args)
    }
    fn call_buffers(
        &mut self,
        function: HostFunction,
        args: &[HostArg],
        buffers: &mut [alloc::vec::Vec<u32>],
    ) -> u32 {
        if render_trace::enabled() {
            render_trace::host(&self.mem, function as u32, &[]);
        }
        let mut addresses = alloc::vec::Vec::new();
        let mut next = self.scratch;
        for buffer in buffers.iter() {
            addresses.push(next);
            for (index, word) in buffer.iter().enumerate() {
                self.mem.write32(next.wrapping_add(4 * index as u32), *word);
            }
            next = next.wrapping_add(4 * buffer.len() as u32);
        }
        let words: alloc::vec::Vec<u32> = args
            .iter()
            .map(|arg| match *arg {
                HostArg::Word(word) => word,
                HostArg::Buffer(index) => addresses[index],
            })
            .collect();
        let result = self
            .calls
            .borrow_mut()
            .cdecl(&mut self.mem, function as u32, None, &words);
        for (buffer, address) in buffers.iter_mut().zip(addresses) {
            for (index, word) in buffer.iter_mut().enumerate() {
                *word = self.mem.read32(address.wrapping_add(4 * index as u32));
            }
        }
        result
    }
    fn call_address(&mut self, target: u32, args: &[u32]) -> u32 {
        if render_trace::enabled() {
            render_trace::host(&self.mem, target, &[]);
        }
        self.calls
            .borrow_mut()
            .cdecl(&mut self.mem, target, None, args)
    }
}

/// The render lane roots in registration order (appended to `lane_inv::INV_ROOTS`).
pub const ROOTS: [u32; 62] = [
    0x21440, 0x21450, 0x214c0, 0x21740, 0x22560, 0x22790, 0x2a630, 0x22950, 0x226e0, 0x22720,
    0x204b0, 0x22ff0, 0x1a010, 0x19640, 0x19d50, 0x19d60, 0x19d40, 0x19650, 0x19d70, 0x8f470,
    0x8f490, 0x8f4e0, 0x1e770, 0x1e7d0, 0x1e7f0, 0x1e970, 0x1eb00, 0x18de0, 0x18ea0, 0x1b7b0,
    0x1d5f0, 0x1ed50, 0x8e710, 0x8f5e0, 0xa2740, 0x10c390, 0x10c400, 0x10c550, 0x10d070, 0x22be0,
    0x2a2d0, 0x2a310, 0x2a320, 0x2a340, 0x2a360, 0x1d380, 0x389b0, 0x459d0, 0x1c720, 0x19cc0,
    0x1ed60, 0x1a2e0, 0x1afa0, 0x1b090, 0x1efb0, 0x2a900, 0x2abe0, 0x2ad60, 0x18e910, 0xffe50,
    0x153bd0, 0x10e410,
];

/// `0x19240` (vertex stream source by slot, cdecl: slot, stride): one stdcall call of the D3D8 library `0x3D58F0(0, buffer, stride)`
/// with `buffer = [slot * 8 + 0x54F16C]`. The `tsfp.c2r` translation (`c2r/t1976_render.rs`, Tier 1) is the reference; the strict
/// `GuestCall` bridge cannot host the callee that pops its own arguments, so the call goes through the device path here.
pub const STREAM_SOURCE_ROOT: u32 = 0x19240;

/// Dispatches a root; `false` when `va` is not one of the render adapters.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    _guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if va == 0x153BD0 || va == 0x10E410 {
        return tsfp_port_engine_render_effects_live_return::run(va, m, regs);
    }
    if va == STREAM_SOURCE_ROOT {
        let entry_esp = regs.esp;
        let slot = m.read32(entry_esp.wrapping_add(4));
        let stride = m.read32(entry_esp.wrapping_add(8));
        let buffer = m.read32((slot & 0xff).wrapping_mul(8).wrapping_add(0x54f16c));
        let (ecx, edx) = (slot & 0xff, buffer);
        let mut calls = Calls {
            regs: *regs,
            entry_esp,
        };
        let result = calls.raw(m, 0x3D58F0, ecx, edx, &[0, buffer, stride]);
        regs.ecx = ecx;
        regs.edx = edx;
        regs.eax = result;
        regs.fpu = calls.regs.fpu;
        regs.xmm = calls.regs.xmm;
        regs.xmm_hi = calls.regs.xmm_hi;
        return true;
    }
    if !ROOTS.contains(&va) {
        return false;
    }
    let entry_esp = regs.esp;
    let stack = |memory: &M, index: u32| memory.read32(entry_esp.wrapping_add(4 + 4 * index));
    let a = [stack(m, 0), stack(m, 1), stack(m, 2), stack(m, 3)];
    let a8 = [
        stack(m, 4),
        stack(m, 5),
        stack(m, 6),
        stack(m, 7),
        stack(m, 8),
    ];
    let (eax_in, ecx_in, edx_in) = (regs.eax, regs.ecx, regs.edx);
    let (ebx_in, esi_in, edi_in) = (regs.ebx, regs.esi, regs.edi);
    let cell = RefCell::new(&mut *m);
    let calls = RefCell::new(Calls {
        regs: *regs,
        entry_esp,
    });
    let mut renderer_memory = SharedMemory(&cell);
    let mut device = Device {
        mem: SharedMemory(&cell),
        calls: &calls,
        scratch: entry_esp.wrapping_sub(DEVICE_STAGE),
    };
    let mut host = Host {
        mem: SharedMemory(&cell),
        calls: &calls,
        scratch: entry_esp.wrapping_sub(HOST_STAGE),
    };
    let mut renderer = Renderer::new(&mut renderer_memory, &mut device, &mut host);
    let mut result: Option<u32> = None;
    let mut st0: Option<f64> = None;
    match va {
        0x21440 => result = Some(renderer.state_slot_pending(a[0])),
        0x21450 => result = Some(renderer.state_slot_set(a[0], a[1])),
        0x214C0 => result = Some(renderer.state_apply()),
        0x21740 => result = Some(u32::from(renderer.dirty_list_has_entries())),
        0x22560 => renderer.refresh_blend_methods(),
        0x22790 => renderer.bind_texture_with_constants(a[0], a[1]),
        0x2A630 => renderer.bind_texture_by_index(eax_in, edx_in),
        0x22950 => result = Some(renderer.texture_pair_matches()),
        0x226E0 => renderer.load_vertex_shader(a[0], a[1], a[2]),
        0x22720 => renderer.set_pixel_program(a[0]),
        0x204B0 => renderer.select_vertex_format(a[0]),
        0x22FF0 => renderer.draw_dispatch(),
        0x1A010 => renderer.set_scissor_clamped(a[0] as i32, a[1] as i32, a[2] as i32, a[3] as i32),
        0x19640 => result = Some(renderer.state_group()),
        0x19D50 => result = Some(renderer.blend_mode()),
        0x19D60 => result = Some(renderer.blend_alpha()),
        0x19D40 => renderer.set_blend_alpha(a[0]),
        0x19650 => result = Some(renderer.set_state_group(a[0])),
        0x19D70 => result = Some(renderer.set_blend_mode(a[0])),
        0x8F470 => renderer.save_state_group_blend_mode_and_alpha(),
        0x8F490 => renderer.restore_saved_state_group_and_blend_mode(),
        0x8F4E0 => renderer.effect_apply_blend_and_state(a[0], a[1]),
        0x1E770 => result = Some(crate::vertex::primitive_type_for_kind(eax_in)),
        0x1E7D0 => result = Some(renderer.draw_indexed_kind(a[0], a[1], a[2])),
        0x1E7F0 => renderer.set_stream_binding(a[0], a[1], a[2] as u8),
        0x1E970 => renderer.build_vertex_declaration(a[0], a[1]),
        0x1EB00 => renderer.emit_vertex_bindings(a[0]),
        0x18DE0 => {
            let (data, first_index) = renderer.lock_vertex_ring(a[0], a[1]);
            renderer.mem.write32(a[2], first_index);
            result = Some(data);
        }
        0x18EA0 => result = Some(renderer.begin_primitive_batch(a[0], a[1], a[2], a[3])),
        0x1B7B0 => renderer.draw_textured_rect(
            f32::from_bits(a[0]),
            f32::from_bits(a[1]),
            f32::from_bits(a[2]),
            f32::from_bits(a[3]),
            a8[0] as i32,
            a8[1] as i32,
            a8[2] as i32,
            a8[3] as i32,
            a8[4],
        ),
        0x1D5F0 => renderer.draw_point_sprite(
            f32::from_bits(a[0]),
            f32::from_bits(a[1]),
            f32::from_bits(a[2]),
            a[3],
            f32::from_bits(a8[0]),
            f32::from_bits(a8[1]),
        ),
        0x1ED50 | 0x1C720 => renderer.select_current_texture_record(a[0]),
        0x19CC0 => result = Some(renderer.set_modulate_colour(a[0])),
        0x1ED60 => result = Some(renderer.set_colour_preset(a[0])),
        0x1A2E0 => renderer.draw_flat_quad(
            f32::from_bits(a[0]),
            f32::from_bits(a[1]),
            f32::from_bits(a[2]),
            f32::from_bits(a[3]),
            a8[0],
        ),
        0x1AFA0 => renderer.draw_flat_line(
            f32::from_bits(a[0]),
            f32::from_bits(a[1]),
            f32::from_bits(a[2]),
            f32::from_bits(a[3]),
            a8[0],
        ),
        0x1B090 => {
            renderer.draw_line_pixels(a[0] as i32, a[1] as i32, a[2] as i32, a[3] as i32, a8[0])
        }
        0x1EFB0 => renderer.fill_rect(a[0] as i32, a[1] as i32, a[2] as i32, a[3] as i32, a8[0]),
        0x2A900 => renderer.effect_setup_texture_and_constant_44(),
        0x2ABE0 => renderer.effect_setup_variant_b(),
        0x2AD60 => renderer.effect_setup_variant_transposed(),
        0x18E910 => {
            result = Some(lighting_t2::fill_light_slot!(renderer, regs, a, edx_in));
        }
        0xFFE50 => crate::particle::advance_pool_record_point(renderer.mem, a[0]),
        0x22BE0 => renderer.prepare_draw(),
        0x2A2D0 => renderer.select_effect_row(a[0]),
        0x2A310 => renderer.set_effect_pass(a[0]),
        0x2A320 => result = Some(renderer.end_effect_pass()),
        0x2A340 => result = Some(renderer.effect_pass_middle()),
        0x2A360 => result = Some(renderer.begin_effect_pass()),
        0x1D380 => renderer.batch_cleanup(),
        0x389B0 | 0x459D0 => {}
        0x8E710 => result = Some(renderer.record_resolved_field(a[0])),
        0x8F5E0 => draw_model_node_tree!(renderer, cell, entry_esp, a, regs),
        0xA2740 => renderer.draw_scene_object(a[0], a[1]),
        0x10D070 => renderer.step_particle(ebx_in, esi_in, f32::from_bits(a[0])),
        0x10C390 => {
            let value = particle::curve_sample(&*renderer.mem, f32::from_bits(a[0]), a[1], a[2]);
            st0 = Some(value);
        }
        0x10C400 => {
            result = Some(particle::particle_colour_step(
                &*renderer.mem,
                eax_in,
                ecx_in,
            ))
        }
        0x10C550 => result = Some(particle::particle_colour(&*renderer.mem, a[0], a[1])),
        _ => return false,
    }
    drop(renderer);
    let state = calls.into_inner().regs;
    regs.fpu = state.fpu;
    regs.xmm = state.xmm;
    regs.xmm_hi = state.xmm_hi;
    if let Some(value) = result {
        regs.eax = value;
    }
    if let Some(value) = st0 {
        regs.fpu.push(value);
    }
    true
}
