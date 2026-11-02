//! T1864: wiring of the T1856 engine services into the level hooks.
//!
//! The service ports (`svc_objects`, `svc_particles`, `svc_effect_submit`, `svc_spawn_c1950`, `svc_slot_teardown`,
//! `svc_particle_efbd0`) each work on their own memory trait. This module gives them ONE shared guest memory:
//!
//! * [`GuestRam`] is a sparse zero-filled byte memory (FABRICATED content, the original reads zero from unmapped pages
//!   in the oracle). The hooks never own it; a [`HookServices`] holds it in a [`RefCell`].
//! * [`CellMemory`] is a proxy over `&RefCell<GuestRam>` that implements every memory trait of the ports
//!   (`strip_update::GuestMemory`, `sweep_records::GuestMemory`, `EffectMemory`, `SvcMemory`) and borrows the cell per
//!   access, so a service can hold the cell while the caller passes the memory as a separate `&mut` argument. This is
//!   the "shared RefCell guest/effect memory cell" `svc_slot_teardown` asks for.
//! * [`HookServices`] runs the three hook entry points: [`HookServices::run_strip_spawner`] (`0x1F48A0` over a
//!   [`StripServices`] stack: objects, particles, spawner `0xC1950`, matrix helpers), [`HookServices::run_prelude`]
//!   (`0x1E2B80`, levels 4 and 0x50, every [`PreludeEffect`] executed by the ported service) and the impact path
//!   ([`HookServices::impact_services`], the `ImpactServices` stack over the same cell).
//!
//! Everything below the ports stays behind recording traits and is written to [`TraceEvent`]s in call order: the sound
//! hardware (`0x28240` callees, T1861), the `0xBABA0` callees (T1862) and the other `0xEDD40` kinds (T1863). The
//! recording stand-ins return 0 (or a fresh bump allocated address for the object pool) and are FABRICATED; the
//! control flow above them is MEASURED in the T1856 documents.

use std::cell::RefCell;
use std::collections::BTreeMap;

use super::callees_t1846::ThrottledSound;
use super::group_members::{
    AdvanceOut, AdvanceProbe, CellHit, CellProbe, SweepGeometry, prelude_sweep,
};
use super::prelude_1e2b80::{PreludeEffect, PreludeEnv, SweepQuery, SweepResult, prelude_1e2b80};
use super::strip_update::{
    GuestMemory, Lcg64, Service, SpawnerFault, StripServices, strip_spawner,
};
use super::svc_destroy::{consume_hit, destroy_pool_object, object_flag, pool_get, release_buffer};
use super::svc_destroy_callees::PortedDestroy;
use super::svc_effect_submit::disturb_objects;
use super::svc_effect_submit::{
    EffectSubmitServices, HwCall, SoundHw, StarterArgs, SvcMemory, sound_starter, submit_effect,
};
use super::svc_effect_tick::{SprayCall, TRAIL_WORDS, TickEngine, TickGlobals, pool_tick};
use super::svc_ember_services::{
    KIND_TABLE, PULSE_GRID, PairServices, PulseServices, SettleServices, TriggerServices,
    light_pulse, settle_entry, sound_pair, trigger_effect,
};
use super::svc_emitter::{EmitterServices, apply_param, create_emitter, emitter_pool_init};
use super::svc_objects::{
    DestroyEffects, GLOBAL_FIRST_SLOT, GLOBAL_PLAYER_ARRAY, GLOBAL_SLOT_BASE, GLOBAL_SLOT_COUNT,
    ObjectServices, destroy_object, push_all_players, push_player,
};
use super::svc_particle_efbd0::{
    EffectEntry, EffectPool, EmberEngine, EmberWorld, KindRow, PlayerSlot, SpawnArgs, SpawnResult,
    W_KIND, W_NEXT, W_PREV, W_TYPE, run_prelude_particle, spawn_effect, start_effect,
};
use super::svc_particle_kinds::{DebrisCall, EmitterParam, KindEngine, start_kind_handler};
use super::svc_particles::{EffectSink, ParticleStripServices, refresh_regions, sound_1, sound_2};
use super::svc_pool::PortedPool;
use super::svc_reloc::PortedFiles;
use super::svc_shape::{PortedShape, ShapeContext};
use super::svc_shape_services::PortedShapeServices;
use super::svc_slot_teardown::{RecordedCallbacks, TeardownImpactServices};
use super::svc_spawn_c1950::{NativeCallees, SpawnAdapter, SpawnLeaves};
use super::sweep_dynamic_shape::{EffectMemory, ImpactServices, rng as guest_rng};
use super::sweep_records;
use super::sweep_shapes::transform_point;

/// Guest addresses of the FABRICATED host scaffolding (stack frames, scratch, object heap, player records). None of
/// them exists in the original image; they only have to be mapped and not collide with each other.
pub mod fabricated {
    /// esp at entry of `0x1F48A0` (vectors live below it).
    pub const STRIP_FRAME: u32 = 0x5F00_0000;
    /// esp at entry of the `0xBD090` call (the destroy block lives below it).
    pub const DESTROY_FRAME: u32 = 0x5E00_0000;
    /// esp at entry of `0xC1950`.
    pub const SPAWN_FRAME: u32 = 0x5D00_0000;
    /// Request record scratch of the sound submit and the starters.
    pub const SUBMIT_SCRATCH: u32 = 0x5C00_0000;
    /// Base of the object pool `[0x72F5E8]` (`0x2E8` byte records, T1865: the real `0xA6880` allocates from it).
    pub const OBJECT_HEAP: u32 = 0x5800_0000;
    /// Size of one pool record.
    pub const OBJECT_SIZE: u32 = 0x2E8;
    /// Model descriptor table `[0x791D4C]` (descriptor 0 for every model, preloaded model block: FABRICATED).
    pub const MODEL_TABLE: u32 = 0x5700_0000;
    /// The preloaded model block descriptor 0 points at.
    pub const MODEL_BLOCK: u32 = 0x5700_1000;
    /// Shape slot pool `[0x70AADC]` (`0x200` byte records).
    pub const SHAPE_POOL: u32 = 0x5600_0000;
    /// esp at entry of the `0xA6880` calls of the recording leaves (the callees with a frame see offsets of it).
    pub const POOL_FRAME: u32 = 0x5D00_0000 - 0x2000;
    /// Debris objects handed out to the kind 44 to 46 handlers (`0x400` bytes each).
    pub const DEBRIS_HEAP: u32 = 0x5900_0000;
    /// Handles handed out for the debris objects (`0x10` apart).
    pub const DEBRIS_HANDLES: u32 = 0x5A00_0000;
    /// Emitter pool records (`0xF8` bytes each, 64 of them), laid out by `HookServices::new`.
    pub const EMITTER_POOL: u32 = 0x5B00_0000;
    /// Scratch for the position and direction vectors passed by pointer to the ported emitter services.
    pub const EMITTER_ARGS: u32 = 0x5B80_0000;
    /// Mirror of the effect entry the settle service reads.
    pub const ENTRY_SCRATCH: u32 = 0x5B90_0000;
    /// Guest address of pool slot 0 for the ported update callbacks (T1871) when the world has no base.
    pub const EFFECT_ENTRIES: u32 = 0x5BA0_0000;
    /// Spark burst record (`+0x18` points at the particle block `+0x400`).
    pub const BURST_RECORD: u32 = 0x5BA0_0000;
    /// Pulse grid for `0x186480`: 32 row pointers at `+0`, one shared empty row (32 heads of -1) at `+0x100`.
    pub const PULSE_ROWS: u32 = 0x5BB0_0000;
    /// esp at entry of the table registrars (`0x8E310`, `0x637E0`, `0x18390`), FABRICATED (no call stack is modelled).
    pub const RELOC_FRAME: u32 = 0x5C80_0000;
    /// Array of player records (`[0x7B0C48]`).
    pub const PLAYER_RECORDS: u32 = 0x5100_0000;
    /// Player objects (one per record).
    pub const PLAYER_OBJECTS: u32 = 0x5200_0000;
    /// `[0x7B0C7C]`: the pointer the level 4 prelude calls `G` (the first player's controller block).
    pub const PRELUDE_PLAYER: u32 = 0x5300_0000;
}

