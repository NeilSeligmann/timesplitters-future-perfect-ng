// SPDX-License-Identifier: GPL-3.0-or-later
//! T2241 TEMPORARY-SHIM: read-only compatibility inlet from the hybrid object/view owners.
//! Removal: T2244 replaces this inlet with native owner commands. All sampled data is owned
//! Rust state; presentation never reads guest memory, runs lifted code or writes guest state.
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use tsfp_engine_anim::display::{ClipSource, DecodedClips, Sampling};
use tsfp_engine_anim::instance::ClipView;
use tsfp_engine_anim::posemath::{Mat4, Sample};
use tsfp_engine_anim::presentation::{ModelCorrection, Nodes, Placement as ViewPlacement, Space};
use tsfp_engine_anim::prop_display::{Placement, PropDisplay, PropTick};
use tsfp_format_anim::{Clip, Node, track_len};

#[derive(Clone, Copy)]
struct Config {
    enabled: bool,
    alpha: Option<f32>,
}
fn config() -> Config {
    static CONFIG: OnceLock<Config> = OnceLock::new();
    *CONFIG.get_or_init(|| {
        let enabled = std::env::var("TSFP_ANIMATION_FRACTIONAL").is_ok_and(|s| s == "1");
        let alpha = std::env::var("TSFP_ANIMATION_ALPHA").ok();
        let parsed = alpha.as_ref().and_then(|s| s.parse::<f32>().ok());
        if alpha.is_some() && !parsed.is_some_and(|v| v.is_finite() && (0.0..=1.0).contains(&v)) {
            eprintln!("native animation: invalid TSFP_ANIMATION_ALPHA; feature disabled");
            return Config {
                enabled: false,
                alpha: None,
            };
        }
        Config {
            enabled,
            alpha: parsed,
        }
    })
}

trait ReadMemory {
    fn bytes(&self, address: u32, len: usize) -> Option<Vec<u8>>;
    fn word(&self, address: u32) -> Option<u32> {
        Some(u32::from_le_bytes(self.bytes(address, 4)?.try_into().ok()?))
    }
    fn float(&self, address: u32) -> Option<f32> {
        Some(f32::from_bits(self.word(address)?))
    }
    fn byte(&self, address: u32) -> Option<u8> {
        Some(*self.bytes(address, 1)?.first()?)
    }
    fn matrix(&self, address: u32) -> Option<Mat4> {
        let b = self.bytes(address, 64)?;
        let matrix =
            std::array::from_fn(|i| f32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap()));
        matrix.iter().all(|v| v.is_finite()).then_some(matrix)
    }
}

#[derive(Default, Clone)]
struct Clips {
    clips: DecodedClips,
    durations: BTreeMap<u32, f32>,
}
impl ClipSource for Clips {
    fn view(&self, id: u32) -> Option<ClipView<'_>> {
        let view = self.clips.view(id)?;
        Some(ClipView {
            duration: *self.durations.get(&id)?,
            ..view
        })
    }
    fn sample(&self, id: u32, node: usize, key: i32) -> Option<Sample> {
        self.clips.sample(id, node, key)
    }
}

/// Exact resident input contract. Streamed key caches are outside this inlet's
/// admission and keep their fixed output; no missing sample becomes an identity.
fn resident_clip(m: &impl ReadMemory, pointer: u32) -> Option<(Clip, f32)> {
    let header_bytes = m.bytes(pointer, 56)?;
    let header: [u32; 14] = std::array::from_fn(|i| {
        u32::from_le_bytes(header_bytes[i * 4..i * 4 + 4].try_into().unwrap())
    });
    let flags = header[2];
    let keys = header[4] as usize;
    let nodes = header[5] as usize;
    if flags & 2 != 0 || flags > 31 || keys == 0 || keys > 65536 || nodes != 1 {
        return None;
    }
    let duration = f32::from_bits(header[6]);
    if !duration.is_finite() {
        return None;
    }
    let kb = m.bytes(header[7], keys.checked_mul(4)?)?;
    let key_frames = kb
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    let nb = m.bytes(header[10], 32)?;
    let node = Node {
        words: std::array::from_fn(|i| {
            u32::from_le_bytes(nb[i * 4..i * 4 + 4].try_into().unwrap())
        }),
    };
    let len = track_len(node.selector(), flags, keys).ok()?;
    if len > 2 * 1024 * 1024 {
        return None;
    }
    let track = m.bytes(node.words[5], len)?;
    Some((
        Clip {
            header,
            key_frames,
            nodes: vec![node],
            distances: Vec::new(),
            tracks: vec![track],
            tail: Vec::new(),
        },
        duration,
    ))
}

