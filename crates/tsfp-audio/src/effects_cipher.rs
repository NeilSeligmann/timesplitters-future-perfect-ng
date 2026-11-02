//! Private effects image decoder. This is a byte transform, not DSP execution.

#[derive(Clone, Copy)]
struct Generator {
    low: u32,
    high: u32,
}

impl Generator {
    fn step(&mut self) {
        let feedback = (((self.low << 3) ^ (self.low << 1) ^ self.low) << 28) ^ self.high;
        self.low = (self.low >> 1) | (self.high << 31);
        self.high = (self.high >> 1) | (feedback & 0x8000_0000);
    }

    fn bytes(self) -> [u8; 8] {
        let mut result = [0; 8];
        result[..4].copy_from_slice(&self.low.to_le_bytes());
        result[4..].copy_from_slice(&self.high.to_le_bytes());
        result
    }
}

fn initial_generator() -> Generator {
    let mut generator = Generator {
        low: 0x49de_12ba,
        high: 0x7fa4_9bca,
    };
    for _ in 0..81 {
        generator.step();
    }
    generator
}

/// Stateful byte decoder for callers that need the original serial read/write
/// behavior when source and destination spans overlap.
pub struct Decoder {
    generator: Generator,
    initial: [u8; 8],
    feedback: [u8; 8],
    lane: usize,
}

impl Decoder {
    /// Start decoding with the supplied key.
    pub fn new(key: [u8; 8]) -> Self {
        let mut generator = initial_generator();
        let initial = generator.bytes();
        generator.step();
        Self {
            generator,
            initial,
            feedback: key,
            lane: 0,
        }
    }

    /// Decode one byte and advance the original generator state.
    pub fn decode_byte(&mut self, input: u8) -> u8 {
        let lane = self.lane;
        let folded = input.wrapping_sub(self.feedback[lane].wrapping_mul(self.initial[lane]));
        self.generator.step();
        self.lane = (lane + 1) & 7;
        self.initial[lane] ^ self.generator.low as u8 ^ folded
    }
}

/// Eight-byte key used to decode the private effects image.
pub fn setup_key() -> [u8; 8] {
    initial_generator().bytes()
}

/// Decode one source span. The caller owns span validation and image layout.
/// Separate source and destination slices make the common non-overlapping path explicit.
pub fn decode(key: [u8; 8], source: &[u8], destination: &mut [u8]) -> bool {
    if source.len() != destination.len() {
        return false;
    }
    let mut decoder = Decoder::new(key);
    for (&input, output) in source.iter().zip(destination.iter_mut()) {
        *output = decoder.decode_byte(input);
    }
    true
}

/// In-place variant matching the C decoder when source and destination coincide.
pub fn decode_in_place(key: [u8; 8], bytes: &mut [u8]) {
    let mut decoder = Decoder::new(key);
    for byte in bytes.iter_mut() {
        *byte = decoder.decode_byte(*byte);
    }
}