// ---------------------------------------------------------------------------------------------------------------
// Memory
// ---------------------------------------------------------------------------------------------------------------

const PAGE: u32 = 0x1000;

/// Sparse zero-filled little endian guest memory.
#[derive(Clone, Debug, Default)]
pub struct GuestRam {
    pages: BTreeMap<u32, Box<[u8; PAGE as usize]>>,
}

impl GuestRam {
    pub fn byte(&self, address: u32) -> u8 {
        self.pages
            .get(&(address / PAGE))
            .map_or(0, |page| page[(address % PAGE) as usize])
    }
    pub fn set_byte(&mut self, address: u32, value: u8) {
        let page = self
            .pages
            .entry(address / PAGE)
            .or_insert_with(|| Box::new([0; PAGE as usize]));
        page[(address % PAGE) as usize] = value;
    }
    pub fn word(&self, address: u32) -> u32 {
        (0..4).fold(0, |acc, k| {
            acc | u32::from(self.byte(address.wrapping_add(k))) << (8 * k)
        })
    }
    pub fn set_word(&mut self, address: u32, value: u32) {
        for k in 0..4 {
            self.set_byte(address.wrapping_add(k), (value >> (8 * k)) as u8);
        }
    }
    pub fn set_float(&mut self, address: u32, value: f32) {
        self.set_word(address, value.to_bits());
    }
}

macro_rules! ram_traits {
    ($type:ty, |$self_:ident| $ram:expr) => {
        impl GuestMemory for $type {
            fn read_u8(&$self_, address: u32) -> u8 {
                $ram.byte(address)
            }
            fn write_u8(&mut $self_, address: u32, value: u8) {
                $ram.set_byte(address, value);
            }
            fn read_u32(&$self_, address: u32) -> u32 {
                $ram.word(address)
            }
            fn write_u32(&mut $self_, address: u32, value: u32) {
                $ram.set_word(address, value);
            }
        }
        impl sweep_records::GuestMemory for $type {
            fn read_u8(&$self_, address: u32) -> u8 {
                $ram.byte(address)
            }
            fn read_u32(&$self_, address: u32) -> u32 {
                $ram.word(address)
            }
        }
        impl EffectMemory for $type {
            fn write_u8(&mut $self_, address: u32, value: u8) {
                $ram.set_byte(address, value);
            }
            fn write_u32(&mut $self_, address: u32, value: u32) {
                $ram.set_word(address, value);
            }
        }
        impl SvcMemory for $type {
            fn r8(&$self_, address: u32) -> u8 {
                $ram.byte(address)
            }
            fn w8(&mut $self_, address: u32, value: u8) {
                $ram.set_byte(address, value);
            }
            fn r32(&$self_, address: u32) -> u32 {
                $ram.word(address)
            }
            fn w32(&mut $self_, address: u32, value: u32) {
                $ram.set_word(address, value);
            }
        }
    };
}

ram_traits!(GuestRam, |self| self);

/// Proxy over the shared cell; every access borrows for its own duration only.
#[derive(Clone, Copy)]
pub struct CellMemory<'a>(pub &'a RefCell<GuestRam>);

ram_traits!(CellMemory<'_>, |self| self.0.borrow_mut());

// ---------------------------------------------------------------------------------------------------------------
// Trace
// ---------------------------------------------------------------------------------------------------------------

/// One recorded call, in call order.
#[derive(Clone, Debug, PartialEq)]
pub enum TraceEvent {
    /// A [`StripServices`] call of `0x1F48A0` (ported or not): service, arguments and result.
    Strip {
        service: Service,
        args: Vec<u32>,
        result: u32,
    },
    /// A sound hardware call below `0x45820`/`0x468A0` (`0x28240` voice start and neighbours, T1861).
    Hardware(HwCall),
    /// A routine below `0xBABA0` (T1862): address and cdecl arguments.
    DestroyCallee { address: u32, args: Vec<u32> },
    /// A leaf below `0xC1950` (pool, hooks, `0xA7090`): name and arguments.
    SpawnLeaf { leaf: &'static str, args: Vec<u32> },
    /// A prelude effect that was executed by its ported service.
    Prelude(PreludeEffect),
    /// A recorded engine call of the ember emitter `0xEFBD0` (services of `EmberEngine` that are not ported).
    Ember {
        service: &'static str,
        args: Vec<u32>,
    },
    /// The sweep query of the prelude ran through `group_members::prelude_sweep` over the null geometry.
    Sweep(SweepQuery),
}

type Trace = RefCell<Vec<TraceEvent>>;

/// Sound hardware stand-in: records every call (T1861 owns the real `0x28240` callees).
struct RecordingHw<'a> {
    trace: &'a Trace,
    players: i32,
}

impl SoundHw for RecordingHw<'_> {
    fn call(&mut self, call: HwCall) -> u32 {
        self.trace.borrow_mut().push(TraceEvent::Hardware(call));
        0
    }
    fn player_count(&mut self) -> i32 {
        self.players
    }
    fn player_slot(&mut self, index: i32) -> i32 {
        index
    }
}

/// The `0xBABA0` callees stand-in (T1862).
struct RecordingDestroy<'a> {
    trace: &'a Trace,
}

impl DestroyEffects for RecordingDestroy<'_> {
    fn call(&mut self, _memory: &mut dyn GuestMemory, address: u32, args: &[u32]) -> u32 {
        self.trace.borrow_mut().push(TraceEvent::DestroyCallee {
            address,
            args: args.to_vec(),
        });
        0
    }
}

/// The leaves below the object spawner: pool objects come from the real `0xA6880` (T1865) over the synthetic model
/// descriptor world of [`install_pool_world`], the rest is recorded.
struct RecordingLeaves<'a> {
    trace: &'a Trace,
}

/// Lays out the synthetic pool world the real `0xA6880` needs, in guest memory: the object pool `[0x72F5E8]`
/// (`0x2E8` byte records from [`fabricated::OBJECT_HEAP`], none used), the model index table `0x78B860` (all zero:
/// every model uses descriptor 0), the descriptor table `[0x791D4C]` with descriptor 0 = kind 1 and a PRELOADED
/// model block (no resource loading happens, the loaders `0x96410` / `0x8E4B0` / `0x8E590` stay unused here), the
/// shape slot pool `[0x70AADC]` and the handle counter `[0x72F5E0] = 1`. FABRICATED, no original data involved.
pub fn install_pool_world(ram: &mut GuestRam) {
    ram.set_word(0x72_F5E8, fabricated::OBJECT_HEAP);
    ram.set_word(0x72_F5E0, 1);
    ram.set_word(0x79_1D4C, fabricated::MODEL_TABLE);
    ram.set_word(fabricated::MODEL_TABLE, 1);
    ram.set_word(fabricated::MODEL_TABLE + 4, fabricated::MODEL_BLOCK);
    ram.set_word(0x70_AADC, fabricated::SHAPE_POOL);
    // One model part with no parent, child or sibling (-1): the real `0x8FB20` walks the part tree from part 0 and
    // never ends on a block without parts (a zeroed part would be its own child).
    ram.set_word(fabricated::MODEL_BLOCK, 1);
    let part = fabricated::MODEL_BLOCK - 0xC8;
    for offset in 0x49..=0x4C {
        ram.set_byte(part + offset, 0xFF);
    }
}

/// Runs `body` over the real pool support with the real file and shape layers between it and the recording engine
/// stand-in: `PortedFiles` (T1866 `PortedResource` plus the T1869 relocators and registrars) so the model loaders
/// `0x96410` / `0x8E4B0` / `0x8E590` run over the pak slots mounted in guest memory, and `PortedShape` (T1866) with
/// `PortedShapeServices` (T1869) for the shape setup below `0x97C00` / `0xA6530`. Two shape layers carry the two
/// frames: `0x97C00` sees `POOL_FRAME - 0x1C` (INFERRED: same call frame as `0x96410`, both take two arguments, MEASURED
/// from the call site of `0xA6880`), `0xA6530` sees `POOL_FRAME - 0x18` (MEASURED in T1865). Still recorded: the
/// formatter, the host file API, the allocators, the texture creation and the pose engine.
pub fn with_pool_layers<R>(trace: &Trace, body: impl FnOnce(&mut PortedPool) -> R) -> R {
    let mut recording = RecordingDestroy { trace };
    let mut files = PortedFiles {
        inner: &mut recording,
        frame: fabricated::RELOC_FRAME,
    };
    let mut services = PortedShapeServices {
        inner: &mut files,
        entry: fabricated::POOL_FRAME.wrapping_sub(0x1C),
    };
    let mut shape = PortedShape {
        inner: &mut services,
        entry: fabricated::POOL_FRAME.wrapping_sub(0x1C),
        context: ShapeContext::ObjectShape,
    };
    let mut place = PortedShape {
        inner: &mut shape,
        entry: fabricated::POOL_FRAME.wrapping_sub(0x18),
        context: ShapeContext::ObjectPlace,
    };
    let mut pool = PortedPool {
        inner: &mut place,
        entry: fabricated::POOL_FRAME,
    };
    body(&mut pool)
}