#[derive(Clone, Copy)]
struct View {
    base: Mat4,
    camera: Option<Mat4>,
    correction: ModelCorrection,
    use_palette: bool,
}
struct Actor {
    id: u64,
    model: u32,
    clip: u32,
    frame: u64,
    display: PropDisplay,
    seen: Instant,
}
#[derive(Clone, Copy)]
struct Range {
    body: u32,
    player: u32,
    start: u32,
    nodes: usize,
    frame: u64,
    view: View,
}
/// Owned native matrix expressions retain explicit copy/multiply provenance.
/// Original temporary addresses exist only in DrawScope's compatibility map.
#[derive(Clone)]
enum Expression {
    Constant(Mat4),
    Native {
        id: u64,
        frame: u64,
        node: usize,
        view: View,
        display: PropDisplay,
        source: Clips,
    },
    Multiply(Arc<Expression>, Arc<Expression>),
    Transpose(Arc<Expression>),
}
impl Expression {
    fn native(&self) -> bool {
        match self {
            Self::Constant(_) => false,
            Self::Native { .. } => true,
            Self::Multiply(a, b) => a.native() || b.native(),
            Self::Transpose(a) => a.native(),
        }
    }
    fn eval(&self, alpha: f32) -> Option<Mat4> {
        match self {
            Self::Constant(m) => Some(*m),
            Self::Native {
                node,
                view,
                display,
                source,
                ..
            } => {
                let sampled = display.sample(source, Sampling::Fractional, alpha).ok()?;
                let mut object = view.base;
                object[12..15].copy_from_slice(&sampled.position);
                let p = ViewPlacement {
                    space: match view.camera {
                        Some(camera) => Space::World { object, camera },
                        None => Space::View(object),
                    },
                    correction: view.correction.matrix().ok()?,
                };
                let nodes = p
                    .place(if view.use_palette {
                        Nodes::Skinned(&sampled.matrices)
                    } else {
                        Nodes::Repeated(std::num::NonZeroUsize::new(sampled.matrices.len())?)
                    })
                    .ok()?;
                nodes.get(*node).copied()
            }
            Self::Multiply(a, b) => Some(tsfp_engine_anim::posemath::mat_mul(
                &a.eval(alpha)?,
                &b.eval(alpha)?,
            )),
            Self::Transpose(a) => {
                let m = a.eval(alpha)?;
                Some(std::array::from_fn(|i| m[(i % 4) * 4 + i / 4]))
            }
        }
    }
    fn identity(&self) -> Option<(u64, u64)> {
        match self {
            Self::Native { id, frame, .. } => Some((*id, *frame)),
            Self::Constant(_) => None,
            Self::Multiply(a, b) => a.identity().or_else(|| b.identity()),
            Self::Transpose(a) => a.identity(),
        }
    }
    fn depth(&self) -> usize {
        match self {
            Self::Multiply(a, b) => 1 + a.depth().max(b.depth()),
            Self::Transpose(a) => 1 + a.depth(),
            _ => 1,
        }
    }
}
struct Pending {
    va: u32,
    destination: u32,
    expression: Option<Arc<Expression>>,
    slot: bool,
}
static MODEL_SCOPES: AtomicU64 = AtomicU64::new(0);
static ELIGIBLE_SCOPES: AtomicU64 = AtomicU64::new(0);
static MATRIX_OPS: AtomicU64 = AtomicU64::new(0);
static NATIVE_EXPRESSIONS: AtomicU64 = AtomicU64::new(0);
struct DrawScope {
    body: u32,
    frame: u64,
    eligible: bool,
    derived: BTreeMap<u32, Arc<Expression>>,
    pending: Vec<Pending>,
}
struct Slot {
    expression: Arc<Expression>,
    frame: u64,
    seen: Instant,
}

