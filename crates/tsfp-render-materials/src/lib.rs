// SPDX-License-Identifier: GPL-3.0-or-later
//! Opt-in, unverified material snapshot boundary. The caller owns asset loading
//! and recipe interpretation; this crate never invents a missing resource.
use std::collections::{BTreeMap, BTreeSet};
use tsfp_scene_model::{
    Material, MaterialId, RasterState, RenderTargetId, Sampler, ShaderRef, TextureBinding,
    TextureRef, TextureSource,
};

const XBT_TEXTURE_IDENTITY_V1: &[u8] = b"tsfp-texture-xbt-v1\0";

/// Loader-issued asset key, distinct from a content identity and guest pointers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssetKey(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Residency<T> {
    Loading,
    Resident(T),
    Unloaded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Asset(AssetKey),
    RenderTarget(RenderTargetId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    pub slot: u8,
    pub source: Source,
    pub sampler: Sampler,
    pub uv_set: u8,
    pub uv_transform: [[f32; 3]; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct MaterialState {
    pub id: MaterialId,
    pub shader: AssetKey,
    pub textures: Vec<Binding>,
    pub parameters: Vec<[f32; 4]>,
    pub raster: RasterState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    DuplicateTextureKey(AssetKey),
    InvalidTexture(AssetKey),
    InvalidMaterialId(MaterialId),
    DuplicateMaterial(MaterialId),
    DuplicateSlot(u8),
    MissingShader(AssetKey),
    ShaderNotResident(AssetKey),
    MissingTexture(AssetKey),
    TextureNotResident(AssetKey),
    InvalidTarget(RenderTargetId),
    InvalidValues,
}

/// Immutable per-frame view of loader residency. Resolve all requested materials
/// atomically: an error yields no partial list and no implicit fallback.
#[derive(Default)]
pub struct Assets {
    pub shaders: BTreeMap<AssetKey, Residency<ShaderRef>>,
    pub textures: BTreeMap<AssetKey, Residency<TextureRef>>,
}

impl Assets {
    /// Register a loaded XBT texture under its loader key and return its
    /// canonical scene-model identity. The runtime texture index at XBT offset
    /// `0x24` is not resource content and is zeroed before hashing; dimensions,
    /// format, retained header layout and pixel payload remain part of the ID.
    ///
    /// The map changes only after encoding and hashing succeed. Re-registering
    /// a key is rejected so one frame cannot silently replace its residency.
    pub fn register_xbt_texture(
        &mut self,
        key: AssetKey,
        texture: &tsfp_format_xbt::Texture,
    ) -> Result<TextureRef, Error> {
        if self.textures.contains_key(&key) {
            return Err(Error::DuplicateTextureKey(key));
        }
        let encoded = texture.encode().map_err(|_| Error::InvalidTexture(key))?;
        let mut canonical = Vec::with_capacity(XBT_TEXTURE_IDENTITY_V1.len() + encoded.len());
        canonical.extend_from_slice(XBT_TEXTURE_IDENTITY_V1);
        canonical.extend_from_slice(&encoded);
        let index_start = XBT_TEXTURE_IDENTITY_V1.len() + std::mem::size_of_val(&texture.words);
        let index_end = index_start + std::mem::size_of::<u32>();
        canonical[index_start..index_end].fill(0);
        let reference = TextureRef(tsfp_scene_model::ContentId(tsfp_state::sha256::sha256(
            &canonical,
        )));
        self.textures.insert(key, Residency::Resident(reference));
        Ok(reference)
    }

    pub fn produce(&self, states: &[MaterialState]) -> Result<Vec<Material>, Error> {
        let mut ids = BTreeSet::new();
        states
            .iter()
            .map(|state| {
                if state.id.0.generation == 0 {
                    return Err(Error::InvalidMaterialId(state.id));
                }
                if !ids.insert(state.id) {
                    return Err(Error::DuplicateMaterial(state.id));
                }
                let shader = match self.shaders.get(&state.shader) {
                    None => return Err(Error::MissingShader(state.shader)),
                    Some(Residency::Resident(shader)) => *shader,
                    Some(_) => return Err(Error::ShaderNotResident(state.shader)),
                };
                if !state.parameters.iter().flatten().all(|v| v.is_finite())
                    || !state.raster.depth_bias_slope.is_finite()
                    || (state.raster.depth_write && state.raster.depth_compare.is_none())
                    || state
                        .raster
                        .blend
                        .is_some_and(|b| !b.constant.iter().all(|v| v.is_finite()))
                {
                    return Err(Error::InvalidValues);
                }
                let mut slots = BTreeSet::new();
                let textures = state
                    .textures
                    .iter()
                    .map(|binding| {
                        if !slots.insert(binding.slot) {
                            return Err(Error::DuplicateSlot(binding.slot));
                        }
                        let s = binding.sampler;
                        if !binding.uv_transform.iter().flatten().all(|v| v.is_finite())
                            || !s.border.iter().all(|v| v.is_finite())
                            || ![s.lod_min, s.lod_max, s.lod_bias]
                                .iter()
                                .all(|v| v.is_finite())
                            || s.lod_min < 0.0
                            || s.lod_min > s.lod_max
                        {
                            return Err(Error::InvalidValues);
                        }
                        let texture = match binding.source {
                            Source::RenderTarget(id) if id.0.generation == 0 => {
                                return Err(Error::InvalidTarget(id));
                            }
                            Source::RenderTarget(id) => TextureSource::RenderTarget(id),
                            Source::Asset(key) => match self.textures.get(&key) {
                                None => return Err(Error::MissingTexture(key)),
                                Some(Residency::Resident(texture)) => {
                                    TextureSource::Asset(*texture)
                                }
                                Some(_) => return Err(Error::TextureNotResident(key)),
                            },
                        };
                        Ok(TextureBinding {
                            slot: binding.slot,
                            texture,
                            sampler: s,
                            uv_set: binding.uv_set,
                            uv_transform: binding.uv_transform,
                        })
                    })
                    .collect::<Result<_, _>>()?;
                Ok(Material {
                    id: state.id,
                    shader,
                    textures,
                    parameters: state.parameters.clone(),
                    raster: state.raster,
                })
            })
            .collect()
    }
}

/// One texture the frame needs from the Rust-owned loader: its scene-side key
/// and the PAK entry that holds the XBT.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureRequest {
    pub key: AssetKey,
    pub entry: tsfp_engine_fileio::assets::AssetKey,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LoaderError {
    Load(AssetKey),
    Material(Error),
}

/// Build the material snapshot of one frame from Rust-owned loader state:
/// every requested XBT is loaded through `loader` from `pak_path`, registered
/// under its content identity, then `states` resolve against those textures
/// and the caller's resident `shaders`. Any failure yields no snapshot.
pub fn snapshot_from_rust_owned_loader(
    loader: &mut tsfp_engine_fileio::assets::AssetLoader,
    pak_path: &str,
    requests: &[TextureRequest],
    shaders: BTreeMap<AssetKey, Residency<ShaderRef>>,
    states: &[MaterialState],
) -> Result<Vec<Material>, LoaderError> {
    let mut assets = Assets { shaders, textures: BTreeMap::new() };
    for request in requests {
        let texture = loader
            .texture(pak_path, request.entry)
            .map_err(|_| LoaderError::Load(request.key))?;
        assets
            .register_xbt_texture(request.key, &texture)
            .map_err(LoaderError::Material)?;
    }
    assets.produce(states).map_err(LoaderError::Material)
}
