//! Typed-core stub: no engine types. Fixed 60 Hz tick driven by the host.
#[derive(Clone, Copy, Default, Debug)]
pub struct InputState {
    pub left: bool,
    pub right: bool,
    pub fire: bool,
    pub stick_x: f32,
}

#[derive(Default)]
pub struct Core {
    pub tick: u64,
    pub angle: f32,
    pub fire_count: u32,
}

impl Core {
    pub const HZ: f64 = 60.0;
    pub fn step(&mut self, input: &InputState) {
        self.tick += 1;
        let turn = input.stick_x + if input.right { 1.0 } else { 0.0 } - if input.left { 1.0 } else { 0.0 };
        self.angle += (0.02 + 0.03 * turn) % std::f32::consts::TAU;
        if input.fire {
            self.fire_count += 1;
        }
    }
    /// Fill an RGBA8 framebuffer (CPU path of a host frame).
    pub fn render_fb(&self, w: usize, h: usize, out: &mut [u8]) {
        let t = (self.tick % 256) as u8;
        for y in 0..h {
            let row = &mut out[y * w * 4..(y + 1) * w * 4];
            for (x, px) in row.chunks_exact_mut(4).enumerate() {
                px[0] = (x as u8).wrapping_add(t);
                px[1] = y as u8;
                px[2] = t;
                px[3] = 255;
            }
        }
    }
}
