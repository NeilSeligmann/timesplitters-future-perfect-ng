//! Bounded, nonblocking CPU readback. Slots are never reused before map completion.
use crate::replay::Frame;
use std::{collections::VecDeque, sync::mpsc};

#[derive(Default, Clone, Copy, Debug)]
pub struct ReadbackStats {
    pub occupancy: usize,
    pub peak: usize,
    pub submitted: u64,
    pub completed: u64,
    pub late: u64,
    pub saturated: u64,
    pub failed: u64,
}
struct Slot {
    buffer: wgpu::Buffer,
    size: (u32, u32),
    pending: Option<(u64, mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>)>,
}
/// Three persistent staging buffers. Overload drops CPU pictures, never surface operations.
#[derive(Default)]
pub struct ReadbackRing {
    slots: Vec<Slot>,
    order: VecDeque<usize>,
    pub stats: ReadbackStats,
}
impl ReadbackRing {
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
        size: (u32, u32),
    ) -> Option<usize> {
        let index = self
            .slots
            .iter()
            .position(|s| s.pending.is_none())
            .unwrap_or(self.slots.len());
        if index == 3 {
            self.stats.saturated += 1;
            return None;
        }
        let padded = (size.0 * 4).div_ceil(256) * 256;
        if index == self.slots.len() || self.slots[index].size != size {
            let slot = Slot {
                buffer: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("tsfp-pipelined-readback"),
                    size: u64::from(padded) * u64::from(size.1),
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                size,
                pending: None,
            };
            if index == self.slots.len() {
                self.slots.push(slot);
            } else {
                self.slots[index] = slot;
            }
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.slots[index].buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(size.1),
                },
            },
            wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
        );
        Some(index)
    }
    /// Called only after the encoder containing the copy is submitted.
    pub fn map(&mut self, index: usize, present: u64) {
        let (tx, rx) = mpsc::channel();
        self.slots[index]
            .buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        self.slots[index].pending = Some((present, rx));
        self.order.push_back(index);
        self.stats.submitted += 1;
        self.stats.occupancy = self.order.len();
        self.stats.peak = self.stats.peak.max(self.stats.occupancy);
    }
    /// Poll only; callbacks can complete out of order but pictures are consumed in submission order.
    /// A picture more than two presents old is discarded rather than publishing stale output.
    pub fn poll(&mut self, device: &wgpu::Device, now: u64) -> Vec<(u64, Frame)> {
        device.poll(wgpu::Maintain::Poll);
        let mut frames = Vec::new();
        while let Some(&index) = self.order.front() {
            let slot = &mut self.slots[index];
            let (number, receiver) = slot.pending.as_ref().unwrap();
            if *number >= now {
                break;
            }
            let result = match receiver.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.stats.failed += 1;
                    slot.pending = None;
                    self.order.pop_front();
                    continue;
                }
            };
            let number = *number;
            if result.is_err() {
                self.stats.failed += 1;
            } else {
                if now.saturating_sub(number) > 2 {
                    self.stats.late += 1;
                } else {
                    let (width, height) = slot.size;
                    let padded = (width * 4).div_ceil(256) * 256;
                    let mapped = slot.buffer.slice(..).get_mapped_range();
                    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
                    for row in mapped.chunks_exact(padded as usize) {
                        rgba.extend_from_slice(&row[..(width * 4) as usize]);
                    }
                    drop(mapped);
                    frames.push((
                        number,
                        Frame {
                            width,
                            height,
                            rgba,
                        },
                    ));
                    self.stats.completed += 1;
                }
                slot.buffer.unmap();
            }
            slot.pending = None;
            self.order.pop_front();
        }
        self.stats.occupancy = self.order.len();
        frames
    }
}