impl RecordingLeaves<'_> {
    fn note(&self, leaf: &'static str, args: &[u32]) {
        self.trace.borrow_mut().push(TraceEvent::SpawnLeaf {
            leaf,
            args: args.to_vec(),
        });
    }
}

impl SpawnLeaves for RecordingLeaves<'_> {
    fn model_hook(&mut self, _m: &mut dyn GuestMemory, function: u32, arg: u32) -> u32 {
        self.note("model_hook", &[function, arg]);
        arg
    }
    fn dispatch(&mut self, _m: &mut dyn GuestMemory, function: u32, object: u32, arg: u32) {
        self.note("dispatch", &[function, object, arg]);
    }
    fn pool_get(&mut self, m: &mut dyn GuestMemory, model: u32, p1: u32, p2: u32) -> u32 {
        // `0xA6880`, ported in `svc_destroy` over the pool support of `svc_pool` (T1865); the engine below it is recorded
        let object = with_pool_layers(self.trace, |pool| pool_get(m, pool, model, p1, p2));
        self.note("pool_get", &[model, p1, p2, object]);
        object
    }
    fn init_object(&mut self, m: &mut dyn GuestMemory, object: u32, p4: u32) {
        self.note("init_object", &[object, p4]);
        // `0x6B060`, ported in `svc_destroy` (T1862); the voice callees below it are recorded.
        consume_hit(m, &mut RecordingDestroy { trace: self.trace }, object, p4);
    }
    fn alloc_hook(&mut self, _m: &mut dyn GuestMemory, function: u32, object: u32, p4: u32) {
        self.note("alloc_hook", &[function, object, p4]);
    }
    fn post_alloc(&mut self, m: &mut dyn GuestMemory, object: u32, mode: u32) -> u32 {
        self.note("post_alloc", &[object, mode]);
        // `0x12E30`, ported in `svc_destroy` (T1862)
        object_flag(m, object, mode)
    }
    fn detach(&mut self, m: &mut dyn GuestMemory, object: u32) {
        self.note("detach", &[object]);
        // `0x10C0E0`, ported in `svc_destroy` (T1862); the heap free below it is recorded
        release_buffer(m, &mut RecordingDestroy { trace: self.trace }, object);
    }
    fn destroy(&mut self, m: &mut dyn GuestMemory, object: u32) {
        self.note("destroy_a7090", &[object]);
        // `0xA7090`, ported in `svc_destroy` (T1862) with its release set ported in `svc_pool` (T1865); the engine
        // callees below those are recorded
        with_pool_layers(self.trace, |pool| destroy_pool_object(m, pool, object));
    }
}

/// `EffectSink` over the shared cell: writes the record to the submit scratch and runs the ported `0x45820`.
struct CellSink<'a> {
    ram: &'a RefCell<GuestRam>,
    hw: RecordingHw<'a>,
}

impl EffectSink for CellSink<'_> {
    fn submit(&mut self, record: &[u32; 12]) -> u32 {
        let mut memory = CellMemory(self.ram);
        for (index, word) in record.iter().enumerate() {
            memory.w32(fabricated::SUBMIT_SCRATCH + 4 * index as u32, *word);
        }
        submit_effect(&mut memory, &mut self.hw, fabricated::SUBMIT_SCRATCH) as u32
    }
}

/// The three services of `0x1F48A0` no other module owns: `0xA14B0` (world matrix, the object body is not in the
/// image's memory here, so the matrix is the identity: FABRICATED), `0x151E80` (ported) and `0xC1950` (spawner).
struct GlueServices<'a> {
    spawner: SpawnAdapter<NativeCallees<RecordingLeaves<'a>>>,
}

impl StripServices for GlueServices<'_> {
    fn call(&mut self, memory: &mut dyn GuestMemory, service: Service, args: &[u32]) -> u32 {
        match service {
            Service::WorldMatrix => {
                for index in 0..16u32 {
                    let one = if index % 5 == 0 { 1.0f32 } else { 0.0 };
                    memory.write_u32(args[0] + 4 * index, one.to_bits());
                }
                0
            }
            Service::TransformVec => {
                let mut matrix = [0.0f32; 16];
                for (index, slot) in matrix.iter_mut().enumerate() {
                    *slot = f32::from_bits(memory.read_u32(args[0] + 4 * index as u32));
                }
                let point = [0, 4, 8].map(|k| f32::from_bits(memory.read_u32(args[1] + k)));
                // INFERRED: only x, y, z are written, the fourth float is left alone.
                let out = transform_point(&matrix, point);
                for (k, value) in out.iter().enumerate() {
                    memory.write_u32(args[1] + 4 * k as u32, value.to_bits());
                }
                0
            }
            Service::SpawnObject => self.spawner.spawn(memory, args),
            _ => 0,
        }
    }
}

/// Records every call that passes through (result included) and forwards it.
struct TracedServices<'a> {
    inner: &'a mut dyn StripServices,
    trace: &'a Trace,
}

impl StripServices for TracedServices<'_> {
    fn call(&mut self, memory: &mut dyn GuestMemory, service: Service, args: &[u32]) -> u32 {
        let result = self.inner.call(memory, service, args);
        self.trace.borrow_mut().push(TraceEvent::Strip {
            service,
            args: args.to_vec(),
            result,
        });
        result
    }
}

/// Null geometry for the prelude sweep: no cell, no hit (the world mesh is the unported level data).
struct NullGeometry;

impl SweepGeometry for NullGeometry {
    fn cell_test(&mut self, _probe: &CellProbe) -> CellHit {
        CellHit::default()
    }
    fn advance(&mut self, probe: &AdvanceProbe) -> AdvanceOut {
        AdvanceOut {
            next: 0,
            from_copy: probe.from_copy,
            vec_copy: probe.vec_copy,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Ember engine
// ---------------------------------------------------------------------------------------------------------------

/// `EmberEngine` over the shared cell: the rng, the sound submit and the player nudge are the ported services, the rest
/// is recorded (`start_kind` is the T1863 gap).
struct CellEmber<'a> {
    ram: &'a RefCell<GuestRam>,
    mem: CellMemory<'a>,
    trace: &'a Trace,
    hw: RecordingHw<'a>,
    /// Debris objects handed out so far by this engine.
    debris_made: u32,
    /// The class table `0x4DEB28` (T1870), when the tick runs the other pool classes.
    classes: Option<&'a super::svc_pool_classes::ClassTable>,
}

impl CellEmber<'_> {
    fn note(&self, service: &'static str, args: &[u32]) {
        self.trace.borrow_mut().push(TraceEvent::Ember {
            service,
            args: args.to_vec(),
        });
    }
    /// Write two vectors to the argument scratch (the ported services take guest pointers). Returns their addresses.
    fn stage(&mut self, first: [f32; 3], second: [f32; 3]) -> (u32, u32) {
        let (a, b) = (fabricated::EMITTER_ARGS, fabricated::EMITTER_ARGS + 0x10);
        for (k, value) in first.iter().enumerate() {
            self.mem.w32(a + 4 * k as u32, value.to_bits());
        }
        for (k, value) in second.iter().enumerate() {
            self.mem.w32(b + 4 * k as u32, value.to_bits());
        }
        (a, b)
    }
    /// Run the ported emitter creation `0x111AE0(0, template, flags, &position, &direction)` over the cell.
    fn create(&mut self, template: u32, flags: u32, position: [f32; 3], direction: [f32; 3]) {
        let (at, along) = self.stage(position, direction);
        let mut leaves = EmberLeaves {
            ram: self.ram,
            trace: self.trace,
            players: self.hw.players,
        };
        let created = create_emitter(&mut self.mem, &mut leaves, 0, template, flags, at, along);
        self.note("emitter_created", &[template, flags, created]);
    }
}

/// Leaves of the ported ember services that the cell does not own: recorded, with FABRICATED answers.
struct EmberLeaves<'a> {
    ram: &'a RefCell<GuestRam>,
    trace: &'a Trace,
    players: i32,
}

