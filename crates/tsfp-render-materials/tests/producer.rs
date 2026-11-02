use tsfp_render_materials::{AssetKey, Assets, Binding, Error, MaterialState, Residency, Source};
use tsfp_scene_model::{
    AddressMode, ContentId, Cull, DrawList, Filter, FrameSample, IDENTITY, InstanceId, Lifetime,
    MaterialId, MeshRef, ModelInstance, RasterState, Sampler, Sampling, ShaderRef, TextureRef,
    TextureSource, Winding,
};

fn fixture() -> (Assets, MaterialState) {
    let mut assets = Assets::default();
    assets.shaders.insert(
        AssetKey(1),
        Residency::Resident(ShaderRef(ContentId([1; 32]))),
    );
    assets.textures.insert(
        AssetKey(2),
        Residency::Resident(TextureRef(ContentId([2; 32]))),
    );
    let sampler = Sampler {
        min: Filter::Linear,
        mag: Filter::Nearest,
        mip: Filter::Linear,
        address: [AddressMode::Repeat; 3],
        border: [0.; 4],
        lod_min: 0.,
        lod_max: 4.,
        lod_bias: 0.,
    };
    let state = MaterialState {
        id: MaterialId(Lifetime {
            slot: 7,
            generation: 1,
        }),
        shader: AssetKey(1),
        textures: vec![Binding {
            slot: 3,
            source: Source::Asset(AssetKey(2)),
            sampler,
            uv_set: 1,
            uv_transform: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        }],
        parameters: vec![[1., 2., 3., 4.]],
        raster: RasterState {
            cull: Cull::Back,
            front_face: Winding::Clockwise,
            blend: None,
            depth_compare: None,
            depth_write: false,
            depth_bias_constant: 0,
            depth_bias_slope: 0.,
            stencil: None,
            colour_write: [true; 4],
        },
    };
    (assets, state)
}

fn xbt_texture() -> tsfp_format_xbt::Texture {
    tsfp_format_xbt::Texture {
        words: [4, 4, 4, 4, 0, 0, 0, 0, 0],
        remainder: vec![0; 92],
        pixels: vec![0x11; 16],
    }
}

#[test]
fn xbt_texture_reference_ignores_runtime_index_and_tracks_format_and_payload() {
    let mut first_texture = xbt_texture();
    first_texture.remainder[..4].copy_from_slice(&7u32.to_le_bytes());
    let mut second_texture = xbt_texture();
    second_texture.remainder[..4].copy_from_slice(&29u32.to_le_bytes());

    let mut first_assets = Assets::default();
    let first = first_assets
        .register_xbt_texture(AssetKey(41), &first_texture)
        .unwrap();
    let mut second_assets = Assets::default();
    let same_content = second_assets
        .register_xbt_texture(AssetKey(99), &second_texture)
        .unwrap();
    assert_eq!(first, same_content);
    assert_eq!(
        first.0.0,
        [
            0x9a, 0xb8, 0x87, 0x31, 0x56, 0x8e, 0xf4, 0xed, 0x9c, 0xa2, 0xd8, 0xe2, 0xc3, 0x42,
            0xb2, 0x79, 0x6c, 0x4c, 0x91, 0xcc, 0x34, 0xb9, 0xae, 0x70, 0x4a, 0x16, 0x12, 0x09,
            0xbf, 0xf7, 0x7a, 0xdd,
        ]
    );

    second_texture.words[5] = 1;
    let changed_format = Assets::default()
        .register_xbt_texture(AssetKey(99), &second_texture)
        .unwrap();
    assert_ne!(first, changed_format);

    second_texture.words[5] = 0;
    second_texture.pixels[0] ^= 1;
    let changed_payload = Assets::default()
        .register_xbt_texture(AssetKey(99), &second_texture)
        .unwrap();
    assert_ne!(first, changed_payload);
}

#[test]
fn xbt_texture_registration_rejects_duplicate_and_invalid_inputs_atomically() {
    let key = AssetKey(41);
    let texture = xbt_texture();
    let mut assets = Assets::default();
    let reference = assets.register_xbt_texture(key, &texture).unwrap();
    assert_eq!(
        assets.register_xbt_texture(key, &texture),
        Err(Error::DuplicateTextureKey(key))
    );
    assert_eq!(
        assets.textures.get(&key),
        Some(&Residency::Resident(reference))
    );

    let mut invalid = xbt_texture();
    invalid.words[0] = 0;
    let invalid_key = AssetKey(42);
    assert_eq!(
        assets.register_xbt_texture(invalid_key, &invalid),
        Err(Error::InvalidTexture(invalid_key))
    );
    assert!(!assets.textures.contains_key(&invalid_key));
}

#[test]
fn resolves_content_and_preserves_snapshot_fields() {
    let (mut assets, state) = fixture();
    let first = assets.produce(std::slice::from_ref(&state)).unwrap();
    assert_eq!(
        first[0].textures[0].texture,
        TextureSource::Asset(TextureRef(ContentId([2; 32])))
    );
    assert_eq!(first[0].textures[0].slot, 3);
    assert_eq!(first[0].textures[0].uv_set, 1);
    assert_eq!(first[0].parameters, state.parameters);
    assets.textures.insert(
        AssetKey(2),
        Residency::Resident(TextureRef(ContentId([3; 32]))),
    );
    let second = assets.produce(&[state]).unwrap();
    assert_ne!(first[0].textures[0].texture, second[0].textures[0].texture);
}

