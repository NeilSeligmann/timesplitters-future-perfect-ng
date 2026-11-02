//! T3934: per-root frame phase timer for the title frame 0x3C440 in readable Rust.
//!
//! Replaces `src/host/frame_phase_timer.c` (T2260/T2261). The lifted code has no frame to profile, so time is split at the
//! cooperative safepoint: the return site at `[esp]` before every lifted call names the root phase it belongs to
//! (docs/t-high-framerate-analysis.md section 2.1). A phase runs from its first root call to the first root call of the
//! next phase. The timer reads clocks only at a phase change and writes nothing guest visible.

use std::fmt::Write as _;

/// Frame phases: U update roots, R render roots, P the outer loop (frame wait, flip and the rest).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    U0,
    U1,
    U2,
    U3,
    U4,
    R0,
    R1,
    R2,
    R3,
    P,
}
pub const PHASES: usize = 10;
const ALL: [Phase; PHASES] = [
    Phase::U0,
    Phase::U1,
    Phase::U2,
    Phase::U3,
    Phase::U4,
    Phase::R0,
    Phase::R1,
    Phase::R2,
    Phase::R3,
    Phase::P,
];
impl Phase {
    pub fn name(self) -> &'static str {
        ["U0", "U1", "U2", "U3", "U4", "R0", "R1", "R2", "R3", "P"][self as usize]
    }
}

/// Lowest return site of each phase inside 0x3C440, from the lift of 0x3C440 and 0x3D370 (U0..R3).
const PHASE_FIRST_SITE: [u32; 9] = [
    0x3C46D, 0x3C4B4, 0x3C4DA, 0x3C5BC, 0x3C5F8, 0x3C609, 0x3C70B, 0x3C7FD, 0x3C8BF,
];
const FRAME_FUNC_END: u32 = 0x3C918;
/// The call of 0x3C440 itself: the frame starts (U0).
const OUTER_CALL_SITE: u32 = 0x3D3CF;
const OUTER_LO: u32 = 0x3D370;
const OUTER_HI: u32 = 0x3D418;

/// The phase a return site belongs to, `None` for a site outside the frame roots (a nested call).
pub fn phase_of_site(site: u32) -> Option<Phase> {
    if site == OUTER_CALL_SITE {
        return Some(Phase::U0);
    }
    if site > OUTER_LO && site <= OUTER_HI {
        return Some(Phase::P);
    }
    if !(PHASE_FIRST_SITE[0]..FRAME_FUNC_END).contains(&site) {
        return None;
    }
    let index = PHASE_FIRST_SITE
        .iter()
        .rposition(|&first| site >= first)
        .unwrap_or(0);
    Some(ALL[index])
}

#[derive(Clone, Copy, Default)]
struct Frame {
    wall_ns: [u64; PHASES],
    cpu_ns: [u64; PHASES],
}

/// Per call site totals inside the drill function (T2261).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrillSite {
    pub site: u32,
    pub calls: u64,
    pub wall_ns: u64,
    pub cpu_ns: u64,
    pub wall_max_ns: u64,
}
const DRILL_SITES: usize = 128;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Summary {
    pub frames: u64,
    pub wall_ms_mean: [f64; PHASES],
    pub wall_ms_p50: [f64; PHASES],
    pub wall_ms_p95: [f64; PHASES],
    pub cpu_ms_mean: [f64; PHASES],
    pub update_wall_ms_mean: f64,
    pub render_wall_ms_mean: f64,
    pub present_wall_ms_mean: f64,
    pub update_cpu_ms_mean: f64,
    pub render_cpu_ms_mean: f64,
    pub present_cpu_ms_mean: f64,
}

pub struct FrameTimer {
    current: Option<Phase>,
    phase_wall0: u64,
    phase_cpu0: u64,
    frame: Frame,
    frame_open: bool,
    frames: Vec<Frame>,
    csv: Option<String>,
    drill_lo: u32,
    drill_hi: u32,
    drill: Vec<DrillSite>,
    drill_open: Option<usize>,
    drill_wall0: u64,
    drill_cpu0: u64,
}