impl EmberLeaves<'_> {
    fn note(&self, service: &'static str, args: &[u32]) {
        self.trace.borrow_mut().push(TraceEvent::Ember {
            service,
            args: args.to_vec(),
        });
    }
    fn submit(&self, request: &[u32; 12]) -> u32 {
        let mut sink = CellSink {
            ram: self.ram,
            hw: RecordingHw {
                trace: self.trace,
                players: self.players,
            },
        };
        sink.submit(request)
    }
}

/// `0xC3E60` has no floor here (FABRICATED: the level mesh is not loaded) and the buffer free is recorded.
impl EmitterServices for EmberLeaves<'_> {
    fn ground(&mut self, _point: [f32; 3]) -> (f32, u32) {
        self.note("emitter_ground", &[]);
        (0.0, 0)
    }
    fn free_block(&mut self, block: u32) {
        self.note("emitter_free_block", &[block]);
    }
}

/// `0x278040` is the level predicate dispatch (`0x2776F0`, not ported): recorded, never lets the settle run.
impl SettleServices for EmberLeaves<'_> {
    fn actor_gate(&mut self, link: u32) -> bool {
        self.note("actor_gate", &[link]);
        false
    }
    fn disturb(
        &mut self,
        mem: &mut dyn SvcMemory,
        position: u32,
        shape_end: u32,
        radius: f32,
        out: u32,
    ) {
        self.note("disturb", &[radius.to_bits()]);
        disturb_objects(mem, position, shape_end, radius, out);
    }
}

/// `0x1836A0` and `0x184040` (pulse pool and region pulse) are recorded and start nothing.
impl PulseServices for EmberLeaves<'_> {
    fn node_pulse(&mut self, node: i32, value: f32) -> u32 {
        self.note("node_pulse", &[node as u32, value.to_bits()]);
        0
    }
    fn region_pulse(&mut self, position: u32) -> u32 {
        self.note("region_pulse", &[position]);
        0
    }
    fn submit(&mut self, request: [u32; 12]) {
        EmberLeaves::submit(self, &request);
    }
}

impl PairServices for EmberLeaves<'_> {
    fn submit(&mut self, request: [u32; 12]) -> i32 {
        EmberLeaves::submit(self, &request) as i32
    }
    fn voice_volumes(&mut self, handle: i32, left: i32, right: i32) {
        self.note("voice_volumes", &[handle as u32, left as u32, right as u32]);
    }
}

/// The burst record pool `0xFF2B0` is one FABRICATED record (reused), `0x1CDAE0` answers 1 and the region tree is empty.
impl TriggerServices for EmberLeaves<'_> {
    fn allocate(&mut self) -> u32 {
        let mut memory = CellMemory(self.ram);
        memory.w32(
            fabricated::BURST_RECORD + 0x18,
            fabricated::BURST_RECORD + 0x400,
        );
        self.note("burst_allocate", &[]);
        fabricated::BURST_RECORD
    }
    fn view_mode(&mut self) -> u32 {
        1
    }
    fn region_query(&mut self, box_start: u32, _ring: u32) -> u32 {
        self.note("burst_regions", &[box_start]);
        0
    }
}

impl EmberEngine for CellEmber<'_> {
    fn rng(&mut self) -> u32 {
        guest_rng(&mut CellMemory(self.ram))
    }
    fn submit_sound(&mut self, request: [u32; 12]) {
        let mut sink = CellSink {
            ram: self.ram,
            hw: RecordingHw {
                trace: self.trace,
                players: self.hw.players,
            },
        };
        sink.submit(&request);
    }
    fn throttled_sound(&mut self, sound: ThrottledSound) {
        self.note("throttled_sound", &[sound.sound_id, sound.cooldown as u32]);
        // `0x46A00(id, a, b, c)`: ported, hardware volumes recorded
        let mut leaves = EmberLeaves {
            ram: self.ram,
            trace: self.trace,
            players: self.hw.players,
        };
        sound_pair(
            &self.mem,
            &mut leaves,
            sound.sound_id,
            sound.arg_a,
            sound.arg_b,
            sound.arg_c,
        );
    }
    fn region_query(&mut self, _region: [f32; 6]) -> Vec<i32> {
        self.note("region_query", &[]);
        Vec::new()
    }
    fn advance_cell(&mut self, probe: &AdvanceProbe) -> AdvanceOut {
        AdvanceOut {
            next: 0,
            from_copy: probe.from_copy,
            vec_copy: probe.vec_copy,
        }
    }
    fn flag_3533f0(&mut self) -> bool {
        false
    }
    fn spawn_emitter(&mut self, template: u32, position: [f32; 3], direction: [f32; 3]) {
        self.note(
            "spawn_emitter",
            &[
                template,
                position[0].to_bits(),
                position[1].to_bits(),
                position[2].to_bits(),
            ],
        );
        self.create(template, 0, position, direction);
    }
    fn set_scale(&mut self, scale: f32) {
        self.note("set_scale", &[scale.to_bits()]);
        apply_param(&mut self.mem, &EmitterParam::Scale(scale));
    }
    fn nudge_player(&mut self, player: usize, f: f32, a: f32, b: f32) {
        let array = self.ram.borrow().word(GLOBAL_PLAYER_ARRAY);
        let record = array.wrapping_add(0x1584 * player as u32);
        push_player(
            &mut CellMemory(self.ram),
            record,
            [f.to_bits(), a.to_bits(), b.to_bits()],
        );
    }
    fn trigger_effect(&mut self, id: i32, position: [f32; 3]) {
        self.note("trigger_effect", &[id as u32]);
        // `0xFF8A0(0, id, &position)`: ported
        let (at, _) = self.stage(position, [0.0; 3]);
        let mut leaves = EmberLeaves {
            ram: self.ram,
            trace: self.trace,
            players: self.hw.players,
        };
        trigger_effect(&mut self.mem, &mut leaves, 0, id as u32, at);
    }
    fn settle(&mut self, _position: [f32; 3]) {
        self.note("settle", &[]);
    }
    fn settle_entry(&mut self, world: &EmberWorld, slot: usize, position: [f32; 3]) {
        self.note("settle", &[]);
        // mirror the entry and the kind rows into the cell, then run the ported `0xEDBE0`
        let entry = fabricated::ENTRY_SCRATCH;
        for (k, word) in world.pool.entries[slot].w.iter().enumerate() {
            self.mem.w32(entry + 4 * k as u32, *word);
        }
        for (kind, row) in world.rows.iter().enumerate() {
            let base = KIND_TABLE + 24 * kind as u32;
            let words = [
                row.effect_a as u32,
                row.effect_b as u32,
                row.word_8,
                row.light_radius.to_bits(),
                row.radius.to_bits(),
                row.flags,
            ];
            for (k, word) in words.iter().enumerate() {
                self.mem.w32(base + 4 * k as u32, *word);
            }
        }
        let (at, _) = self.stage(position, [0.0; 3]);
        let mut leaves = EmberLeaves {
            ram: self.ram,
            trace: self.trace,
            players: self.hw.players,
        };
        settle_entry(&mut self.mem, &mut leaves, at, 0, entry);
    }
    fn light(&mut self, position: [f32; 3], radius_squared: f32) {
        self.note("light", &[radius_squared.to_bits()]);
        // `0x186480(&position, radius_squared)`: ported
        let (at, _) = self.stage(position, [0.0; 3]);
        let mut leaves = EmberLeaves {
            ram: self.ram,
            trace: self.trace,
            players: self.hw.players,
        };
        light_pulse(&mut self.mem, &mut leaves, at, radius_squared);
    }
    fn teardown_slot(&mut self, _slot: usize, _entry: &mut EffectEntry) {}
    fn start_kind(&mut self, world: &mut EmberWorld, slot: usize, kind: u32) -> u32 {
        match start_kind_handler(world, self, slot, kind) {
            Some(result) => result,
            None => {
                self.note("start_kind_unhandled", &[slot as u32, kind]);
                0
            }
        }
    }
}

