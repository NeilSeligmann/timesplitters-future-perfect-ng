// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: wraps=tsfp-gameplay/char_tick_class_a
// portreg: wraps=tsfp-gameplay/char_tick_idle_variant
// portreg: wraps=tsfp-gameplay/char_tick_idle_variation
// portreg: wraps=tsfp-ports/c2r/t1926_characters
//! T1971: guest-memory lane adapters for the readable NPC tick ports of T1964 (Tier 0 here, Tier 2 needs `tools.rust_trace`).
//! The pure ports (`tsfp-gameplay/src/char_tick_*.rs`) are compiled here by `#[path]`. Each adapter loads the actor context
//! (`ctx = [obj+0x7C]`) from guest memory into the port's context struct, implements the port's `*World` leaf trait over the
//! retained original callees (`HostBridge::call_cdecl`, arguments recovered from the original call sites), runs the tick and
//! writes back only the fields the port changed. Leaves that take `&mut ctx` (awareness, navigate, walk, request) flush the
//! changed fields before the guest call and reload the whole context after it, because the originals read and write it.
//! Roots: 0x265CC0 (class A follower), 0x283730 (idle variant), 0x2621A0 (idle variation, usercall EBX = obj, args refresh and
//! source on the stack). 0x26E150 (route step, EAX/EBX register ABI, 17 register-argument leaves) and 0x2AEFD0 (low class, 730
//! line port) are NOT adapted yet (debt, docs/t1964-npc-ticks.md). EAX is left as entered (the ticks return nothing the
//! callers use, the trace compares memory only). The original `ret` is plain (no callee pop), checked on all five.
use crate::{
    abi::{ExtRegisters, GuestCall},
    gp_bridge::HostBridge,
    memory::GuestMemory,
};
use core::cell::RefCell;

#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-gameplay/src/char_tick_class_a.rs"]
mod class_a;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-gameplay/src/char_tick_idle_variant.rs"]
mod idle_variant;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-gameplay/src/char_tick_idle_variation.rs"]
mod idle_variation;

const TICK_RATE: u32 = 0x4e_8cec;
const TIME_SCALE: u32 = 0x4e_794c;
const WEAPON_TABLE: u32 = 0x4f_9bac;
const WEAPON_STRIDE: u32 = 0x29c;
/// Per-call random draw is the guest `0x153B40` itself, nothing to model here.
const RANDOM: u32 = 0x15_3b40;

type Host<'a, M, G> = HostBridge<'a, M, G>;

/// Reads `[base + offset]` through a pointer the original only follows on some paths: an unusable pointer (null, small, wild) gives 0
/// instead of a fault. The loaders snapshot these fields eagerly, the original reads them only when the state needs them (T1996).
fn word<M: GuestMemory>(m: &M, base: u32, offset: u32) -> u32 {
    if crate::anim_lane::guest_ok(base) {
        m.read32(base.wrapping_add(offset))
    } else {
        0
    }
}
/// [`word`] for a float.
fn word_float<M: GuestMemory>(m: &M, base: u32, offset: u32) -> f32 {
    f32::from_bits(word(m, base, offset))
}

fn flag(value: u32) -> bool {
    value & 0xff != 0
}
fn float<M: GuestMemory>(m: &M, address: u32) -> f32 {
    f32::from_bits(m.read32(address))
}
/// Writes `value` at `address` when it differs from the last synchronised value.
fn put<M: GuestMemory>(m: &mut M, address: u32, old: u32, new: u32) {
    if old != new {
        m.write32(address, new);
    }
}