impl Default for FrameTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameTimer {
    /// Default drill range is the U4 simulation 0x1CE420..0x1CE62F.
    pub fn new() -> Self {
        Self {
            current: None,
            phase_wall0: 0,
            phase_cpu0: 0,
            frame: Frame::default(),
            frame_open: false,
            frames: Vec::new(),
            csv: None,
            drill_lo: 0x1CE420,
            drill_hi: 0x1CE630,
            drill: Vec::new(),
            drill_open: None,
            drill_wall0: 0,
            drill_cpu0: 0,
        }
    }

    /// Move the drill range to a nested function (TSFP_FPT_DRILL=LO:HI, hex); an empty range is ignored.
    pub fn set_drill_range(&mut self, lo: u32, hi: u32) {
        if lo < hi {
            self.drill_lo = lo;
            self.drill_hi = hi;
        }
    }

    /// Start collecting one CSV row per frame; returns the header row.
    pub fn start_csv(&mut self) -> String {
        let mut header = String::from("frame");
        for phase in ALL {
            let _ = write!(header, ",{}_wall_ns", phase.name());
        }
        for phase in ALL {
            let _ = write!(header, ",{}_cpu_ns", phase.name());
        }
        header.push('\n');
        self.csv = Some(String::new());
        header
    }

    /// Rows collected since the last call (empty when CSV output is off).
    pub fn take_csv_rows(&mut self) -> String {
        self.csv.as_mut().map(std::mem::take).unwrap_or_default()
    }

    fn in_drill(&self, site: u32) -> bool {
        (self.drill_lo..self.drill_hi).contains(&site)
    }

    /// True when a return site changes timing state: a root phase site or a drill site.
    pub fn is_tracked(&self, site: u32) -> bool {
        phase_of_site(site).is_some() || self.in_drill(site)
    }

    fn drill_note(&mut self, site: u32, wall_ns: u64, cpu_ns: u64) {
        if let Some(open) = self.drill_open.take() {
            let wall = wall_ns - self.drill_wall0;
            let entry = &mut self.drill[open];
            entry.calls += 1;
            entry.wall_ns += wall;
            entry.cpu_ns += cpu_ns - self.drill_cpu0;
            entry.wall_max_ns = entry.wall_max_ns.max(wall);
        }
        if !self.in_drill(site) {
            return;
        }
        let index = match self.drill.iter().position(|entry| entry.site == site) {
            Some(index) => index,
            None if self.drill.len() == DRILL_SITES => return,
            None => {
                self.drill.push(DrillSite {
                    site,
                    ..DrillSite::default()
                });
                self.drill.len() - 1
            }
        };
        self.drill_open = Some(index);
        self.drill_wall0 = wall_ns;
        self.drill_cpu0 = cpu_ns;
    }

    fn close_frame(&mut self) {
        if let Some(csv) = self.csv.as_mut() {
            let _ = write!(csv, "{}", self.frames.len());
            for value in self.frame.wall_ns.iter().chain(&self.frame.cpu_ns) {
                let _ = write!(csv, ",{value}");
            }
            csv.push('\n');
        }
        self.frames.push(self.frame);
    }

    /// Note a return site with explicit wall and thread CPU clocks (nanoseconds).
    pub fn note(&mut self, site: u32, wall_ns: u64, cpu_ns: u64) {
        let phase = phase_of_site(site);
        if phase.is_some() || self.in_drill(site) {
            self.drill_note(site, wall_ns, cpu_ns);
        }
        let Some(phase) = phase else { return };
        if Some(phase) == self.current {
            return;
        }
        if let Some(current) = self.current {
            self.frame.wall_ns[current as usize] += wall_ns - self.phase_wall0;
            self.frame.cpu_ns[current as usize] += cpu_ns - self.phase_cpu0;
        }
        if phase == Phase::U0 {
            if self.frame_open {
                self.close_frame();
            }
            self.frame = Frame::default();
            self.frame_open = true;
        }
        self.current = Some(phase);
        self.phase_wall0 = wall_ns;
        self.phase_cpu0 = cpu_ns;
    }

