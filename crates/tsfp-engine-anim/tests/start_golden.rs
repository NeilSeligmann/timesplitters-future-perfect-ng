//! T1668: `0x5A360` against the ORIGINAL (`tools/t1668_start_gold.py`).
use tsfp_engine_anim::start::{Layer, Request, StartEnv, start_animation};

struct World {
    slot: u32,
    zero_slot: u32,
    kind: String,
    p0: u32,
    scale_seen: std::cell::Cell<u32>,
}

impl StartEnv for World {
    fn slot_of(&self, id: u32) -> u32 {
        if id == 0 { self.zero_slot } else { self.slot }
    }
    fn duration(&self, slot: u32) -> Option<f32> {
        // every slot has a clip except `slot` when the kind is null
        if slot == self.slot && self.kind == "null" {
            return None;
        }
        Some(match self.kind.as_str() {
            "pos" => 2.5,
            "neg" => -1.5,
            _ => 0.0,
        })
    }
    fn policy0(&self, _: u32) -> u32 {
        self.p0
    }
    fn root_at_start(&self, slot: u32, scale: f32) -> [f32; 3] {
        self.scale_seen.set(scale.to_bits());
        // the stub writes f(clip page) = page * 0.5 + i * 0.125 + 1 with page = 0x40 + slot
        let page = if slot == self.slot && self.kind == "null" { 0.0 } else { (0x40 + slot) as f32 };
        [0, 1, 2].map(|i| page * 0.5 + i as f32 * 0.125 + 1.0)
    }
}

fn dwords(text: &str) -> Vec<u32> {
    text.split_whitespace().map(|w| u32::from_str_radix(w, 16).unwrap()).collect()
}

#[test]
fn start_animation_equals_the_original() {
    let text = std::fs::read_to_string(format!("{}/tests/data/t1668/start.golden", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let (mut total, mut skipped, mut immediate) = (0, 0, 0);
    for line in text.lines() {
        let (input, output) = line.split_once(" : ").unwrap();
        let parts: Vec<&str> = input.split(" | ").collect();
        let head: Vec<&str> = parts[0].split(' ').collect();
        let (id, blend, speed, scale) = (
            head[1].parse::<u32>().unwrap(),
            f32::from_bits(u32::from_str_radix(head[2], 16).unwrap()),
            f32::from_bits(u32::from_str_radix(head[3], 16).unwrap()),
            f32::from_bits(u32::from_str_radix(head[4], 16).unwrap()),
        );
        let world = World {
            slot: head[5].parse().unwrap(),
            kind: head[6].to_string(),
            p0: u32::from_str_radix(head[7], 16).unwrap(),
            zero_slot: head[9].parse().unwrap(),
            scale_seen: Default::default(),
        };
        let mut current = Layer(dwords(parts[1]).try_into().unwrap());
        let mut previous = Layer(dwords(parts[2]).try_into().unwrap());
        let mut request = Request(dwords(parts[3]).try_into().unwrap());
        let applied = start_animation(&world, id, blend, speed, scale, &mut request, &mut current, &mut previous);
        let outs: Vec<&str> = output.split(" | ").collect();
        let want_current = dwords(outs[0]);
        let want_previous = dwords(outs[1]);
        let req_and_calls: Vec<&str> = outs[2].split_whitespace().collect();
        let want_request: Vec<u32> = req_and_calls[..6].iter().map(|w| u32::from_str_radix(w, 16).unwrap()).collect();
        assert_eq!(current.0.to_vec(), want_current, "{line}");
        assert_eq!(previous.0.to_vec(), want_previous, "{line}");
        assert_eq!(request.0.to_vec(), want_request, "{line}");
        total += 1;
        skipped += usize::from(!applied);
        immediate += usize::from(blend == 0.0);
        if applied {
            assert_eq!(world.scale_seen.get(), scale.to_bits());
        }
    }
    assert!(total == 1500 && skipped > 20 && immediate > 300, "{total} {skipped} {immediate}");
}