/// The effect pool tick (T1868): the per kind start is the ported `0xEDD40`, the trail and spray services are recorded.
impl TickEngine for CellEmber<'_> {
    fn start_entry(&mut self, world: &mut EmberWorld, slot: usize) -> u32 {
        self.note(
            "tick_start",
            &[slot as u32, world.pool.entries[slot].w[W_KIND]],
        );
        start_effect(world, self, slot)
    }
    fn trail(&mut self, slot: usize, record: &[u32; TRAIL_WORDS], timer: f32) {
        self.note("trail", &[slot as u32, record[7], timer.to_bits()]);
    }
    fn tick_class(&mut self, world: &mut EmberWorld, slot: usize, timer_before: f32) {
        super::svc_pool_classes::class_tick(world, self, slot, timer_before);
    }
    fn trail_with(
        &mut self,
        world: &mut EmberWorld,
        slot: usize,
        record: &[u32; TRAIL_WORDS],
        timer: f32,
        tick: &TickGlobals,
    ) {
        self.trail(slot, record, timer);
        let entry = &mut world.pool.entries[slot];
        let mut counter = entry.w[0xFC / 4];
        super::svc_light::trail_light(
            &mut LightAdapter(self),
            &mut counter,
            record,
            entry.position(),
            timer,
            tick.steps,
            world.globals.frame_rate,
        );
        world.pool.entries[slot].w[0xFC / 4] = counter;
    }
    fn spray(&mut self, call: SprayCall) {
        match call {
            SprayCall::Burst {
                mode,
                owner,
                position,
                velocity,
            } => {
                self.note("spray_burst", &[mode, owner]);
                super::svc_spray::burst(
                    &mut SprayAdapter(self),
                    mode,
                    owner,
                    0,
                    position,
                    velocity,
                );
            }
            SprayCall::Streak {
                position,
                velocity,
                cell,
            } => {
                self.note("spray_streak", &[cell]);
                super::svc_spray::streak(&mut SprayAdapter(self), position, velocity, cell, 2.0, 2);
            }
        }
    }
    fn owner_cell(&mut self, owner: u32) -> u32 {
        self.ram.borrow().word(owner.wrapping_add(0x34))
    }
}

/// The class table of the cell (T1870): callbacks of the other pool classes are not ported, their calls are recorded.
impl super::svc_pool_classes::ClassEngine for CellEmber<'_> {
    fn callback(&self, entry_type: u32, slot: super::svc_pool_classes::ClassSlot) -> u32 {
        self.classes
            .map_or(0, |table| table.callback(entry_type, slot))
    }
    fn run_callback(
        &mut self,
        world: &mut EmberWorld,
        slot: usize,
        which: super::svc_pool_classes::ClassSlot,
        address: u32,
    ) {
        // `0x389B0` is a bare `ret` (the default of several classes): nothing to run, nothing to record
        if address == super::svc_pool_classes::NOOP_CALLBACK {
            return;
        }
        let entry_type = world.pool.entries[slot].w[W_TYPE];
        // the ported update callbacks (T1871, T1876) and init callbacks (T1876) run over the cell (entries are guest words, engine routines recorded)
        let ported = match which {
            super::svc_pool_classes::ClassSlot::Update => {
                super::eff_dispatch::UPDATE_CALLBACKS.contains(&address)
            }
            super::svc_pool_classes::ClassSlot::Init => {
                super::eff_dispatch::INIT_CALLBACKS.contains(&address)
            }
            super::svc_pool_classes::ClassSlot::Late => {
                super::eff_dispatch::LATE_CALLBACKS.contains(&address)
            }
            super::svc_pool_classes::ClassSlot::Destroy => {
                super::eff_dispatch::DESTROY_CALLBACKS.contains(&address)
            }
        };
        if ported {
            let service = match which {
                super::svc_pool_classes::ClassSlot::Init => "effect_init",
                super::svc_pool_classes::ClassSlot::Late => "effect_late",
                super::svc_pool_classes::ClassSlot::Destroy => "effect_destroy",
                _ => "effect_update",
            };
            self.note(service, &[address, slot as u32, entry_type]);
            let base = if world.pool.base != 0 {
                world.pool.base
            } else {
                fabricated::EFFECT_ENTRIES
            };
            let entry = base.wrapping_add(slot as u32 * super::svc_effect_tick::ENTRY_BYTES);
            let mut guest = CellGuest {
                cell: self,
                world,
                base,
            };
            super::eff_dispatch::native(&mut guest, address, &[entry]);
            return;
        }
        self.note(
            "class_callback",
            &[which.offset(), address, slot as u32, entry_type],
        );
    }
    fn read_word(&mut self, address: u32) -> u32 {
        self.ram.borrow().word(address)
    }
    fn write_word(&mut self, address: u32, value: u32) {
        self.ram.borrow_mut().set_word(address, value);
    }
    fn late_prelude(&mut self) {
        self.note("late_prelude", &[]);
    }
}

/// The effect pool entries and the cell RAM as one guest address space for the ported update callbacks (T1871): entry words are the pool
/// entries of the world, every other address is a cell word. Engine routines the port does not implement are RECORDED (`engine_call`, the first
/// four argument words) and answer 0 (FABRICATED neutral: no ray hit, no handle); the pool routines are the ported ones: `0xDC550` releases the slot,
/// `0xDC4A0` allocates a child (type from the argument, a scratch entry when the pool is full), `0xEDD40` starts a waiting entry.
struct CellGuest<'a, 'b> {
    cell: &'a mut CellEmber<'b>,
    world: &'a mut EmberWorld,
    base: u32,
}

impl CellGuest<'_, '_> {
    fn locate(&self, address: u32) -> Option<(usize, usize)> {
        let offset = address.wrapping_sub(self.base);
        let size = super::svc_effect_tick::ENTRY_BYTES;
        let count = self.world.pool.entries.len() as u32;
        (address >= self.base && offset < count * size && offset % 4 == 0)
            .then(|| ((offset / size) as usize, ((offset % size) / 4) as usize))
    }
    fn slot_of(&self, address: u32) -> Option<usize> {
        self.locate(address).map(|(slot, _)| slot)
    }
}

impl super::eff_guest::Guest for CellGuest<'_, '_> {
    fn read(&mut self, address: u32) -> u32 {
        match self.locate(address) {
            Some((slot, word)) => self.world.pool.entries[slot].w[word],
            None => self.cell.ram.borrow().word(address),
        }
    }
    fn write(&mut self, address: u32, value: u32) {
        match self.locate(address) {
            Some((slot, word)) => self.world.pool.entries[slot].w[word] = value,
            None => self.cell.ram.borrow_mut().set_word(address, value),
        }
    }
    fn call(&mut self, va: u32, args: &[u32]) -> u32 {
        if let Some(result) = super::eff_dispatch::native(self, va, args) {
            return result;
        }
        match va {
            0xDC550 => {
                if let Some(slot) = args.first().and_then(|entry| self.slot_of(*entry)) {
                    self.world.pool.release(self.cell, slot);
                }
                0
            }
            0xDC4A0 => {
                let wanted = args.first().copied().unwrap_or(0);
                match self.world.pool.allocate(self.cell, wanted) {
                    Some(slot) => self
                        .base
                        .wrapping_add(slot as u32 * super::svc_effect_tick::ENTRY_BYTES),
                    None => fabricated::ENTRY_SCRATCH,
                }
            }
            0xEDD40 => {
                let slot = args.first().and_then(|entry| self.slot_of(*entry));
                slot.map_or(0, |slot| start_effect(self.world, self.cell, slot))
            }
            _ => {
                let words: Vec<u32> = std::iter::once(va)
                    .chain(args.iter().copied().take(4))
                    .collect();
                self.cell.note("engine_call", &words);
                0
            }
        }
    }
    fn call_f(&mut self, va: u32, args: &[u32]) -> f32 {
        f32::from_bits(self.call(va, args))
    }
}