    pub fn drill_sites(&self) -> &[DrillSite] {
        &self.drill
    }

    pub fn summary(&self) -> Summary {
        let mut summary = Summary {
            frames: self.frames.len() as u64,
            ..Summary::default()
        };
        if self.frames.is_empty() {
            return summary;
        }
        let count = self.frames.len();
        for phase in ALL {
            let p = phase as usize;
            let wall: f64 = self.frames.iter().map(|f| f.wall_ns[p] as f64).sum();
            let cpu: f64 = self.frames.iter().map(|f| f.cpu_ns[p] as f64).sum();
            summary.wall_ms_mean[p] = wall / count as f64 / 1e6;
            summary.cpu_ms_mean[p] = cpu / count as f64 / 1e6;
            let mut sorted: Vec<u64> = self.frames.iter().map(|f| f.wall_ns[p]).collect();
            sorted.sort_unstable();
            summary.wall_ms_p50[p] = sorted[count / 2] as f64 / 1e6;
            summary.wall_ms_p95[p] = sorted[count * 95 / 100] as f64 / 1e6;
            let (wall_sum, cpu_sum) = match phase {
                Phase::U0 | Phase::U1 | Phase::U2 | Phase::U3 | Phase::U4 => (
                    &mut summary.update_wall_ms_mean,
                    &mut summary.update_cpu_ms_mean,
                ),
                Phase::R0 | Phase::R1 | Phase::R2 | Phase::R3 => (
                    &mut summary.render_wall_ms_mean,
                    &mut summary.render_cpu_ms_mean,
                ),
                Phase::P => (
                    &mut summary.present_wall_ms_mean,
                    &mut summary.present_cpu_ms_mean,
                ),
            };
            *wall_sum += summary.wall_ms_mean[p];
            *cpu_sum += summary.cpu_ms_mean[p];
        }
        summary
    }