/// Retained original leaves shared by the three adapters (all cdecl, arguments as recovered from the call sites).
struct Leaves<'h, 'a, M: GuestMemory, G: GuestCall> {
    host: &'h mut Host<'a, M, G>,
    obj: u32,
    ctx: u32,
}
impl<M: GuestMemory, G: GuestCall> Leaves<'_, '_, M, G> {
    fn plays(&mut self, id: u32) -> bool {
        flag(self.host.call_cdecl(0x59d20, &[self.obj, id]))
    }
    fn transition(&mut self, id: u32) -> bool {
        flag(self.host.call_cdecl(0x59db0, &[self.obj, id]))
    }
    /// `0x59DD0(obj+4)`: the animation info record pointer.
    fn finished(&mut self) -> bool {
        let info = self.host.read32(self.obj.wrapping_add(4));
        flag(self.host.call_cdecl(0x59dd0, &[info]))
    }
    fn play(&mut self, id: u32, blend: f32, speed: f32) {
        self.host
            .call_cdecl(0x5f460, &[self.obj, id, blend.to_bits(), speed.to_bits()]);
    }
    fn rng_raw(&mut self) -> u32 {
        self.host.call_cdecl(RANDOM, &[])
    }
    fn awareness(&mut self) -> bool {
        flag(self.host.call_cdecl(0x15e120, &[self.ctx, 1]))
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// 0x265CC0 class A follower
// ---------------------------------------------------------------------------------------------------------------------

fn load_class_a<M: GuestMemory>(m: &M, obj: u32) -> (u32, class_a::ClassACtx) {
    let e = m.read32(obj.wrapping_add(0x7c));
    let word_a = m.read32(e.wrapping_add(0xacc));
    let word_b = m.read32(e.wrapping_add(0x67c));
    let ctx = class_a::ClassACtx {
        state: m.read32(e.wrapping_add(0x20)),
        pending: m.read32(e.wrapping_add(0x9d0)) as i32,
        previous: m.read32(e.wrapping_add(0x9d4)),
        state_time: float(m, e.wrapping_add(0x9d8)),
        flags_byte: m.read8(e.wrapping_add(0x2c)),
        timer: float(m, e.wrapping_add(0xa90)),
        tick_rate: m.read32(TICK_RATE) as i32,
        time_scale: float(m, TIME_SCALE),
        selector: m.read32(e.wrapping_add(8)),
        node: m.read32(e.wrapping_add(0x678)) as i32,
        target_node: m.read32(e.wrapping_add(0x684)) as i32,
        clear_a: m.read32(e.wrapping_add(0x9ac)),
        clear_b: m.read32(e.wrapping_add(0x9a8)),
        // `[E+0xACC]+0x18` when the node exists, else `[E+0x67C]+0x18` (the port picks the second when `None`).
        route_word_a: (word_a != 0).then(|| word(m, word_a, 0x18)),
        route_word_b: word(m, word_b, 0x18),
        arrival_index: m.read32(obj.wrapping_add(0x1a0)),
    };
    (e, ctx)
}

fn store_class_a<M: GuestMemory>(
    m: &mut M,
    obj: u32,
    e: u32,
    old: &class_a::ClassACtx,
    new: &class_a::ClassACtx,
) {
    put(m, e.wrapping_add(0x20), old.state, new.state);
    put(
        m,
        e.wrapping_add(0x9d0),
        old.pending as u32,
        new.pending as u32,
    );
    put(m, e.wrapping_add(0x9d4), old.previous, new.previous);
    put(
        m,
        e.wrapping_add(0x9d8),
        old.state_time.to_bits(),
        new.state_time.to_bits(),
    );
    put(
        m,
        e.wrapping_add(0xa90),
        old.timer.to_bits(),
        new.timer.to_bits(),
    );
    put(m, e.wrapping_add(0x678), old.node as u32, new.node as u32);
    put(
        m,
        e.wrapping_add(0x684),
        old.target_node as u32,
        new.target_node as u32,
    );
    put(m, e.wrapping_add(0x9ac), old.clear_a, new.clear_a);
    put(m, e.wrapping_add(0x9a8), old.clear_b, new.clear_b);
    put(
        m,
        obj.wrapping_add(0x1a0),
        old.arrival_index,
        new.arrival_index,
    );
    if old.flags_byte != new.flags_byte {
        m.write8(e.wrapping_add(0x2c), new.flags_byte);
    }
}

struct ClassAWorld<'h, 'a, M: GuestMemory, G: GuestCall> {
    leaves: Leaves<'h, 'a, M, G>,
    /// Context as last synchronised with guest memory (flush diff base).
    synced: class_a::ClassACtx,
}

impl<M: GuestMemory, G: GuestCall> ClassAWorld<'_, '_, M, G> {
    /// Flushes the port's changed fields, runs the guest leaf, then reloads the whole context (T1972): the originals
    /// `0x15E120`, `0xDA280` and `0x159300` read and write the actor context.
    fn with_guest(
        &mut self,
        ctx: &mut class_a::ClassACtx,
        call: impl FnOnce(&mut Leaves<'_, '_, M, G>),
    ) {
        let (obj, e) = (self.leaves.obj, self.leaves.ctx);
        {
            let mut memory = self.leaves.host.memory();
            store_class_a(&mut memory, obj, e, &self.synced, ctx);
        }
        call(&mut self.leaves);
        let (_, fresh) = load_class_a(&*self.leaves.host, obj);
        self.synced = fresh.clone();
        *ctx = fresh;
    }
}

impl<M: GuestMemory, G: GuestCall> class_a::ClassAWorld for ClassAWorld<'_, '_, M, G> {
    fn aware(&mut self, ctx: &mut class_a::ClassACtx) {
        self.with_guest(ctx, |l| {
            l.awareness();
        });
    }
    fn plays(&mut self, id: u32) -> bool {
        self.leaves.plays(id)
    }
    fn play(&mut self, id: u32, blend: f32, speed: f32) {
        self.leaves.play(id, blend, speed);
    }
    fn key_node(&mut self, key: u32) -> i32 {
        self.leaves.host.call_cdecl(0x9cc40, &[key]) as i32
    }
    fn player_node(&mut self) -> i32 {
        // `[[0x7B0C48]+0x14]->ext+0x678`, a pure memory walk in the original.
        let host = &self.leaves.host;
        let player = host.read32(0x7b_0c48);
        let target = word(&**host, player, 0x14);
        let ctx = word(&**host, target, 0x7c);
        word(&**host, ctx, 0x678) as i32
    }
    fn reposition(&mut self, ctx: &mut class_a::ClassACtx, node: i32) {
        self.with_guest(ctx, |l| {
            l.host.call_cdecl(0xda280, &[l.ctx, node as u32, 1]);
        });
    }
    fn route(&mut self, node: i32) -> bool {
        flag(
            self.leaves
                .host
                .call_cdecl(0x15d500, &[self.leaves.obj, node as u32]),
        )
    }
    fn start_navigation(&mut self, ctx: &mut class_a::ClassACtx, word: u32) {
        self.with_guest(ctx, |l| {
            l.host.call_cdecl(0x159300, &[l.obj, word, 0]);
        });
    }
    fn update_floor(&mut self) {
        self.leaves.host.call_cdecl(0x15f0d0, &[self.leaves.obj]);
    }
    fn update_yaw(&mut self) {
        self.leaves.host.call_cdecl(0x15cd00, &[self.leaves.obj]);
    }
    fn finished(&mut self) -> bool {
        self.leaves.finished()
    }
    fn rng_raw(&mut self) -> u32 {
        self.leaves.rng_raw()
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// 0x283730 idle variant
// ---------------------------------------------------------------------------------------------------------------------

fn load_idle_variant<M: GuestMemory>(m: &M, obj: u32) -> (u32, idle_variant::IdleCtx) {
    let c = m.read32(obj.wrapping_add(0x7c));
    let anchor_ptr = m.read32(c);
    let record = m.read32(c.wrapping_add(0xbf0));
    let slot = m.read32(c.wrapping_add(0x204));
    let weapons = m.read32(WEAPON_TABLE);
    let vec3 = |base: u32| {
        [
            word_float(m, base, 0),
            word_float(m, base, 4),
            word_float(m, base, 8),
        ]
    };
    let object = m.read32(c.wrapping_add(4));
    let ctx = idle_variant::IdleCtx {
        state: m.read32(c.wrapping_add(0x20)),
        pending: m.read32(c.wrapping_add(0x9d0)) as i32,
        previous: m.read32(c.wrapping_add(0x9d4)),
        state_time: float(m, c.wrapping_add(0x9d8)),
        saved_state: m.read32(c.wrapping_add(0x9dc)),
        wake_time: float(m, c.wrapping_add(0x9e0)),
        clock: float(m, c.wrapping_add(0xb30)),
        sway: float(m, c.wrapping_add(0xa90)),
        flags: m.read32(c.wrapping_add(0x2c)),
        status_flags: m.read32(c.wrapping_add(0x24)),
        model: m.read16(c.wrapping_add(0x48)),
        distance_squared: float(m, c.wrapping_add(0xaf4)),
        record_flags: word(m, record, 0x58),
        weapon_flags: word(
            m,
            weapons
                .wrapping_add(0xc)
                .wrapping_add(slot.wrapping_mul(WEAPON_STRIDE)),
            0,
        ),
        priority: m.read32(c.wrapping_add(8)) as i32,
        position: vec3(c.wrapping_add(0x30)),
        anchor: (anchor_ptr != 0).then(|| idle_variant::IdleAnchor {
            flags: word(m, anchor_ptr, 0x28),
            mode: word(m, anchor_ptr, 0x10),
            position: vec3(anchor_ptr.wrapping_add(0x64)),
        }),
        object_position: vec3(object.wrapping_add(0x64)),
    };
    (c, ctx)
}

fn idle_variant_words(c: &idle_variant::IdleCtx) -> [(u32, u32); 13] {
    [
        (0x20, c.state),
        (0x9d0, c.pending as u32),
        (0x9d4, c.previous),
        (0x9d8, c.state_time.to_bits()),
        (0x9dc, c.saved_state),
        (0x9e0, c.wake_time.to_bits()),
        (0xb30, c.clock.to_bits()),
        (0xa90, c.sway.to_bits()),
        (0x2c, c.flags),
        (0x24, c.status_flags),
        (0x30, c.position[0].to_bits()),
        (0x34, c.position[1].to_bits()),
        (0x38, c.position[2].to_bits()),
    ]
}

fn store_idle_variant<M: GuestMemory>(
    m: &mut M,
    c: u32,
    old: &idle_variant::IdleCtx,
    new: &idle_variant::IdleCtx,
) {
    for ((offset, before), (_, after)) in idle_variant_words(old)
        .into_iter()
        .zip(idle_variant_words(new))
    {
        put(m, c.wrapping_add(offset), before, after);
    }
}

struct IdleVariantWorld<'h, 'a, M: GuestMemory, G: GuestCall> {
    leaves: Leaves<'h, 'a, M, G>,
}

impl<M: GuestMemory, G: GuestCall> IdleVariantWorld<'_, '_, M, G> {
    /// Flushes the port's pending ctx changes, runs `call`, then reloads the ctx the guest leaf may have changed.
    fn with_ctx<R>(
        &mut self,
        ctx: &mut idle_variant::IdleCtx,
        call: impl FnOnce(&mut Leaves<'_, '_, M, G>) -> R,
    ) -> R {
        let (_, synced) = load_idle_variant(&*self.leaves.host, self.leaves.obj);
        {
            let mut memory = self.leaves.host.memory();
            store_idle_variant(&mut memory, self.leaves.ctx, &synced, ctx);
        }
        let result = call(&mut self.leaves);
        let (_, fresh) = load_idle_variant(&*self.leaves.host, self.leaves.obj);
        *ctx = fresh;
        result
    }
}

impl<M: GuestMemory, G: GuestCall> idle_variant::IdleVariantWorld
    for IdleVariantWorld<'_, '_, M, G>
{
    fn awareness(&mut self, ctx: &mut idle_variant::IdleCtx) -> bool {
        self.with_ctx(ctx, |l| l.awareness())
    }
    fn navigate(&mut self, ctx: &mut idle_variant::IdleCtx) {
        self.with_ctx(ctx, |l| {
            l.host.call_cdecl(0x15d470, &[l.obj]);
        });
    }
    fn plays(&mut self, id: u32) -> bool {
        self.leaves.plays(id)
    }
    fn transition(&mut self, id: u32) -> bool {
        self.leaves.transition(id)
    }
    fn finished(&mut self) -> bool {
        self.leaves.finished()
    }
    fn play(&mut self, id: u32, blend: f32, speed: f32) {
        self.leaves.play(id, blend, speed);
    }
    fn rng_raw(&mut self) -> u32 {
        self.leaves.rng_raw()
    }
    fn walk(&mut self, ctx: &mut idle_variant::IdleCtx) {
        self.with_ctx(ctx, |l| {
            l.host.call_cdecl(0x15cd00, &[l.obj]);
        });
    }
    fn request(&mut self, ctx: &mut idle_variant::IdleCtx, code: u32) {
        self.with_ctx(ctx, |l| {
            l.host.call_cdecl(0x17570, &[l.ctx, code, 0]);
        });
    }
}

fn idle_variant_globals<M: GuestMemory>(m: &M) -> idle_variant::IdleGlobals {
    idle_variant::IdleGlobals {
        clips: core::array::from_fn(|row| {
            core::array::from_fn(|k| m.read32(0x52_0838 + 32 * row as u32 + 4 * k as u32))
        }),
        tick_rate: m.read32(TICK_RATE) as i32,
        time_scale: float(m, TIME_SCALE),
        priority_floor: m.read32(0x79_0950) as i32,
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// 0x2621A0 idle variation
// ---------------------------------------------------------------------------------------------------------------------

fn load_idle_variation<M: GuestMemory>(m: &M, obj: u32) -> (u32, idle_variation::IdleVariationCtx) {
    let c = m.read32(obj.wrapping_add(0x7c));
    let slot = m.read32(c.wrapping_add(0x204));
    let weapons = m.read32(WEAPON_TABLE);
    let ctx = idle_variation::IdleVariationCtx {
        weapon_flags: word(
            m,
            weapons
                .wrapping_add(0xc)
                .wrapping_add(slot.wrapping_mul(WEAPON_STRIDE)),
            0,
        ) as u8,
        model: m.read32(c.wrapping_add(0x94)),
        flags: m.read32(c.wrapping_add(0x28)),
        aux_flags: m.read32(c.wrapping_add(0x2c)),
        refresh_timer: float(m, c.wrapping_add(0xa00)),
        repeats: m.read32(c.wrapping_add(0xaa0)) as i32,
        wait: float(m, c.wrapping_add(0xa80)),
        loop_time: float(m, c.wrapping_add(0xb30)),
        variation_time: float(m, c.wrapping_add(0xb34)),
        state_time: float(m, c.wrapping_add(0x9d8)),
        tick_rate: m.read32(TICK_RATE) as i32,
    };
    (c, ctx)
}

struct IdleVariationWorld<'h, 'a, M: GuestMemory, G: GuestCall>(Leaves<'h, 'a, M, G>);

impl<M: GuestMemory, G: GuestCall> idle_variation::IdleVariationWorld
    for IdleVariationWorld<'_, '_, M, G>
{
    fn select_clip(&mut self, stance: u32, variant: u32) -> u32 {
        self.0.host.call_cdecl(0x2aefb0, &[stance, variant])
    }
    fn plays(&mut self, id: u32) -> bool {
        self.0.plays(id)
    }
    fn finished(&mut self) -> bool {
        self.0.finished()
    }
    fn clip_time(&mut self) -> f32 {
        self.0.host.call_float(0x59d50, &[self.0.obj])
    }
    fn rng_raw(&mut self) -> u32 {
        self.0.rng_raw()
    }
    fn play(&mut self, id: u32, blend: f32, speed: f32) {
        self.0.play(id, blend, speed);
    }
}

// ---------------------------------------------------------------------------------------------------------------------

fn arg<M: GuestMemory>(m: &M, regs: &ExtRegisters, index: u32) -> u32 {
    m.read32(regs.esp.wrapping_add(4).wrapping_add(4 * index))
}

/// Dispatches a root; `false` when `va` is not one of the NPC adapters.
///
/// T2031/T1996: the port runs on `anim_lane::guarded`, a checked view of guest memory (an access outside 0x10000..0x80000000
/// reads 0 or is dropped). When the object, its context or any pointer read out of them is null or wild, the port's writes are
/// rolled back and the retained lifted original runs on the untouched state, exactly like the animation lane.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if !ROOTS.contains(&va) {
        return false;
    }
    crate::anim_lane::guarded(va, m, guest, regs, |view, guest, regs| {
        port(va, view, guest, regs)
    })
}

/// The adapter body of [`run`]; `false` when `va` is not one of the NPC adapters.
fn port<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    let entry = *regs;
    let (a0, a1) = (arg(m, &entry, 0), arg(m, &entry, 1));
    // 0x2621A0 is usercall (EBX = object), the others take the object on the stack.
    let obj = if va == 0x2621a0 { entry.ebx } else { a0 };
    let cell = RefCell::new(&mut *m);
    let mut host = HostBridge::new(&cell, guest, &entry);
    match va {
        0x265cc0 => {
            let (e, before) = load_class_a(&host, obj);
            let mut after = before.clone();
            let mut world = ClassAWorld {
                leaves: Leaves {
                    host: &mut host,
                    obj,
                    ctx: e,
                },
                synced: before.clone(),
            };
            class_a::tick(&mut after, &mut world);
            let synced = world.synced;
            let mut memory = host.memory();
            store_class_a(&mut memory, obj, e, &synced, &after);
        }
        0x283730 => {
            let (c, before) = load_idle_variant(&host, obj);
            let globals = idle_variant_globals(&host);
            let mut after = before.clone();
            let mut world = IdleVariantWorld {
                leaves: Leaves {
                    host: &mut host,
                    obj,
                    ctx: c,
                },
            };
            idle_variant::tick(&mut after, &globals, &mut world);
            // The world reloads `after` from guest memory after every leaf that takes the ctx, so what is left to write
            // is the difference to the last guest state (re-read here).
            let (_, synced) = load_idle_variant(&host, obj);
            let mut memory = host.memory();
            store_idle_variant(&mut memory, c, &synced, &after);
        }
        0x2621a0 => {
            let (c, before) = load_idle_variation(&host, obj);
            let mut after = before.clone();
            let source_flags = host_source_flags(&host, a1);
            let mut world = IdleVariationWorld(Leaves {
                host: &mut host,
                obj,
                ctx: c,
            });
            idle_variation::tick(&mut after, &mut world, a0, source_flags);
            let mut memory = host.memory();
            put(&mut memory, c.wrapping_add(0x28), before.flags, after.flags);
            put(
                &mut memory,
                c.wrapping_add(0x2c),
                before.aux_flags,
                after.aux_flags,
            );
            put(
                &mut memory,
                c.wrapping_add(0xa00),
                before.refresh_timer.to_bits(),
                after.refresh_timer.to_bits(),
            );
            put(
                &mut memory,
                c.wrapping_add(0xaa0),
                before.repeats as u32,
                after.repeats as u32,
            );
            put(
                &mut memory,
                c.wrapping_add(0xa80),
                before.wait.to_bits(),
                after.wait.to_bits(),
            );
            put(
                &mut memory,
                c.wrapping_add(0xb30),
                before.loop_time.to_bits(),
                after.loop_time.to_bits(),
            );
            put(
                &mut memory,
                c.wrapping_add(0xb34),
                before.variation_time.to_bits(),
                after.variation_time.to_bits(),
            );
        }
        _ => return false,
    }
    true
}

/// `[source + 0x34]`, the flag word of the second stack argument of 0x2621A0.
fn host_source_flags<M: GuestMemory>(m: &M, source: u32) -> u32 {
    m.read32(source.wrapping_add(0x34))
}

/// The NPC lane roots in registration order (appended to `lane_inv::INV_ROOTS`).
pub const ROOTS: [u32; 3] = [0x265cc0, 0x283730, 0x2621a0];

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::BTreeMap, rc::Rc, vec::Vec};

    #[derive(Clone, Default)]
    struct Mem(Rc<RefCell<BTreeMap<u32, u8>>>);
    impl GuestMemory for Mem {
        fn read8(&self, a: u32) -> u8 {
            assert!(
                (0x1_0000..0x8000_0000).contains(&a),
                "raw read outside the mapped range: {a:#x}"
            );
            *self.0.borrow().get(&a).unwrap_or(&0)
        }
        fn write8(&mut self, a: u32, v: u8) {
            assert!(
                (0x1_0000..0x8000_0000).contains(&a),
                "raw write outside the mapped range: {a:#x}"
            );
            self.0.borrow_mut().insert(a, v);
        }
    }
    /// Logs `(va, first args)`; answers every leaf with `answer`.
    struct Guest {
        mem: Mem,
        log: Rc<RefCell<Vec<(u32, Vec<u32>)>>>,
        answer: u32,
    }
    impl GuestCall for Guest {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            let args = (0..4)
                .map(|i| self.mem.read32(regs.esp.wrapping_add(4 * i)))
                .collect();
            self.log.borrow_mut().push((va, args));
            regs.eax = self.answer;
        }
    }
    fn setup(va: u32, answer: u32) -> (Mem, Guest, ExtRegisters) {
        let mut mem = Mem::default();
        let (obj, c) = (0x11000u32, 0x12000u32);
        mem.write32(obj + 0x7c, c);
        mem.write32(obj + 4, 0x13000);
        mem.write32(0x4e_8cec, 60);
        mem.write32(0x4e_794c, 1.0f32.to_bits());
        let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = 0x18000;
        mem.write32(regs.esp + 4, obj);
        if va == 0x2621a0 {
            regs.ebx = obj;
            mem.write32(regs.esp + 4, 1);
            mem.write32(regs.esp + 8, 0x14000);
        }
        let guest = Guest {
            mem: mem.clone(),
            log: Rc::default(),
            answer,
        };
        (mem, guest, regs)
    }