#[derive(Default, Clone, Copy)]
struct Stats {
    observations: u64,
    actors: u64,
    views: u64,
    bindings: u64,
    uploads: u64,
    changed: u64,
    rejected: u64,
    mismatches: u64,
}
#[derive(Default)]
struct Bank {
    clips: Clips,
    actors: BTreeMap<u32, Actor>,
    matrices: BTreeMap<(u32, u32), (Mat4, u64)>,
    ranges: Vec<Range>,
    slots: BTreeMap<u32, Slot>,
    draws: Vec<DrawScope>,
    next_id: u64,
    stats: Stats,
}
fn close(a: &Mat4, b: &Mat4) -> bool {
    a.iter().zip(b).all(|(a, b)| {
        a.is_finite() && b.is_finite() && (a - b).abs() <= 1e-5 + 1e-5 * a.abs().max(b.abs())
    })
}
impl Bank {
    fn note(&mut self, va: u32, args: [u32; 4], frame: u64, m: &impl ReadMemory) {
        self.stats.observations += 1;
        let result = match va {
            0x5f040 => self.actor(args[0], frame, m),
            0xa14b0 => self.object_matrix(args, frame, m),
            0xa1b80 => self.views(args[0], frame, m),
            0x22520 => self.binding(args[0], args[1], frame, m),
            _ => Some(()),
        };
        if result.is_none() {
            self.stats.rejected += 1;
            if va == 0x5f040 {
                self.actors.remove(&args[0]);
                self.ranges.retain(|r| r.body != args[0]);
                self.slots.clear(); // failed source cannot retain earlier temporary bindings
            }
        }
    }
    fn actor(&mut self, body: u32, frame: u64, m: &impl ReadMemory) -> Option<()> {
        // Body/model pointers occur only in this replaceable inlet, never native IDs.
        let model = m.word(body)?;
        let info = m.word(body.checked_add(4)?)?;
        let count = m.word(info.checked_add(4)?)? as usize;
        if count == 0 || count > 40 {
            self.actors.remove(&body);
            return None;
        }
        let slot = m.word(body.checked_add(0xd8)?)?;
        if slot >= 4096 {
            self.actors.remove(&body);
            return None;
        }
        let table = m.word(0x7e3954)?;
        let clip = m.word(table.checked_add(slot.checked_mul(4)?)?)?;
        let (decoded, duration) = resident_clip(m, clip)?;
        // Normalize the legacy key-time domain to the native 60 Hz clip domain.
        // This is a read-only inlet conversion, not a simulation time change.
        let fps = m.float(0x4d1994)?;
        if !fps.is_finite() || fps <= 0.0 {
            return None;
        }
        let scale = fps / 60.0;
        let time = m.float(body.checked_add(0xdc)?)? * scale;
        let speed = m.float(body.checked_add(0x108)?)?;
        let rate = m.float(0x4e794c)?;
        let per = m.word(0x4e8cec)? as i32 as f32;
        let step = speed * rate / per * scale;
        let duration = duration * scale;
        if !time.is_finite() || !step.is_finite() || !duration.is_finite() {
            return None;
        }
        let changed_clip = self.clips.clips.0.get(&clip) != Some(&decoded);
        self.clips.clips.0.insert(clip, decoded);
        self.clips.durations.insert(clip, duration);
        let reset = self.actors.get(&body).is_none_or(|a| {
            a.model != model
                || a.clip != clip
                || (a.frame != frame && a.frame.checked_add(1) != Some(frame))
                || time < a.display.ticks().1.time
                || changed_clip
        });
        let generation = if reset {
            self.next_id = self.next_id.checked_add(1)?;
            self.next_id
        } else {
            self.actors.get(&body)?.id
        };
        let tick = PropTick {
            generation,
            clip,
            time,
            time_step: step,
        };
        let candidate = if reset {
            PropDisplay::new(tick, Placement::Model, count, &self.clips).ok()?
        } else {
            let mut d = self.actors.get(&body)?.display.clone();
            d.publish(tick, &self.clips).ok()?;
            d
        };
        // Qualify the fixed result before admitting this legacy inlet/model.
        let actual = m.word(body.checked_add(0x10)?)?;
        let expected = candidate.committed();
        for (i, want) in expected.matrices.iter().enumerate() {
            let got = m.matrix(actual.checked_add((i as u32).checked_mul(64)?)?)?;
            if !close(want, &got) {
                self.stats.mismatches += 1;
                self.actors.remove(&body);
                return None;
            }
        }
        let position: [f32; 3] =
            std::array::from_fn(|i| m.float(model + 0x64 + 4 * i as u32).unwrap_or(f32::NAN));
        if !position
            .iter()
            .zip(expected.position)
            .all(|(a, b)| a.is_finite() && (a - b).abs() <= 1e-5 + 1e-5 * a.abs().max(b.abs()))
        {
            self.stats.mismatches += 1;
            self.actors.remove(&body);
            return None;
        }
        self.actors.insert(
            body,
            Actor {
                id: generation,
                model,
                clip,
                frame,
                display: candidate,
                seen: Instant::now(),
            },
        );
        self.stats.actors += 1;
        Some(())
    }
    fn object_matrix(&mut self, args: [u32; 4], frame: u64, m: &impl ReadMemory) -> Option<()> {
        let [out, object, _, player] = args;
        // +0x32 selects cached orientation, not reflection. The imported
        // matrix already contains that orientation. Reflection lives in the
        // selected player's two-bit +0x2B4 state and is excluded by this inlet.
        if player > 3
            || args[2] == 0
            || (m.word(object.checked_add(0x2b4)?)? >> (player * 2)) & 3 != 0
        {
            return None;
        }
        self.matrices
            .insert((object, player), (m.matrix(out)?, frame));
        Some(())
    }
    fn views(&mut self, object: u32, frame: u64, m: &impl ReadMemory) -> Option<()> {
        let body = m.word(object.checked_add(4)?)?;
        let actor = self.actors.get(&body)?;
        if actor.model != object || actor.frame != frame {
            return None;
        }
        let info = m.word(body.checked_add(4)?)?;
        let count = m.word(info.checked_add(4)?)? as usize;
        let correction = if m.word(info.checked_add(0xc)?)? & 0x10 != 0 {
            let p = [
                m.float(info + 0x38)?,
                m.float(info + 0x3c)?,
                m.float(info + 0x40)?,
            ];
            ModelCorrection::Translation(p)
        } else {
            ModelCorrection::None
        };
        let camera = if m.byte(object.checked_add(0x31)?)? != 0 {
            None
        } else {
            let context = m.word(0x7b0cc0)?;
            Some(m.matrix(m.word(context.checked_add(0x208)?)?)?)
        };
        let use_palette = m.byte(object.checked_add(0x28)?)? & 8 != 0;
        self.ranges.retain(|r| r.frame == frame && r.body != body);
        for player in 0..4 {
            let Some(&(base, f)) = self.matrices.get(&(object, player)) else {
                continue;
            };
            if f != frame {
                continue;
            }
            let start = m.word(body.checked_add(0x20 + player * 4)?)?;
            if start == 0 {
                continue;
            }
            let view = View {
                base,
                camera,
                correction,
                use_palette,
            };
            let range = Range {
                body,
                player,
                start,
                nodes: count,
                frame,
                view,
            };
            let native = self.placed(range, 0.0)?;
            for (node, want) in native.iter().enumerate() {
                if !close(want, &m.matrix(start.checked_add(node as u32 * 64)?)?) {
                    self.stats.mismatches += 1;
                    return None;
                }
            }
            self.ranges.push(range);
            self.stats.views += 1;
        }
        Some(())
    }
    fn placed(&self, range: Range, alpha: f32) -> Option<Vec<Mat4>> {
        let actor = self.actors.get(&range.body)?;
        if actor.frame != range.frame {
            return None;
        }
        let sampled = actor
            .display
            .sample(&self.clips, Sampling::Fractional, alpha)
            .ok()?;
        let mut object = range.view.base;
        object[12..15].copy_from_slice(&sampled.position);
        let placement = ViewPlacement {
            space: match range.view.camera {
                Some(camera) => Space::World { object, camera },
                None => Space::View(object),
            },
            correction: range.view.correction.matrix().ok()?,
        };
        placement
            .place(if range.view.use_palette {
                Nodes::Skinned(&sampled.matrices)
            } else {
                Nodes::Repeated(std::num::NonZeroUsize::new(sampled.matrices.len())?)
            })
            .ok()
    }
    fn binding(&mut self, source: u32, slot: u32, frame: u64, m: &impl ReadMemory) -> Option<()> {
        self.slots.remove(&slot); // Every write invalidates provenance, including unrelated sources.
        let range = *self.ranges.iter().find(|r| {
            r.frame == frame
                && source >= r.start
                && u64::from(source) < u64::from(r.start) + r.nodes as u64 * 64
                && (source - r.start) % 64 == 0
        })?;
        let native = self.placed(range, 0.0)?;
        let node = ((source - range.start) / 64) as usize;
        if !close(&native[node], &m.matrix(source)?) {
            self.stats.mismatches += 1;
            return None;
        }
        let expression = self.expression(source, frame, m)?;
        let seen = self.actors.get(&range.body)?.seen;
        self.slots.insert(
            slot,
            Slot {
                expression,
                frame,
                seen,
            },
        );
        self.stats.bindings += 1;
        Some(())
    }
    fn expression(&self, address: u32, frame: u64, m: &impl ReadMemory) -> Option<Arc<Expression>> {
        if let Some(scope) = self.draws.last() {
            if scope.frame == frame {
                if let Some(expr) = scope.derived.get(&address) {
                    return Some(expr.clone());
                }
            }
        }
        if let Some(range) = self.ranges.iter().find(|r| {
            r.frame == frame
                && address >= r.start
                && u64::from(address) < u64::from(r.start) + r.nodes as u64 * 64
                && (address - r.start) % 64 == 0
                && self.draws.last().is_none_or(|scope| scope.body == r.body)
        }) {
            let actor = self.actors.get(&range.body)?;
            let mut source = Clips::default();
            source
                .clips
                .0
                .insert(actor.clip, self.clips.clips.0.get(&actor.clip)?.clone());
            source
                .durations
                .insert(actor.clip, *self.clips.durations.get(&actor.clip)?);
            return Some(Arc::new(Expression::Native {
                id: actor.id,
                frame,
                node: ((address - range.start) / 64) as usize,
                view: range.view,
                display: actor.display.clone(),
                source,
            }));
        }
        Some(Arc::new(Expression::Constant(m.matrix(address)?)))
    }
    fn begin_model(&mut self, body: u32, frame: u64) {
        MODEL_SCOPES.fetch_add(1, Ordering::Relaxed);
        let eligible = self.actors.get(&body).is_some_and(|a| a.frame == frame)
            && self
                .ranges
                .iter()
                .any(|r| r.body == body && r.frame == frame);
        ELIGIBLE_SCOPES.fetch_add(u64::from(eligible), Ordering::Relaxed);
        self.draws.push(DrawScope {
            body,
            frame,
            eligible,
            derived: BTreeMap::new(),
            pending: Vec::new(),
        });
    }
    fn wants(&self, va: u32) -> bool {
        matches!(va, 0x5f040 | 0xa14b0 | 0xa1b80 | 0x22520 | 0x18ea90)
            || (matches!(
                va,
                0x1531d0 | 0x153270 | 0x1530e0 | 0x153240 | 0x153440 | 0x151650 | 0x153600
            ) && self.draws.last().is_some_and(|d| d.eligible))
    }
    fn before(&mut self, va: u32, args: [u32; 4], frame: u64, m: &impl ReadMemory) {
        if va == 0x18ea90 {
            self.begin_model(args[0], frame);
            return;
        }
        if !matches!(
            va,
            0x1531d0 | 0x153270 | 0x1530e0 | 0x153240 | 0x153440 | 0x151650 | 0x153600 | 0x22520
        ) {
            return;
        }
        if va == 0x22520 {
            self.slots.remove(&args[1]);
        }
        if !self
            .draws
            .last()
            .is_some_and(|d| d.eligible && d.frame == frame)
        {
            return;
        }
        MATRIX_OPS.fetch_add(1, Ordering::Relaxed);
        // Operands are captured BEFORE the original helper, including aliasing writes.
        let expression = match va {
            0x1531d0 | 0x22520 => {
                self.expression(if va == 0x22520 { args[0] } else { args[1] }, frame, m)
            }
            0x153270 => self
                .expression(args[1], frame, m)
                .zip(self.expression(args[2], frame, m))
                .map(|(a, b)| Arc::new(Expression::Multiply(a, b))),
            0x1530e0 => self
                .expression(args[1], frame, m)
                .map(|a| Arc::new(Expression::Transpose(a))),
            _ => None,
        }
        .filter(|e| e.native() && e.depth() <= 64);
        NATIVE_EXPRESSIONS.fetch_add(u64::from(expression.is_some()), Ordering::Relaxed);
        let destination = if va == 0x22520 { args[1] } else { args[0] };
        let scope = self.draws.last_mut().unwrap();
        if va != 0x22520 {
            scope.derived.remove(&destination);
        }
        scope.pending.push(Pending {
            va,
            destination,
            expression,
            slot: va == 0x22520,
        });
    }
    fn after(&mut self, va: u32, args: [u32; 4], frame: u64, m: &impl ReadMemory) {
        if va == 0x18ea90 {
            self.draws.pop();
            return;
        }
        if matches!(
            va,
            0x1531d0 | 0x153270 | 0x1530e0 | 0x153240 | 0x153440 | 0x151650 | 0x153600 | 0x22520
        ) {
            if self
                .draws
                .last()
                .is_some_and(|d| d.eligible && d.frame == frame)
            {
                let Some(pending) = self.draws.last_mut().unwrap().pending.pop() else {
                    return;
                };
                if pending.va != va {
                    self.draws.last_mut().unwrap().derived.clear();
                    return;
                }
                let Some(expr) = pending.expression else {
                    return;
                };
                let Some(want) = expr.eval(0.0) else {
                    return;
                };
                let actual = if pending.slot {
                    m.matrix(0x563188u32.saturating_add(pending.destination.saturating_mul(64)))
                } else {
                    m.matrix(pending.destination)
                };
                let compare = if pending.slot {
                    std::array::from_fn(|i| want[(i % 4) * 4 + i / 4])
                } else {
                    want
                };
                if !actual.is_some_and(|a| close(&compare, &a)) {
                    self.stats.mismatches += 1;
                    return;
                }
                if pending.slot {
                    let seen = expr
                        .identity()
                        .and_then(|(id, _)| {
                            self.actors.values().find(|a| a.id == id).map(|a| a.seen)
                        })
                        .unwrap_or_else(Instant::now);
                    self.slots.insert(
                        pending.destination,
                        Slot {
                            expression: expr,
                            frame,
                            seen,
                        },
                    );
                    self.stats.bindings += 1;
                } else {
                    self.draws
                        .last_mut()
                        .unwrap()
                        .derived
                        .insert(pending.destination, expr);
                }
                return;
            }
            if va != 0x22520 {
                return;
            }
        }
        self.note(va, args, frame, m);
    }
    /// Direct shader uploads retain the caller's source pointer/register layout.
    /// Some model effects bypass the 22520 dirty-matrix cache entirely.
    fn overlay_source(
        &mut self,
        index: u32,
        source: u32,
        words: &[u32],
        alpha: Option<f32>,
        frame: u64,
    ) -> Option<Vec<u32>> {
        if index >= 192 || words.len() > ((192 - index) as usize) * 4 {
            return None;
        }
        let scope = self.draws.last()?;
        if !scope.eligible || scope.frame != frame {
            return None;
        }
        let seen = self.actors.get(&scope.body)?.seen;
        let fraction =
            alpha.unwrap_or_else(|| (seen.elapsed().as_secs_f32() * 60.0).clamp(0.0, 1.0));
        let mut out = words.to_vec();
        let mut sampled = BTreeMap::<u32, (Mat4, Mat4)>::new();
        let mut applied = false;
        let mut changed = false;
        for (i, word) in out.iter_mut().enumerate() {
            let address = source.checked_add((i as u32).checked_mul(4)?)?;
            let Some((&start, expression)) = scope.derived.range(..=address).next_back() else {
                continue;
            };
            let offset = address.checked_sub(start)?;
            if offset >= 64 || offset % 4 != 0 {
                continue;
            }
            let offset = (offset / 4) as usize;
            if !sampled.contains_key(&start) {
                sampled.insert(start, (expression.eval(0.0)?, expression.eval(fraction)?));
            }
            let (fixed_matrix, fractional_matrix) = sampled.get(&start)?;
            let fixed = fixed_matrix[offset];
            let original = f32::from_bits(words[i]);
            // Qualification applies to the exact source word, including row-only uploads.
            if !fixed.is_finite()
                || !original.is_finite()
                || (fixed - original).abs() > 1e-5 + 1e-5 * fixed.abs().max(original.abs())
            {
                return None;
            }
            let value = fractional_matrix[offset];
            if !value.is_finite() {
                return None;
            }
            changed |= value.to_bits() != *word;
            *word = value.to_bits();
            applied = true;
        }
        if applied {
            self.stats.bindings += 1;
            self.stats.uploads += 1;
            self.stats.changed += u64::from(changed);
            if self.stats.uploads == 1 {
                eprintln!(
                    "native animation: first direct-source fractional shader upload (TEMPORARY-SHIM inlet, removal T2244)"
                );
            }
        }
        applied.then_some(out)
    }
    fn overlay(
        &mut self,
        index: u32,
        words: &[u32],
        alpha: Option<f32>,
        frame: u64,
    ) -> Option<Vec<u32>> {
        if index < 0x3c || (index - 0x3c) % 4 != 0 || words.len() % 16 != 0 {
            return None;
        }
        let first = (index - 0x3c) / 4;
        let mut out = words.to_vec();
        let mut applied = false;
        let mut changed = false;
        for (i, chunk) in out.chunks_exact_mut(16).enumerate() {
            let Some(slot) = self.slots.get(&(first + i as u32)) else {
                continue;
            };
            if slot.frame != frame {
                continue;
            }
            let fraction =
                alpha.unwrap_or_else(|| (slot.seen.elapsed().as_secs_f32() * 60.0).clamp(0.0, 1.0));
            let Some(matrix) = slot.expression.eval(fraction) else {
                continue;
            };
            let sampled: [u32; 16] = std::array::from_fn(|i| matrix[(i % 4) * 4 + i / 4].to_bits());
            changed |= chunk != sampled;
            chunk.copy_from_slice(&sampled);
            applied = true;
        }
        if applied {
            self.stats.uploads += 1;
            self.stats.changed += u64::from(changed);
            if self.stats.uploads == 1 {
                eprintln!(
                    "native animation: first fractional vertex-constant upload (TEMPORARY-SHIM inlet, removal T2244)"
                );
            }
        }
        applied.then_some(out)
    }
}
static TOTAL: std::sync::Mutex<Stats> = std::sync::Mutex::new(Stats {
    observations: 0,
    actors: 0,
    views: 0,
    bindings: 0,
    uploads: 0,
    changed: 0,
    rejected: 0,
    mismatches: 0,
});
fn record_delta(before: Stats, after: Stats) {
    let mut total = TOTAL.lock().unwrap_or_else(|p| p.into_inner());
    total.observations += after.observations - before.observations;
    total.actors += after.actors - before.actors;
    total.views += after.views - before.views;
    total.bindings += after.bindings - before.bindings;
    total.uploads += after.uploads - before.uploads;
    total.changed += after.changed - before.changed;
    total.rejected += after.rejected - before.rejected;
    total.mismatches += after.mismatches - before.mismatches;
}