    /// The end-of-run report (per phase mean, p50, p95 wall ms, update and render split, drill table).
    pub fn report(&self) -> String {
        let summary = self.summary();
        let mut out = String::new();
        let _ = writeln!(
            out,
            "frame phases   T2260: {} frames of 0x3C440 (wall ms mean/p50/p95, thread cpu ms mean)",
            summary.frames
        );
        if summary.frames == 0 {
            return out;
        }
        for phase in ALL {
            let p = phase as usize;
            let _ = writeln!(
                out,
                "frame phases   {:<2} {:7.3} {:7.3} {:7.3}  cpu {:7.3}",
                phase.name(),
                summary.wall_ms_mean[p],
                summary.wall_ms_p50[p],
                summary.wall_ms_p95[p],
                summary.cpu_ms_mean[p]
            );
        }
        let frame_wall = summary.update_wall_ms_mean + summary.render_wall_ms_mean;
        let frame_cpu = summary.update_cpu_ms_mean + summary.render_cpu_ms_mean;
        let share = |part: f64, total: f64| {
            if total > 0.0 {
                100.0 * part / total
            } else {
                0.0
            }
        };
        let _ = writeln!(
            out,
            "frame phases   update U0..U4 {:.3} ms wall {:.3} cpu, render R0..R3 {:.3} ms wall {:.3} cpu, outer loop P {:.3} ms wall {:.3} cpu; render share of 0x3C440 {:.1}% wall {:.1}% cpu",
            summary.update_wall_ms_mean,
            summary.update_cpu_ms_mean,
            summary.render_wall_ms_mean,
            summary.render_cpu_ms_mean,
            summary.present_wall_ms_mean,
            summary.present_cpu_ms_mean,
            share(summary.render_wall_ms_mean, frame_wall),
            share(summary.render_cpu_ms_mean, frame_cpu)
        );
        let frames = self.frames.len() as f64;
        let _ = writeln!(
            out,
            "frame drill    T2261: call sites in 0x{:X}..0x{:X} (return site, calls, wall ms per frame, max ms, cpu ms per frame)",
            self.drill_lo, self.drill_hi
        );
        for entry in &self.drill {
            let _ = writeln!(
                out,
                "frame drill    0x{:06X} {:8} {:8.3} {:8.3}  cpu {:8.3}",
                entry.site,
                entry.calls,
                entry.wall_ns as f64 / frames / 1e6,
                entry.wall_max_ns as f64 / 1e6,
                entry.cpu_ns as f64 / frames / 1e6
            );
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn site_map() {
        use Phase::*;
        let cases = [
            (0x3D3CF, Some(U0)),
            (0x3C46D, Some(U0)),
            (0x3C4AD, Some(U0)),
            (0x3C4C8, Some(U1)),
            (0x3C56F, Some(U2)),
            (0x3C5F3, Some(U3)),
            (0x3C5F8, Some(U4)),
            (0x3C6CE, Some(R0)),
            (0x3C70B, Some(R1)),
            (0x3C7E7, Some(R1)),
            (0x3C8B5, Some(R2)),
            (0x3C90C, Some(R3)),
            (0x3D3D4, Some(P)),
            (0x3D3EB, Some(P)),
            (0x3D395, Some(P)),
            (0x3C440, None),
            (0x1CE45A, None),
            (0x3C918, None),
        ];
        for (site, phase) in cases {
            assert_eq!(phase_of_site(site), phase, "{site:#x}");
        }
    }

    #[test]
    fn split_matches_c_reference() {
        let mut timer = FrameTimer::new();
        let steps: [(u32, u64); 19] = [
            (0x3C8BF, 5),
            (0x3D3CF, 10),
            (0x3C46D, 11),
            (0x3C4B4, 20),
            (0x1CE45A, 25),
            (0x3C4DA, 30),
            (0x3C5BC, 40),
            (0x3C5F8, 50),
            (0x3C609, 60),
            (0x3C70B, 70),
            (0x3C7FD, 100),
            (0x3C8BF, 110),
            (0x3D3D4, 120),
            (0x3D3EB, 125),
            (0x3D395, 150),
            (0x3D3CF, 160),
            (0x3C609, 170),
            (0x3D3D4, 200),
            (0x3D3CF, 210),
        ];
        for (site, t) in steps {
            timer.note(site, t * 1_000_000, t * 500_000);
        }
        let s = timer.summary();
        assert_eq!(s.frames, 2);
        let near = |a: f64, b: f64| (a - b).abs() < 1e-9;
        assert!(near(s.wall_ms_mean[Phase::U0 as usize], 10.0));
        assert!(near(s.wall_ms_mean[Phase::U1 as usize], 5.0));
        assert!(near(s.wall_ms_mean[Phase::R0 as usize], 20.0));
        assert!(near(s.wall_ms_mean[Phase::R1 as usize], 15.0));
        assert!(near(s.wall_ms_mean[Phase::P as usize], 25.0));
        assert!(near(s.cpu_ms_mean[Phase::P as usize], 12.5));
        assert!(near(s.update_wall_ms_mean, 30.0));
        assert!(near(s.render_wall_ms_mean, 45.0));
        assert!(near(s.wall_ms_p95[Phase::R1 as usize], 30.0));
        let drill = timer.drill_sites();
        assert_eq!(drill.len(), 1);
        assert_eq!(
            drill[0],
            DrillSite {
                site: 0x1CE45A,
                calls: 1,
                wall_ns: 5_000_000,
                cpu_ns: 2_500_000,
                wall_max_ns: 5_000_000
            }
        );
        assert!(timer.report().contains("frame phases   P"));
    }

    #[test]
    fn csv_has_header_and_one_row_per_closed_frame() {
        let mut timer = FrameTimer::new();
        assert!(timer.start_csv().starts_with("frame,U0_wall_ns"));
        timer.note(0x3D3CF, 1, 1);
        timer.note(0x3D3D4, 5, 5);
        timer.note(0x3D3CF, 9, 9);
        assert_eq!(
            timer.take_csv_rows(),
            "0,4,0,0,0,0,0,0,0,0,4,4,0,0,0,0,0,0,0,0,4\n"
        );
    }
}
