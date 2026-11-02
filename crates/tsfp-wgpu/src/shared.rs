// SPDX-License-Identifier: GPL-3.0-or-later
//! T2229 latest-wins GPU publication. The caller serializes encode+submit and acquire with one mutex.
#[derive(Default)]
struct Slot {
    texture: Option<Box<wgpu::Texture>>,
    held: bool,
    present: u64,
}

#[derive(Default)]
pub struct SharedRing {
    slots: [Slot; 3],
    newest: Option<usize>,
    cursor: usize,
}

impl SharedRing {
    /// A held Box never moves or changes, including across output resize.
    pub fn acquire(&mut self) -> Option<(&wgpu::Texture, u64, u32)> {
        let index = self.newest.take()?;
        let slot = &mut self.slots[index];
        slot.held = true;
        Some((slot.texture.as_deref()?, slot.present, index as u32))
    }

    pub fn release(&mut self, index: u32) {
        if let Some(slot) = self.slots.get_mut(index as usize) {
            slot.held = false;
        }
    }

    /// Encode only; the caller must submit before unlocking the ring.
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::Texture,
        present: u64,
    ) {
        let Some(index) = (0..3)
            .map(|offset| (self.cursor + offset) % 3)
            .find(|&index| !self.slots[index].held)
        else {
            return;
        };
        let slot = &mut self.slots[index];
        let size = source.size();
        if slot
            .texture
            .as_ref()
            .is_none_or(|texture| texture.size() != size)
        {
            slot.texture = Some(Box::new(device.create_texture(&wgpu::TextureDescriptor {
                label: Some("tsfp-shared-frame"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })));
        }
        encoder.copy_texture_to_texture(
            source.as_image_copy(),
            slot.texture.as_ref().unwrap().as_image_copy(),
            size,
        );
        slot.present = present;
        self.newest = Some(index);
        self.cursor = (index + 1) % 3;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{GpuContext, Preference};

    #[test]
    fn offscreen_ring_order_wrap_resize_and_device_lifetime() {
        let gpu = GpuContext::new(&Preference::Software).expect("software Vulkan adapter");
        let device = gpu.device.clone();
        let queue = gpu.queue.clone();
        drop(gpu); // cloned handles own the device, not the shell's original handles
        let mut ring = SharedRing::default();
        assert!(ring.acquire().is_none());
        let publish = |ring: &mut SharedRing, width, present| {
            let source = device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width,
                    height: 4,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                source.as_image_copy(),
                &vec![present as u8; width as usize * 4 * 4],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(4),
                },
                source.size(),
            );
            let mut encoder = device.create_command_encoder(&Default::default());
            ring.encode(&device, &mut encoder, &source, present);
            queue.submit([encoder.finish()]);
        };
        publish(&mut ring, 4, 1);
        let (texture, present, held) = ring.acquire().unwrap();
        let original = texture as *const wgpu::Texture;
        assert_eq!(present, 1);
        assert!(ring.acquire().is_none());
        for present in 2..12 {
            publish(&mut ring, 8, present);
            let (texture, number, slot) = ring.acquire().unwrap();
            assert_eq!(number, present);
            assert_ne!(slot, held);
            assert_eq!(texture.width(), 8);
            assert_eq!(
                ring.slots[held as usize].texture.as_deref().unwrap() as *const _,
                original
            );
            assert_eq!(
                ring.slots[held as usize].texture.as_ref().unwrap().width(),
                4
            );
            ring.release(slot);
        }
        ring.release(held);
        for present in 12..15 {
            publish(&mut ring, 16, present);
        }
        let (texture, number, _) = ring.acquire().unwrap();
        assert_eq!(number, 14); // latest wins even without acquisitions
        assert_eq!(texture.width(), 16);
        // All three acquired: no free slot, producer must leave held contents unchanged.
        let mut held_slots = Vec::new();
        for number in 20..22 {
            publish(&mut ring, 16, number);
            held_slots.push(ring.acquire().unwrap().2);
        }
        publish(&mut ring, 32, 99);
        assert!(ring.acquire().is_none());
        for slot in held_slots {
            ring.release(slot);
        }
        device.poll(wgpu::Maintain::Wait); // test-only completion/validation
    }
}
