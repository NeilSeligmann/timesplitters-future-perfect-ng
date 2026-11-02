// SPDX-License-Identifier: GPL-3.0-or-later
use crate::*;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContractError {
    InvalidInterval,
    InvalidAlpha,
    InvalidNumber(&'static str),
    InvalidIdentity(&'static str),
    DuplicateIdentity(&'static str),
    MissingBinding(&'static str),
    InvalidRectangle,
    InvalidState(&'static str),
}

/// Only DrawList::validate can construct this borrow. It prevents a backend
/// from accidentally treating unchecked producer data as a complete frame.
pub struct ValidatedDrawList<'a>(&'a DrawList);
impl<'a> ValidatedDrawList<'a> {
    pub fn frame(&self) -> &'a DrawList {
        self.0
    }
}

fn finite(values: impl IntoIterator<Item = f32>, field: &'static str) -> Result<(), ContractError> {
    if values.into_iter().all(f32::is_finite) {
        Ok(())
    } else {
        Err(ContractError::InvalidNumber(field))
    }
}
fn matrix(m: &Matrix4, field: &'static str) -> Result<(), ContractError> {
    finite(m.iter().flatten().copied(), field)
}
fn identities(
    ids: impl IntoIterator<Item = Lifetime>,
    kind: &'static str,
) -> Result<(), ContractError> {
    let mut slots = BTreeSet::new();
    for id in ids {
        if id.generation == 0 {
            return Err(ContractError::InvalidIdentity(kind));
        }
        if !slots.insert(id.slot) {
            return Err(ContractError::DuplicateIdentity(kind));
        }
    }
    Ok(())
}
fn rect(rect: Rect, extent: [u32; 2], allow_empty: bool) -> Result<(), ContractError> {
    if extent.contains(&0)
        || (!allow_empty && (rect.width == 0 || rect.height == 0))
        || rect.x.checked_add(rect.width).is_none_or(|x| x > extent[0])
        || rect
            .y
            .checked_add(rect.height)
            .is_none_or(|y| y > extent[1])
    {
        return Err(ContractError::InvalidRectangle);
    }
    Ok(())
}
fn direction(v: [f32; 3]) -> Result<(), ContractError> {
    finite(v, "light.direction")?;
    if v == [0.; 3] {
        return Err(ContractError::InvalidState("light.direction"));
    }
    Ok(())
}
fn light(light: &Light) -> Result<(), ContractError> {
    finite(light.diffuse, "light.diffuse")?;
    finite(light.specular, "light.specular")?;
    match light.kind {
        LightKind::Ambient => (),
        LightKind::Directional { direction: v } => direction(v)?,
        LightKind::Point {
            position,
            range,
            attenuation,
        }
        | LightKind::Spot {
            position,
            range,
            attenuation,
            ..
        } => {
            finite(position, "light.position")?;
            finite(attenuation, "light.attenuation")?;
            if !range.is_finite() || range < 0. {
                return Err(ContractError::InvalidNumber("light.range"));
            }
        }
    }
    if let LightKind::Spot {
        direction: v,
        inner_cos,
        outer_cos,
        falloff,
        ..
    } = light.kind
    {
        direction(v)?;
        if !(-1.0..=1.0).contains(&inner_cos)
            || !(-1.0..=1.0).contains(&outer_cos)
            || inner_cos < outer_cos
            || !falloff.is_finite()
            || falloff < 0.
        {
            return Err(ContractError::InvalidState("light.spot"));
        }
    }
    Ok(())
}
fn material(material: &Material) -> Result<(), ContractError> {
    finite(
        material.parameters.iter().flatten().copied(),
        "material.parameters",
    )?;
    finite([material.raster.depth_bias_slope], "material.depth_bias")?;
    if material.raster.depth_write && material.raster.depth_compare.is_none() {
        return Err(ContractError::InvalidState("material.depth_write"));
    }
    if let Some(blend) = material.raster.blend {
        finite(blend.constant, "material.blend")?;
    }
    let mut slots = BTreeSet::new();
    for texture in &material.textures {
        if !slots.insert(texture.slot) {
            return Err(ContractError::DuplicateIdentity("texture.slot"));
        }
        finite(
            texture.uv_transform.iter().flatten().copied(),
            "texture.uv_transform",
        )?;
        let sampler = texture.sampler;
        finite(sampler.border, "sampler.border")?;
        finite(
            [sampler.lod_min, sampler.lod_max, sampler.lod_bias],
            "sampler.lod",
        )?;
        if sampler.lod_min < 0. || sampler.lod_min > sampler.lod_max {
            return Err(ContractError::InvalidState("sampler.lod"));
        }
    }
    Ok(())
}

impl DrawList {
    /// Validate the entire frame before returning a backend admission token.
    /// Does not resolve resources or certify shader/pixel equivalence. Zero-area
    /// scissors remain explicit no-op draws, not an excuse to reorder commands.
    pub fn validate(&self) -> Result<ValidatedDrawList<'_>, ContractError> {
        self.sample.validate()?;
        identities(self.cameras.iter().map(|x| x.id.0), "camera")?;
        identities(self.models.iter().map(|x| x.id.0), "instance")?;
        identities(self.materials.iter().map(|x| x.id.0), "material")?;
        identities(self.lights.iter().map(|x| x.id.0), "light")?;
        identities(self.effects.iter().map(|x| x.id.0), "effect")?;
        identities(self.targets.iter().map(|x| x.id.0), "target")?;
        let targets: std::collections::BTreeMap<_, _> =
            self.targets.iter().map(|x| (x.id, x)).collect();
        for target in &self.targets {
            if target.extent.contains(&0) {
                return Err(ContractError::InvalidRectangle);
            }
        }
        let cameras: std::collections::BTreeMap<_, _> =
            self.cameras.iter().map(|x| (x.id, x)).collect();
        let models: std::collections::BTreeMap<_, _> =
            self.models.iter().map(|x| (x.id, x)).collect();
        let materials: std::collections::BTreeMap<_, _> =
            self.materials.iter().map(|x| (x.id, x)).collect();
        let lights: BTreeSet<_> = self.lights.iter().map(|x| x.id).collect();
        let effects: std::collections::BTreeMap<_, _> =
            self.effects.iter().map(|x| (x.id, x)).collect();
        for camera in &self.cameras {
            matrix(&camera.view, "camera.view")?;
            matrix(&camera.projection, "camera.projection")?;
            finite(camera.position, "camera.position")?;
            if !camera
                .depth_range
                .into_iter()
                .all(|z| (0.0..=1.0).contains(&z))
                || camera.depth_range[0] > camera.depth_range[1]
            {
                return Err(ContractError::InvalidState("camera.depth_range"));
            }
            rect(camera.viewport, [u32::MAX; 2], false)?;
        }
        for item in &self.materials {
            material(item)?;
            for binding in &item.textures {
                if let TextureSource::RenderTarget(id) = binding.texture {
                    if !targets.contains_key(&id) {
                        return Err(ContractError::MissingBinding("texture.target"));
                    }
                }
            }
        }
        for item in &self.lights {
            light(item)?;
        }
        for model in &self.models {
            if !materials.contains_key(&model.material) {
                return Err(ContractError::MissingBinding("instance.material"));
            }
            matrix(&model.transform, "instance.transform")?;
            for joint in &model.skin_palette {
                matrix(joint, "instance.skin_palette")?;
            }
        }
        for effect in &self.effects {
            if !materials.contains_key(&effect.material) {
                return Err(ContractError::MissingBinding("effect.material"));
            }
            identities(effect.particles.iter().map(|x| x.id.0), "particle")?;
            for particle in &effect.particles {
                matrix(&particle.transform, "particle.transform")?;
                finite(particle.colour, "particle.colour")?;
                finite(particle.uv_rect, "particle.uv_rect")?;
            }
        }
        for pass in &self.passes {
            if let Target::Offscreen(id) = pass.target {
                let target = targets
                    .get(&id)
                    .ok_or(ContractError::MissingBinding("pass.target"))?;
                if pass.extent != target.extent {
                    return Err(ContractError::InvalidRectangle);
                }
            }
            let camera = cameras
                .get(&pass.camera)
                .ok_or(ContractError::MissingBinding("pass.camera"))?;
            rect(camera.viewport, pass.extent, false)?;
            for draw in &pass.draws {
                let (selected_lights, scissor, material_id) = match draw {
                    Draw::Clear { colour, depth, .. } => {
                        if let Some(colour) = colour {
                            finite(*colour, "clear.colour")?;
                        }
                        if depth.is_some_and(|z| !(0.0..=1.0).contains(&z)) {
                            return Err(ContractError::InvalidNumber("clear.depth"));
                        }
                        continue;
                    }
                    Draw::Mesh {
                        instance,
                        lights,
                        scissor,
                    } => {
                        let material = models
                            .get(instance)
                            .ok_or(ContractError::MissingBinding("draw.instance"))?
                            .material;
                        (lights, scissor, material)
                    }
                    Draw::Particles {
                        batch,
                        lights,
                        scissor,
                    } => {
                        let material = effects
                            .get(batch)
                            .ok_or(ContractError::MissingBinding("draw.effect"))?
                            .material;
                        (lights, scissor, material)
                    }
                };
                if let Target::Offscreen(target) = pass.target {
                    let material = materials.get(&material_id).expect("validated material");
                    if material
                        .textures
                        .iter()
                        .any(|x| x.texture == TextureSource::RenderTarget(target))
                    {
                        return Err(ContractError::InvalidState("target.feedback"));
                    }
                }
                rect(*scissor, pass.extent, true)?;
                identities(selected_lights.iter().map(|x| x.0), "draw.light")?;
                if selected_lights.iter().any(|id| !lights.contains(id)) {
                    return Err(ContractError::MissingBinding("draw.light"));
                }
            }
        }
        Ok(ValidatedDrawList(self))
    }
}