    #[test]
    fn unknown_root_is_inert() {
        let (mut mem, mut guest, mut regs) = setup(0x265cc0, 0);
        assert!(!run(0x1234, &mut mem, &mut guest, &mut regs));
        assert!(guest.log.borrow().is_empty());
    }

    #[test]
    fn class_a_state_zero_queues_setup_and_calls_awareness() {
        let (mut mem, mut guest, mut regs) = setup(0x265cc0, 0);
        mem.write32(0x12000 + 0x20, 0);
        mem.write32(0x12000 + 0x9d0, u32::MAX);
        assert!(run(0x265cc0, &mut mem, &mut guest, &mut regs));
        let log = guest.log.borrow();
        assert_eq!(
            (log[0].0, &log[0].1[..2]),
            (0x15e120, &[0x12000, 1][..]),
            "awareness(ctx, 1) first"
        );
        assert_eq!(mem.read32(0x12000 + 0x9d0), 1, "state 0 queues state 1");
    }

    #[test]
    fn idle_variation_reads_source_flag_and_plays_loop() {
        let (mut mem, mut guest, mut regs) = setup(0x2621a0, 1);
        mem.write32(0x14_000 + 0x34, 0x8_0000);
        mem.write32(0x4f_9bac, 0x15000);
        assert!(run(0x2621a0, &mut mem, &mut guest, &mut regs));
        assert_eq!(
            mem.read32(0x12000 + 0x2c) & 0x400,
            0x400,
            "forced refresh sets the aux flag"
        );
        assert!(
            guest.log.borrow().iter().any(|(va, _)| *va == 0x2aefb0),
            "stance selector called"
        );
    }