/// The ported trail and light service `0xBB850` (T1870) over the cell: objects and rooms are guest memory words of the cell, the engine
/// routines (`0x873C0` room query, `0x12A590`, `0x3533F0`, `0x3568C0`, `0x2220E0`, `0x88C60`, `0x919D0`, `0xBB6C0`, `0xF4920`) are recorded
/// with FABRICATED neutral answers (no room hit, no veto, no occluder, no contact).
struct LightAdapter<'a, 'b>(&'a mut CellEmber<'b>);

impl super::svc_light::LightHost for LightAdapter<'_, '_> {
    fn read(&mut self, address: u32) -> u32 {
        self.0.ram.borrow().word(address)
    }
    fn write(&mut self, address: u32, value: u32) {
        self.0.ram.borrow_mut().set_word(address, value);
    }
    fn rng(&mut self) -> u32 {
        self.0.rng()
    }
    fn ground_effect(&mut self, position: [f32; 3], height: f32, word: u32) {
        self.0.note(
            "light_ground",
            &[
                position[0].to_bits(),
                position[1].to_bits(),
                position[2].to_bits(),
                height.to_bits(),
                word,
            ],
        );
    }
    fn rooms_in_box(&mut self, region: [f32; 6]) -> Option<Vec<u32>> {
        self.0.note("light_rooms", &region.map(f32::to_bits));
        Some(Vec::new())
    }
    fn light_room(&mut self, room_object: u32, strength: f32, _region: [f32; 6]) {
        self.0
            .note("light_room", &[room_object, strength.to_bits()]);
    }
    fn light_object(&mut self, object: u32, strength: f32) {
        self.0.note("light_object", &[object, strength.to_bits()]);
    }
    fn actor_gate(&mut self) -> bool {
        false
    }
    fn actor_check(&mut self, _value: u32) -> bool {
        true
    }
    fn actor_centre(&mut self, _object: u32) -> [f32; 3] {
        [0.0; 3]
    }
    fn occluded(&mut self, _cell: u32, _from: [f32; 3], _vector: [f32; 3]) -> bool {
        false
    }
    fn contact(
        &mut self,
        _word: u32,
        _position: [f32; 3],
        _direction: [f32; 3],
    ) -> Option<([f32; 3], [f32; 3])> {
        None
    }
    fn apply(&mut self, call: super::svc_light::LightCall) {
        self.0.note(
            "light_apply",
            &[
                call.object,
                call.intensity.to_bits(),
                call.distance.to_bits(),
                call.first,
            ],
        );
    }
}

/// The ported spray services (T1870) over the cell: emitters are created through the ported `0x111AE0`, the owner mask and the streak
/// sprite (`0x1128C0`) are recorded. `0x80F20` (the sprite table of the image) answers the cell word, 0 when nothing was loaded.
struct SprayAdapter<'a, 'b>(&'a mut CellEmber<'b>);

impl super::svc_spray::SprayHost for SprayAdapter<'_, '_> {
    fn rng(&mut self) -> u32 {
        self.0.rng()
    }
    fn word(&mut self, address: u32) -> u32 {
        self.0.ram.borrow().word(address)
    }
    fn create_emitter(
        &mut self,
        _handle: u32,
        template: u32,
        flags: u32,
        position: [f32; 3],
        vector: [f32; 3],
    ) {
        self.0.create(template, flags, position, vector);
    }
    fn emitter_mask(&mut self, mask: u32) {
        self.0.note("emitter_mask", &[mask]);
    }
    fn sprite_value(&mut self, index: u32) -> u32 {
        self.0.ram.borrow().word(0x004D_2D64 + 24 * index)
    }
    fn streak_sprite(&mut self, call: super::svc_spray::StreakSprite) {
        self.0.note(
            "streak_sprite",
            &[
                call.sprite,
                call.kind as u32,
                call.length.to_bits(),
                call.cell,
            ],
        );
    }
}

/// The per kind handlers of `0xEDD40` (T1863) run in full; the emitters and the other engine calls they make are recorded.
impl KindEngine for CellEmber<'_> {
    fn guest(&mut self) -> &mut dyn GuestMemory {
        &mut self.mem
    }
    fn emitter(&mut self, template: u32, flags: u32, position: [f32; 3], direction: [f32; 3]) {
        let mut words = vec![template, flags];
        words.extend(position.iter().chain(&direction).map(|v| v.to_bits()));
        self.note("emitter", &words);
        self.create(template, flags, position, direction);
    }
    fn emitter_param(&mut self, param: EmitterParam) {
        let (name, words): (&'static str, Vec<u32>) = match param {
            EmitterParam::Scale(v) => ("emitter_scale", vec![v.to_bits()]),
            EmitterParam::Span(a, b) => ("emitter_span", vec![a.to_bits(), b.to_bits()]),
            EmitterParam::Param70(v) => ("emitter_param70", vec![v.to_bits()]),
            EmitterParam::Param74(v) => ("emitter_param74", vec![v.to_bits()]),
            EmitterParam::Vec98(v) => ("emitter_vec98", v.iter().map(|c| c.to_bits()).collect()),
            EmitterParam::Attach(handle) => ("emitter_attach", vec![handle]),
        };
        self.note(name, &words);
        apply_param(&mut self.mem, &param);
    }
    fn actor_kick(&mut self, handle: u32) {
        self.note("actor_kick", &[handle]);
    }
    fn actor_gate(&mut self, actor: u32) -> bool {
        self.note("actor_gate", &[actor]);
        false
    }
    fn flash(&mut self, vector: [f32; 3], origin: [f32; 3]) {
        let words: Vec<u32> = vector.iter().chain(&origin).map(|v| v.to_bits()).collect();
        self.note("flash", &words);
    }
    fn beam(&mut self, handle: u32, position: [f32; 3], direction: [f32; 3]) {
        let mut words = vec![handle];
        words.extend(position.iter().chain(&direction).map(|v| v.to_bits()));
        self.note("beam", &words);
    }
    fn screen_flash(&mut self, value: f32) {
        self.note("screen_flash", &[value.to_bits()]);
    }
    fn debris(&mut self, call: DebrisCall) -> u32 {
        match call {
            DebrisCall::Create { class, .. } => {
                self.note("debris_create", &[class]);
                self.debris_made += 1;
                fabricated::DEBRIS_HEAP + 0x400 * (self.debris_made - 1)
            }
            DebrisCall::Handle { object } => {
                self.note("debris_handle", &[object]);
                fabricated::DEBRIS_HANDLES + 0x10 * self.debris_made
            }
            DebrisCall::Bind { handle, object } => {
                self.note("debris_bind", &[handle, object]);
                0
            }
            DebrisCall::Limit { handle } => {
                self.note("debris_limit", &[handle]);
                0
            }
            DebrisCall::Launch { handle, .. } => {
                self.note("debris_launch", &[handle]);
                0
            }
            DebrisCall::Finish {
                object,
                life,
                count,
            } => {
                self.note("debris_finish", &[object, life.to_bits(), count]);
                0
            }
        }
    }
}