thread_local! {static BANK:RefCell<Bank>=RefCell::new(Bank::default());}

pub type ReadFn = unsafe extern "C" fn(u32, *mut u8, usize) -> bool;
struct Reader(ReadFn);
impl ReadMemory for Reader {
    fn bytes(&self, address: u32, len: usize) -> Option<Vec<u8>> {
        if address < 0x10000
            || len > 2 * 1024 * 1024
            || u64::from(address) + len as u64 > u64::from(u32::MAX) + 1
        {
            return None;
        }
        let mut bytes = vec![0; len];
        // SAFETY: the host validates the guest span and writes only the supplied owned buffer.
        unsafe { (self.0)(address, bytes.as_mut_ptr(), len) }.then_some(bytes)
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rt_animation_enabled() -> u32 {
    u32::from(config().enabled)
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rt_animation_wants_root(va: u32) -> u32 {
    if !config().enabled {
        return 0;
    }
    BANK.with(|b| u32::from(b.borrow().wants(va)))
}
/// # Safety
/// `read` must validate guest spans and fill only the supplied output extent.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_animation_note(
    phase: u32,
    va: u32,
    a0: u32,
    a1: u32,
    a2: u32,
    a3: u32,
    frame: u64,
    read: ReadFn,
) {
    if !config().enabled {
        return;
    }
    let result = std::panic::catch_unwind(|| {
        BANK.with(|b| {
            let mut bank = b.borrow_mut();
            let before = bank.stats;
            if phase == 0 {
                bank.before(va, [a0, a1, a2, a3], frame, &Reader(read));
            } else {
                bank.after(va, [a0, a1, a2, a3], frame, &Reader(read));
            }
            record_delta(before, bank.stats);
        })
    });
    if result.is_err() {
        eprintln!("native animation: compatibility inlet panic; snapshot dropped");
    }
}
/// # Safety
/// `words` and `output` hold `count` readable/writable u32 values respectively.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_animation_constants(
    index: u32,
    words: *const u32,
    count: u32,
    output: *mut u32,
    frame: u64,
) -> u32 {
    if !config().enabled || count > 768 || words.is_null() || output.is_null() {
        return 0;
    }
    std::panic::catch_unwind(|| {
        BANK.with(|bank| {
            // SAFETY: input extent is guaranteed by the host vertex-constant boundary.
            let original = unsafe { std::slice::from_raw_parts(words, count as usize) };
            let mut b = bank.borrow_mut();
            let before = b.stats;
            let sampled = b.overlay(index, original, config().alpha, frame);
            record_delta(before, b.stats);
            let Some(sampled) = sampled else { return 0 };
            unsafe { std::ptr::copy_nonoverlapping(sampled.as_ptr(), output, sampled.len()) };
            1
        })
    })
    .unwrap_or(0)
}
/// # Safety
/// `words`/`output` contain `count` readable/writable words; `source` is provenance only.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_animation_source_constants(
    index: u32,
    source: u32,
    words: *const u32,
    count: u32,
    output: *mut u32,
    frame: u64,
) -> u32 {
    if !config().enabled || count > 768 || words.is_null() || output.is_null() {
        return 0;
    }
    std::panic::catch_unwind(|| {
        BANK.with(|bank| {
            let original = unsafe { std::slice::from_raw_parts(words, count as usize) };
            let mut b = bank.borrow_mut();
            let before = b.stats;
            let sampled = b.overlay_source(index, source, original, config().alpha, frame);
            record_delta(before, b.stats);
            let Some(sampled) = sampled else { return 0 };
            unsafe { std::ptr::copy_nonoverlapping(sampled.as_ptr(), output, sampled.len()) };
            1
        })
    })
    .unwrap_or(0)
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rt_animation_report() {
    if !config().enabled {
        return;
    }
    let s = TOTAL.lock().unwrap_or_else(|p| p.into_inner());
    eprintln!(
        "native animation scopes: model={} eligible={} matrix_ops={} native_expr={}",
        MODEL_SCOPES.load(Ordering::Relaxed),
        ELIGIBLE_SCOPES.load(Ordering::Relaxed),
        MATRIX_OPS.load(Ordering::Relaxed),
        NATIVE_EXPRESSIONS.load(Ordering::Relaxed)
    );
    eprintln!(
        "native animation: observations={} actors={} views={} bindings={} uploads={} changed={} rejected={} mismatches={} (read-only TEMPORARY-SHIM, removal T2244)",
        s.observations,
        s.actors,
        s.views,
        s.bindings,
        s.uploads,
        s.changed,
        s.rejected,
        s.mismatches
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Memory(BTreeMap<u32, u8>);
    impl Memory {
        fn put(&mut self, at: u32, b: &[u8]) {
            for (i, v) in b.iter().enumerate() {
                self.0.insert(at + i as u32, *v);
            }
        }
        fn put_word(&mut self, at: u32, v: u32) {
            self.put(at, &v.to_le_bytes());
        }
        fn put_float(&mut self, at: u32, v: f32) {
            self.put_word(at, v.to_bits());
        }
        fn put_matrix(&mut self, at: u32, m: Mat4) {
            for (i, v) in m.iter().enumerate() {
                self.put_float(at + i as u32 * 4, *v);
            }
        }
    }
    impl ReadMemory for Memory {
        fn bytes(&self, address: u32, len: usize) -> Option<Vec<u8>> {
            (0..len)
                .map(|i| self.0.get(&address.checked_add(i as u32)?).copied())
                .collect()
        }
    }
    const OBJECT: u32 = 0x10000;
    const BODY: u32 = 0x20000;
    const INFO: u32 = 0x30000;
    const CLIP: u32 = 0x40000;
    const KEYS: u32 = 0x50000;
    const NODE: u32 = 0x50200;
    const TRACK: u32 = 0x50300;
    const POSE: u32 = 0x60000;
    const WORLD: u32 = 0x70000;
    const VIEW: u32 = 0x71000;
    fn identity() -> Mat4 {
        std::array::from_fn(|i| if i % 5 == 0 { 1.0 } else { 0.0 })
    }
    fn memory() -> Memory {
        let mut m = Memory::default();
        m.put_word(BODY, OBJECT);
        m.put_word(BODY + 4, INFO);
        m.put_word(BODY + 0x10, POSE);
        m.put_word(BODY + 0x20, VIEW);
        for i in 1..4 {
            m.put_word(BODY + 0x20 + 4 * i, 0);
        }
        m.put_word(BODY + 0xd8, 0);
        m.put_float(BODY + 0xdc, 0.0);
        m.put_float(BODY + 0x108, -1.0);
        m.put_word(INFO + 4, 1);
        m.put_word(INFO + 0xc, 0);
        m.put_word(OBJECT + 4, BODY);
        m.put_word(OBJECT + 0x28, 8);
        m.put_word(OBJECT + 0x30, 0x10100);
        m.put_word(OBJECT + 0x2b4, 0);
        for i in 0..3 {
            m.put_float(OBJECT + 0x64 + 4 * i, 0.0);
        }
        m.put_float(0x4d1994, 60.0);
        m.put_float(0x4e794c, -1.0);
        m.put_word(0x4e8cec, 60);
        m.put_word(0x7e3954, 0x80000);
        m.put_word(0x80000, CLIP);
        let mut header = [0u32; 14];
        header[3] = 2;
        header[4] = 2;
        header[5] = 1;
        header[6] = (1.0f32 / 60.0).to_bits();
        header[7] = KEYS;
        header[10] = NODE;
        for (i, v) in header.into_iter().enumerate() {
            m.put_word(CLIP + i as u32 * 4, v);
        }
        m.put_word(KEYS, 0);
        m.put_word(KEYS + 4, 1);
        let mut node = [0u32; 8];
        node[1] = 8;
        node[3] = 2;
        node[4] = KEYS;
        node[5] = TRACK;
        for (i, v) in node.into_iter().enumerate() {
            m.put_word(NODE + i as u32 * 4, v);
        }
        for (key, pos) in [0.0f32, 120.0].into_iter().enumerate() {
            let sample = [0.0, pos, 0.0, 0.0, 0.0, 0.0, 1.0];
            for (i, v) in sample.into_iter().enumerate() {
                m.put_float(TRACK + key as u32 * 28 + i as u32 * 4, v);
            }
        }
        for addr in [POSE, WORLD, VIEW] {
            m.put_matrix(addr, identity());
        }
        m
    }
    fn bank(m: &Memory) -> Bank {
        let mut b = Bank::default();
        b.note(0x5f040, [BODY, 0, 0, 0], 10, m);
        b.note(0xa14b0, [WORLD, OBJECT, 1, 0], 10, m);
        b.note(0xa1b80, [OBJECT, 0, 0, 0], 10, m);
        b.note(0x22520, [VIEW, 0, 0, 0], 10, m);
        assert_eq!((b.stats.actors, b.stats.views, b.stats.bindings), (1, 1, 1));
        b
    }
    #[test]
    fn read_only_inlet_binds_explicit_source_and_only_changes_display_words() {
        let m = memory();
        let before = m.0.clone();
        let mut b = bank(&m);
        let words = identity().map(f32::to_bits);
        assert_eq!(b.overlay(0x3c, &words, Some(0.0), 10).unwrap(), words);
        let fraction = b.overlay(0x3c, &words, Some(0.5), 10).unwrap();
        assert_eq!(f32::from_bits(fraction[7]), 60.0);
        assert_eq!(b.stats.changed, 1);
        assert_eq!(m.0, before, "presentation must have no guest writes");
    }
    #[test]
    fn every_unrelated_slot_write_invalidates_animation_provenance() {
        let m = memory();
        let mut b = bank(&m);
        b.note(0x22520, [POSE, 0, 0, 0], 10, &m);
        assert!(
            b.overlay(0x3c, &identity().map(f32::to_bits), Some(0.5), 10)
                .is_none()
        );
    }
    #[test]
    fn stale_frame_or_failed_capture_cannot_reuse_an_old_pose() {
        let mut m = memory();
        let mut b = bank(&m);
        assert!(
            b.overlay(0x3c, &identity().map(f32::to_bits), Some(0.5), 11)
                .is_none()
        );
        m.put_float(0x4d1994, 0.0);
        b.note(0x5f040, [BODY, 0, 0, 0], 11, &m);
        assert!(b.actors.is_empty() && b.ranges.is_empty() && b.slots.is_empty());
        assert!(
            b.overlay(0x3c, &identity().map(f32::to_bits), Some(0.5), 10)
                .is_none()
        );
    }
    #[test]
    fn live_key_time_domain_is_normalized_without_changing_simulation() {
        let mut m = memory();
        m.put_float(0x4d1994, 120.0);
        let before = m.0.clone();
        let mut b = bank(&m);
        let fraction = b
            .overlay(0x3c, &identity().map(f32::to_bits), Some(0.5), 10)
            .unwrap();
        assert_eq!(f32::from_bits(fraction[7]), 120.0);
        assert_eq!(m.0, before);
    }
    #[test]
    fn fixed_pose_mismatch_is_not_admitted_as_a_native_actor() {
        let mut m = memory();
        let mut wrong = identity();
        wrong[0] = 2.0;
        m.put_matrix(POSE, wrong);
        let mut b = Bank::default();
        b.note(0x5f040, [BODY, 0, 0, 0], 10, &m);
        assert_eq!(b.stats.mismatches, 1);
        assert!(b.actors.is_empty());
    }
    #[test]
    fn copy_and_multiply_preserve_explicit_native_provenance_into_temporary_slots() {
        let mut m = memory();
        let mut b = bank(&m);
        const TMP: u32 = 0x72000;
        const FACTOR: u32 = 0x73000;
        const OUT: u32 = 0x74000;
        let mut scale = identity();
        scale[5] = 2.0;
        m.put_matrix(FACTOR, scale);
        b.before(0x18ea90, [BODY, 0, 0, 0], 10, &m);
        b.before(0x1531d0, [TMP, VIEW, 0, 0], 10, &m);
        m.put_matrix(TMP, identity());
        b.after(0x1531d0, [TMP, VIEW, 0, 0], 10, &m);
        b.before(0x153270, [OUT, TMP, FACTOR, 0], 10, &m);
        m.put_matrix(OUT, scale);
        b.after(0x153270, [OUT, TMP, FACTOR, 0], 10, &m);
        b.before(0x22520, [OUT, 0, 0, 0], 10, &m);
        m.put_matrix(0x563188, scale);
        b.after(0x22520, [OUT, 0, 0, 0], 10, &m);
        b.after(0x18ea90, [BODY, 0, 0, 0], 10, &m);
        let rendered = b
            .overlay(0x3c, &scale.map(f32::to_bits), Some(0.5), 10)
            .unwrap();
        assert_eq!(f32::from_bits(rendered[7]), 120.0);
        assert_eq!(
            b.stats.bindings, 2,
            "one direct and one explicit temporary binding"
        );
    }
    #[test]
    fn copied_native_expression_owns_a_snapshot_not_later_mutable_actor_state() {
        let mut m = memory();
        let mut b = bank(&m);
        const TMP: u32 = 0x72000;
        b.before(0x18ea90, [BODY, 0, 0, 0], 10, &m);
        b.before(0x1531d0, [TMP, VIEW, 0, 0], 10, &m);
        m.put_matrix(TMP, identity());
        b.after(0x1531d0, [TMP, VIEW, 0, 0], 10, &m);
        b.before(0x22520, [TMP, 0, 0, 0], 10, &m);
        m.put_matrix(0x563188, identity());
        b.after(0x22520, [TMP, 0, 0, 0], 10, &m);
        b.after(0x18ea90, [BODY, 0, 0, 0], 10, &m);
        b.actors.clear();
        b.clips = Clips::default(); // already submitted data remains owned
        let rendered = b
            .overlay(0x3c, &identity().map(f32::to_bits), Some(0.5), 10)
            .unwrap();
        assert_eq!(f32::from_bits(rendered[7]), 60.0);
        assert!(
            b.overlay(0x3c, &identity().map(f32::to_bits), Some(0.5), 11)
                .is_none()
        );
    }
    #[test]
    fn direct_shader_upload_uses_actual_register_and_row_source_layout() {
        let mut m = memory();
        let mut b = bank(&m);
        const TMP: u32 = 0x72000;
        b.before(0x18ea90, [BODY, 0, 0, 0], 10, &m);
        b.before(0x1530e0, [TMP, VIEW, 0, 0], 10, &m);
        m.put_matrix(TMP, identity());
        b.after(0x1530e0, [TMP, VIEW, 0, 0], 10, &m);
        let row = identity()[4..8]
            .iter()
            .map(|v| v.to_bits())
            .collect::<Vec<_>>();
        let output = b.overlay_source(13, TMP + 16, &row, Some(0.5), 10).unwrap();
        assert_eq!(f32::from_bits(output[3]), 60.0);
        assert_eq!(b.stats.changed, 1);
        let mut wrong = row.clone();
        wrong[0] = 17.0f32.to_bits();
        assert!(
            b.overlay_source(13, TMP + 16, &wrong, Some(0.5), 10)
                .is_none()
        );
        b.after(0x18ea90, [BODY, 0, 0, 0], 10, &m);
        assert!(
            b.overlay_source(13, TMP + 16, &row, Some(0.5), 10)
                .is_none()
        );
    }
}