    #[test]
    fn idle_variant_unaware_enters_hold_state_with_rng() {
        let (mut mem, mut guest, mut regs) = setup(0x283730, 0);
        mem.write32(0x4f_9bac, 0x15000);
        mem.write32(0x12000 + 0x20, 1);
        mem.write32(0x12000 + 0x9d0, u32::MAX);
        assert!(run(0x283730, &mut mem, &mut guest, &mut regs));
        let log = guest.log.borrow();
        assert_eq!(log[0].0, 0x15e120);
        assert!(
            log.iter().any(|(va, _)| *va == 0x153b40),
            "rng drawn when unaware"
        );
        assert_eq!(mem.read32(0x12000 + 0x20), 6, "unaware: hold state");
        assert_eq!(mem.read32(0x12000 + 0x9dc), 1, "saved state preserved");
    }

    /// T2031/T1996: a null, small, wild or unmapped object in any argument or in EBX never reaches raw memory (the strict `Mem`
    /// panics on it) and the call is handed to the retained original, whose result is returned.
    #[test]
    fn bad_pointers_fall_back_to_the_original() {
        for &va in &ROOTS {
            for pointer in [
                0u32,
                4,
                0xffff,
                0xdead_beef,
                0x8000_0000,
                0xffff_fffc,
                0x2_0000,
            ] {
                let (mut mem, mut guest, mut regs) = setup(va, 0xa5a5);
                guest.answer = 0xa5a5;
                for i in 0..4 {
                    mem.write32(regs.esp + 4 + 4 * i, pointer);
                }
                regs.ebx = pointer;
                assert!(
                    run(va, &mut mem, &mut guest, &mut regs),
                    "{va:#x} {pointer:#x}"
                );
                assert_eq!(
                    guest.log.borrow().last().map(|call| call.0),
                    Some(va),
                    "{va:#x} {pointer:#x}: original runs"
                );
                assert_eq!(regs.eax, 0xa5a5, "{va:#x} {pointer:#x}: original result");
                assert_eq!(regs.esp, 0x18000, "{va:#x}: esp restored");
            }
        }
    }
}