/// An ember world with a free pool of `slots` entries in class 0, the kind rows undefined (FABRICATED: the stock kind
/// table is game data the host has not loaded) and one [`PlayerSlot`] per player.
pub fn fabricated_ember_world(slots: usize, players: usize) -> EmberWorld {
    let mut entries = vec![EffectEntry::default(); slots];
    for (index, entry) in entries.iter_mut().enumerate() {
        entry.w[W_TYPE] = u32::MAX;
        entry.w[W_PREV] = (index as i32 - 1) as u32;
        entry.w[W_NEXT] = if index + 1 < slots {
            index as u32 + 1
        } else {
            u32::MAX
        };
    }
    EmberWorld {
        pool: EffectPool {
            entries,
            head: vec![0],
            tail: vec![slots as i32 - 1],
            class_of_type: vec![0; 0x40],
            dirty: 0,
            base: 0,
        },
        rows: vec![
            KindRow {
                effect_a: -1,
                effect_b: -1,
                ..KindRow::default()
            };
            0x40
        ],
        effect_defined: vec![false; 0x400],
        players: vec![PlayerSlot::default(); players],
        ..EmberWorld::default()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Impact path
// ---------------------------------------------------------------------------------------------------------------

/// Ground height service (`0xC3E60`) stand-in: a flat floor at `height`.
pub struct FlatGround {
    pub height: f32,
}

impl ImpactServices for FlatGround {
    fn disturb(&mut self, _position: [f32; 3], _radius: f32, _flags: u32) {}
    fn spawn_request(&mut self, _record: [u32; 12]) {}
    fn build_debris(&mut self, _slot: u32) {}
    fn ground_height(&mut self, _point: [f32; 3], _y_bias: f32, _half_extent: f32) -> f32 {
        self.height
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Hook services
// ---------------------------------------------------------------------------------------------------------------

/// What one run of a wired hook entry point did.
#[derive(Clone, Debug, PartialEq)]
pub struct ServiceReport {
    /// Address of the hook function (`0x1F48A0` or `0x1E2B80`).
    pub function: u32,
    /// `Some` when the ported function took the divide fault the original would take.
    pub fault: Option<SpawnerFault>,
    /// Trace events of this run (the new tail of [`HookServices::trace`]).
    pub trace: Vec<TraceEvent>,
}

/// The guest memory, the pools and the recorded calls the wired hooks share. One per world.
pub struct HookServices {
    pub ram: RefCell<GuestRam>,
    trace: Trace,
    pub rng: Lcg64,
    pub ember: EmberWorld,
    /// The class table `0x4DEB28` read from the image by [`HookServices::load_kind_tables`] (empty until then: no class callbacks).
    pub classes: super::svc_pool_classes::ClassTable,
    players: u32,
}

impl HookServices {
    /// `players` is the local player count (1 or 2): the player array, the count `[0x790950]` and the prelude player
    /// pointer are laid out in the cell.
    pub fn new(players: u32) -> Self {
        let mut ram = GuestRam::default();
        ram.set_word(GLOBAL_PLAYER_ARRAY, fabricated::PLAYER_RECORDS);
        ram.set_word(GLOBAL_FIRST_SLOT, players);
        ram.set_word(GLOBAL_SLOT_COUNT, 0);
        ram.set_word(GLOBAL_SLOT_BASE, fabricated::PLAYER_RECORDS);
        for index in 0..players {
            let record = fabricated::PLAYER_RECORDS + 0x1584 * index;
            let object = fabricated::PLAYER_OBJECTS + 0x400 * index;
            ram.set_word(record + 8, object);
            // object byte +0x21C bit 2: the player can be pushed
            ram.set_byte(object + 0x21C, 4);
        }
        install_pool_world(&mut ram);
        ram.set_word(0x7B_0C7C, fabricated::PRELUDE_PLAYER);
        // `[0x4E8CEC]` = 60 and `[0x4E7944]` = 1 (frame rate and step), read by the prelude.
        ram.set_word(0x4E_8CEC, 60);
        ram.set_word(0x4E_7944, 1);
        emitter_pool_init(&mut ram, fabricated::EMITTER_POOL, 64);
        // an empty pulse grid: every cell head is -1 (zeroed memory would read as a node 0 chaining to itself)
        ram.set_word(PULSE_GRID, fabricated::PULSE_ROWS);
        for row in 0..32u32 {
            ram.set_word(
                fabricated::PULSE_ROWS + 4 * row,
                fabricated::PULSE_ROWS + 0x100,
            );
            ram.set_word(fabricated::PULSE_ROWS + 0x100 + 2 * row, 0xFFFF_FFFF);
        }
        // `0x153B40` seed of a fresh boot: the state words start at 0.
        HookServices {
            ram: RefCell::new(ram),
            trace: RefCell::new(Vec::new()),
            rng: Lcg64::default(),
            ember: fabricated_ember_world(16, players as usize),
            classes: super::svc_pool_classes::ClassTable::default(),
            players,
        }
    }

    pub fn players(&self) -> u32 {
        self.players
    }

    /// `0xEFBD0(kind, ...)` for any kind over the cell: the 31 kind handlers of `0xEDD40` (T1863) run, their emitters are
    /// created in the cell's emitter pool and the other engine services run their ported code (leaves recorded).
    pub fn spawn_effect(&mut self, args: &SpawnArgs) -> SpawnResult {
        let mut engine = CellEmber {
            ram: &self.ram,
            mem: CellMemory(&self.ram),
            debris_made: 0,
            classes: None,
            trace: &self.trace,
            hw: RecordingHw {
                trace: &self.trace,
                players: self.players as i32,
            },
        };
        spawn_effect(&mut self.ember, &mut engine, args)
    }

    /// `0x111AE0(0, template, flags, &position, &vector)` over the cell (T1867): the ported emitter creation. Returns its
    /// result (1 = created, 0 = pool empty).
    pub fn create_emitter(
        &mut self,
        template: u32,
        flags: u32,
        position: [f32; 3],
        vector: [f32; 3],
    ) -> u32 {
        let mut engine = CellEmber {
            ram: &self.ram,
            mem: CellMemory(&self.ram),
            debris_made: 0,
            classes: None,
            trace: &self.trace,
            hw: RecordingHw {
                trace: &self.trace,
                players: self.players as i32,
            },
        };
        engine.create(template, flags, position, vector);
        let last = self
            .trace
            .borrow()
            .iter()
            .rev()
            .find_map(|event| match event {
                TraceEvent::Ember {
                    service: "emitter_created",
                    args,
                } => args.get(2).copied(),
                _ => None,
            });
        last.unwrap_or(0)
    }

    /// One frame of the effect pool tick `0xDC7B0` (T1868) over the cell: waiting entries (negative timer) count up by
    /// `steps / frame rate` and start when they reach zero (`0xEDD40`, the ported kind starts), live entries report their
    /// trail record and are released after 1.25 seconds.
    pub fn tick_effects(&mut self, steps: i32) {
        {
            // the guest frame globals the ported update callbacks read (T1871)
            let mut ram = self.ram.borrow_mut();
            ram.set_word(0x004E_7944, steps as u32);
            ram.set_float(0x004E_794C, steps as f32);
            ram.set_word(0x004E_8CEC, self.ember.globals.frame_rate as u32);
        }
        let mut engine = CellEmber {
            ram: &self.ram,
            mem: CellMemory(&self.ram),
            debris_made: 0,
            classes: Some(&self.classes),
            trace: &self.trace,
            hw: RecordingHw {
                trace: &self.trace,
                players: self.players as i32,
            },
        };
        pool_tick(&mut self.ember, &mut engine, &TickGlobals::frame(steps));
        super::svc_pool_classes::late_loop(&mut self.ember, &mut engine);
    }

    /// The class init callback (`+0x30`) of pool slot `slot` over the cell (T1876): the 19 ported init callbacks run as guest code, the rest is recorded.
    /// The original calls it from the spawn code after the entry is filled.
    pub fn init_effect(&mut self, slot: usize) {
        let mut engine = CellEmber {
            ram: &self.ram,
            mem: CellMemory(&self.ram),
            debris_made: 0,
            classes: Some(&self.classes),
            trace: &self.trace,
            hw: RecordingHw {
                trace: &self.trace,
                players: self.players as i32,
            },
        };
        super::svc_pool_classes::class_init(&mut self.ember, &mut engine, slot);
    }

    /// `0xDC960`: the destroy loop over the pool (level teardown), with the class destroy callbacks recorded (T1870).
    pub fn destroy_effects(&mut self) {
        let mut engine = CellEmber {
            ram: &self.ram,
            mem: CellMemory(&self.ram),
            debris_made: 0,
            classes: Some(&self.classes),
            trace: &self.trace,
            hw: RecordingHw {
                trace: &self.trace,
                players: self.players as i32,
            },
        };
        super::svc_pool_classes::destroy_loop(&mut self.ember, &mut engine);
    }

    /// Load the effect kind rows `0x4DB490` and the effect definitions `0x4E0FA0` from an XBE image (T1867). Returns
    /// false (nothing changed) when the image does not map them.
    pub fn load_kind_tables(&mut self, image: &[u8]) -> bool {
        if let Some(classes) = super::svc_pool_classes::ClassTable::from_image(image) {
            self.classes = classes;
        }
        super::svc_kind_table::install_tables(&mut self.ember, image)
    }

    /// Define the particle templates (`0x78DBA0`, `0x78ED40`, `0x7B0A80`, the ember pair and the rest of the boot table) in the
    /// cell by running the original definer code of a caller supplied XBE image through the ported setters (T1868). Returns
    /// the definer reports, or the error of the first definer the interpreter could not run (the cell may be partly written).
    pub fn load_templates(
        &mut self,
        image: &[u8],
    ) -> Result<Vec<(u32, super::svc_template_defs::DefReport)>, super::svc_template_defs::DefError>
    {
        super::svc_template_defs::load_templates(image, &mut CellMemory(&self.ram))
    }

    /// Every recorded call so far.
    pub fn trace(&self) -> Vec<TraceEvent> {
        self.trace.borrow().clone()
    }

    fn hw(&self) -> RecordingHw<'_> {
        RecordingHw {
            trace: &self.trace,
            players: self.players as i32,
        }
    }

    /// `0x1F48A0` over the full service stack: [`ObjectServices`] (lookups, flags, destroy `0xBD090`), then
    /// [`ParticleStripServices`] (`0x217B0` through the sound submit, `0x129E0`, `0x12B840`) and last the glue
    /// (`0xA14B0`, `0x151E80`, `0xC1950`). Every service call is traced with its result.
    pub fn run_strip_spawner(&mut self) -> ServiceReport {
        let start = self.trace.borrow().len();
        let mut recording = RecordingDestroy { trace: &self.trace };
        let mut destroy = PortedDestroy {
            inner: &mut recording,
            entry: fabricated::DESTROY_FRAME - 0x68,
        };
        let mut sink = CellSink {
            ram: &self.ram,
            hw: RecordingHw {
                trace: &self.trace,
                players: self.players as i32,
            },
        };
        let mut glue = GlueServices {
            spawner: SpawnAdapter {
                callees: NativeCallees {
                    leaves: RecordingLeaves { trace: &self.trace },
                },
                scratch_frame: fabricated::SPAWN_FRAME,
            },
        };
        let mut particles = ParticleStripServices {
            sink: &mut sink,
            inner: &mut glue,
        };
        let mut objects = ObjectServices {
            rest: &mut particles,
            destroy: &mut destroy,
            frame: fabricated::DESTROY_FRAME,
        };
        let mut traced = TracedServices {
            inner: &mut objects,
            trace: &self.trace,
        };
        let mut memory = CellMemory(&self.ram);
        let fault = strip_spawner(
            &mut memory,
            &mut traced,
            &mut self.rng,
            fabricated::STRIP_FRAME,
        )
        .err();
        ServiceReport {
            function: 0x1F48A0,
            fault,
            trace: self.trace.borrow()[start..].to_vec(),
        }
    }

    /// `0xC1950(args)` alone (the `Service::SpawnObject` arm of the strip driver): returns the object (0 = failed).
    pub fn spawn_object(&mut self, args: [u32; 10]) -> u32 {
        let mut adapter = SpawnAdapter {
            callees: NativeCallees {
                leaves: RecordingLeaves { trace: &self.trace },
            },
            scratch_frame: fabricated::SPAWN_FRAME,
        };
        adapter.spawn(&mut CellMemory(&self.ram), &args)
    }

    /// `0x1E2B80` (levels 4 and 0x50): the ported prelude over the shared cell, every effect executed by its service.
    pub fn run_prelude(&mut self) -> ServiceReport {
        let start = self.trace.borrow().len();
        {
            let mut env = CellPrelude { services: self };
            prelude_1e2b80(&mut env);
        }
        ServiceReport {
            function: 0x1E2B80,
            fault: None,
            trace: self.trace.borrow()[start..].to_vec(),
        }
    }

    /// Runs `body` with the `ImpactServices` stack over the shared cell: effect submit and disturbance
    /// ([`EffectSubmitServices`]), the debris builder `0x103A00` ([`TeardownImpactServices`]) and a flat ground.
    /// `memory` is the proxy to pass as the `EffectMemory` argument of the sweep functions.
    pub fn impact_services<R>(
        &mut self,
        ground: f32,
        body: impl FnOnce(&mut CellMemory<'_>, &mut dyn ImpactServices) -> R,
    ) -> R {
        let mut hw = self.hw();
        let mut submit_memory = CellMemory(&self.ram);
        let mut flat = FlatGround { height: ground };
        let submit = EffectSubmitServices {
            memory: &mut submit_memory,
            hw: &mut hw,
            inner: &mut flat,
            scratch: fabricated::SUBMIT_SCRATCH,
            submitted: Vec::new(),
        };
        let mut services = TeardownImpactServices {
            memory: &self.ram,
            inner: submit,
            hooks: RecordedCallbacks::default(),
        };
        let mut memory = CellMemory(&self.ram);
        body(&mut memory, &mut services)
    }
}

/// `PreludeEnv` over the shared cell: reads and writes hit the RAM, every effect runs its ported service.
struct CellPrelude<'a> {
    services: &'a mut HookServices,
}

impl PreludeEnv for CellPrelude<'_> {
    fn read(&self, address: u32) -> u32 {
        self.services.ram.borrow().word(address)
    }
    fn write(&mut self, address: u32, value: u32) {
        self.services.ram.borrow_mut().set_word(address, value);
    }
    fn effect(&mut self, effect: PreludeEffect) {
        let services = &mut *self.services;
        let mut memory = CellMemory(&services.ram);
        match &effect {
            PreludeEffect::Particle { .. } => {
                let mut engine = CellEmber {
                    ram: &services.ram,
                    mem: CellMemory(&services.ram),
                    debris_made: 0,
                    classes: None,
                    trace: &services.trace,
                    hw: RecordingHw {
                        trace: &services.trace,
                        players: services.players as i32,
                    },
                };
                run_prelude_particle(&mut services.ember, &mut engine, &effect);
            }
            PreludeEffect::SpawnObject { words } => {
                let mut sink = CellSink {
                    ram: &services.ram,
                    hw: RecordingHw {
                        trace: &services.trace,
                        players: services.players as i32,
                    },
                };
                sink.submit(words);
            }
            PreludeEffect::Sound1A6 { value } => {
                let mut sink = CellSink {
                    ram: &services.ram,
                    hw: RecordingHw {
                        trace: &services.trace,
                        players: services.players as i32,
                    },
                };
                sound_1(&mut sink, 0x1A6, value.to_bits());
            }
            PreludeEffect::Sound1A7 { count } => {
                let mut sink = CellSink {
                    ram: &services.ram,
                    hw: RecordingHw {
                        trace: &services.trace,
                        players: services.players as i32,
                    },
                };
                sound_2(&mut sink, 0x1A7, *count as u32, 0x3F00_0000);
            }
            PreludeEffect::PlayerNudge { player, a, b } => {
                push_player(
                    &mut memory,
                    *player,
                    [0x3F19_999A, a.to_bits(), b.to_bits()],
                );
            }
            PreludeEffect::Release { entry } => {
                let mut recording = RecordingDestroy {
                    trace: &services.trace,
                };
                let mut destroy = PortedDestroy {
                    inner: &mut recording,
                    entry: fabricated::DESTROY_FRAME - 0x68,
                };
                destroy_object(&mut memory, &mut destroy, *entry, fabricated::DESTROY_FRAME);
            }
            PreludeEffect::Light {
                target,
                select,
                value,
            } => {
                let mut hw = RecordingHw {
                    trace: &services.trace,
                    players: services.players as i32,
                };
                sound_starter(
                    &mut memory,
                    &mut hw,
                    StarterArgs {
                        handle_ptr: *target,
                        position: 0,
                        id: *select as i32,
                        gain: value.to_bits(),
                        radius: 0x4248_0000,
                        start_value: 0x3F80_0000,
                        flags: 0,
                    },
                    fabricated::SUBMIT_SCRATCH + 0x100,
                );
            }
            PreludeEffect::Sweep(query) => {
                services.trace.borrow_mut().push(TraceEvent::Sweep(*query));
            }
            PreludeEffect::Refresh { object } => refresh_regions(&mut memory, *object),
            PreludeEffect::Flash => push_all_players(&mut memory, [0x3DCC_CCCD; 3]),
        }
        services
            .trace
            .borrow_mut()
            .push(TraceEvent::Prelude(effect));
    }
    fn sweep(&mut self, query: &SweepQuery) -> SweepResult {
        prelude_sweep(&mut NullGeometry, query, [0.0; 4])
    }
}