#[test]
fn produced_material_is_admitted_when_referenced_by_a_scene_model() {
    let (mut assets, state) = fixture();
    assets.textures.remove(&AssetKey(2));
    let texture = assets
        .register_xbt_texture(AssetKey(2), &xbt_texture())
        .unwrap();
    let materials = assets.produce(&[state.clone()]).unwrap();
    assert_eq!(
        materials[0].textures[0].texture,
        TextureSource::Asset(texture)
    );
    let material_id = state.id;
    let frame = DrawList {
        sample: FrameSample {
            previous_tick: 12,
            current_tick: 12,
            sampling: Sampling::Original,
        },
        targets: vec![],
        cameras: vec![],
        models: vec![ModelInstance {
            id: InstanceId(Lifetime {
                slot: 8,
                generation: 1,
            }),
            mesh: MeshRef(ContentId([4; 32])),
            material: material_id,
            transform: IDENTITY,
            skin_palette: vec![],
            discontinuity: false,
        }],
        materials,
        lights: vec![],
        effects: vec![],
        passes: vec![],
    };

    let admitted = frame.validate().unwrap();
    assert_eq!(admitted.frame().models[0].material, material_id);
    assert_eq!(admitted.frame().materials[0].id, material_id);
}

#[test]
fn missing_loading_and_unloaded_assets_fail_without_fallback() {
    let (mut assets, state) = fixture();
    assets.textures.remove(&AssetKey(2));
    assert_eq!(
        assets.produce(std::slice::from_ref(&state)),
        Err(Error::MissingTexture(AssetKey(2)))
    );
    for residency in [Residency::Loading, Residency::Unloaded] {
        assets.textures.insert(AssetKey(2), residency);
        assert_eq!(
            assets.produce(std::slice::from_ref(&state)),
            Err(Error::TextureNotResident(AssetKey(2)))
        );
    }
    assets.shaders.insert(AssetKey(1), Residency::Loading);
    assert_eq!(
        assets.produce(&[state]),
        Err(Error::ShaderNotResident(AssetKey(1)))
    );
}

#[test]
fn rejects_duplicate_and_invalid_state() {
    let (assets, mut state) = fixture();
    assert_eq!(
        assets.produce(&[state.clone(), state.clone()]),
        Err(Error::DuplicateMaterial(state.id))
    );
    state.textures.push(state.textures[0].clone());
    assert_eq!(
        assets.produce(&[state.clone()]),
        Err(Error::DuplicateSlot(3))
    );
    state.textures.pop();
    state.textures[0].sampler.lod_max = -1.;
    assert_eq!(assets.produce(&[state]), Err(Error::InvalidValues));
}

fn pak_with_xbt(dir: &std::path::Path, key: u32, bytes: &[u8]) {
    let mut pak = b"P5CK".to_vec();
    pak.extend(((12 + bytes.len()) as u32).to_le_bytes());
    pak.extend(16u32.to_le_bytes());
    pak.extend(bytes);
    for word in [key, 12, bytes.len() as u32, 0] {
        pak.extend(word.to_le_bytes());
    }
    std::fs::write(dir.join("t.pak"), pak).unwrap();
}

#[test]
fn loader_snapshot_resolves_textures_from_rust_owned_pak_state() {
    use tsfp_engine_fileio::assets::{AssetKey as Entry, AssetLoader};
    use tsfp_render_materials::{LoaderError, TextureRequest, snapshot_from_rust_owned_loader};
    let dir = std::env::temp_dir().join(format!("tsfp-mat-loader-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let texture = xbt_texture();
    pak_with_xbt(&dir, 5, &texture.encode().unwrap());
    let mut vfs = tsfp_engine_fileio::vfs::Vfs::new();
    vfs.mount_host_dir("C:", &dir).unwrap();
    let mut loader = AssetLoader::new(vfs);
    let (fixture_assets, state) = fixture();
    let shaders = fixture_assets.shaders;
    let request = |occurrence| TextureRequest {
        key: AssetKey(2),
        entry: Entry { key: 5, occurrence },
    };
    let run = |loader: &mut AssetLoader, requests: &[TextureRequest]| {
        snapshot_from_rust_owned_loader(loader, "C:/t.pak", requests, shaders.clone(), &[state.clone()])
    };
    let materials = run(&mut loader, &[request(0)]).unwrap();
    let mut expected = Assets::default();
    let reference = expected.register_xbt_texture(AssetKey(2), &texture).unwrap();
    assert_eq!(
        materials[0].textures[0].texture,
        TextureSource::Asset(reference)
    );
    assert_eq!(
        run(&mut loader, &[request(1)]),
        Err(LoaderError::Load(AssetKey(2)))
    );
    assert_eq!(
        run(&mut loader, &[]),
        Err(LoaderError::Material(Error::MissingTexture(AssetKey(2))))
    );
    let _ = std::fs::remove_dir_all(&dir);
}
