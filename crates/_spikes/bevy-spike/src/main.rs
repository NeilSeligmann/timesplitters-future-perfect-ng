//! T1984 Bevy spike. Args: --fb WxH --frames N --size WxH --no-vsync
use bevy::asset::RenderAssetUsages;
use bevy::audio::AudioPlayer;
use bevy::input::gamepad::GamepadEvent;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_resource::{AsBindGroup, Extent3d, ShaderRef, TextureDimension, TextureFormat};
use bevy::sprite::{AlphaMode2d, Material2d, Material2dPlugin};
use bevy::window::PresentMode;
use spike_core::{Core, InputState};
use std::time::{Duration, Instant};

#[derive(Resource)]
struct Cfg { fb_w: usize, fb_h: usize, frames: u32, size: (u32, u32), vsync: bool, headless: bool, noupd: bool }
#[derive(Resource)]
struct CoreRes { core: Core, input: InputState, started: Instant }
#[derive(Resource)]
struct FbHandle(Handle<Image>);
#[derive(Resource, Default)]
struct Stats {
    frames: u32, upload: Duration, fill: Duration, first_update: Option<Duration>,
    frame_times: Vec<f32>, cpu0: f64, wall0: Option<Instant>, key_events: u32, pad_events: u32,
}
#[derive(Component)]
struct Spinner;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct FixedFn {
    #[texture(0)] #[sampler(1)] texture: Handle<Image>,
    #[uniform(2)] modulate: LinearRgba,
}
impl Material2d for FixedFn {
    fn vertex_shader() -> ShaderRef { "fixedfn.wgsl".into() }
    fn fragment_shader() -> ShaderRef { "fixedfn.wgsl".into() }
    fn alpha_mode(&self) -> AlphaMode2d { AlphaMode2d::Blend }
}

fn cpu_seconds() -> f64 {
    let s = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let rest = s.rsplit(')').next().unwrap_or("");
    let f: Vec<&str> = rest.split_whitespace().collect();
    let t = |i: usize| f.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    (t(11) + t(12)) / 100.0
}

fn parse_wh(s: &str) -> (u32, u32) {
    let (a, b) = s.split_once('x').expect("WxH");
    (a.parse().unwrap(), b.parse().unwrap())
}

fn main() {
    let t0 = Instant::now();
    let mut cfg = Cfg { fb_w: 640, fb_h: 480, frames: 300, size: (1280, 720), vsync: false, headless: false, noupd: false };
    let a: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < a.len() {
        match a[i].as_str() {
            "--fb" => { let w = parse_wh(&a[i + 1]); cfg.fb_w = w.0 as usize; cfg.fb_h = w.1 as usize; i += 1 }
            "--size" => { cfg.size = parse_wh(&a[i + 1]); i += 1 }
            "--frames" => { cfg.frames = a[i + 1].parse().unwrap(); i += 1 }
            "--vsync" => cfg.vsync = true,
            "--headless" => cfg.headless = true,
            "--no-update" => cfg.noupd = true,
            _ => { eprintln!("usage: bevy-spike [--fb WxH] [--size WxH] [--frames N] [--vsync] [--headless]"); return }
        }
        i += 1;
    }
    let present = if cfg.vsync { PresentMode::AutoVsync } else { PresentMode::AutoNoVsync };
    let size = cfg.size;
    let headless = cfg.headless;
    let mut app = App::new();
    app.add_plugins((DefaultPlugins_(present, size, headless), Material2dPlugin::<FixedFn>::default()));
    if headless {
        app.add_plugins(bevy::app::ScheduleRunnerPlugin::run_loop(Duration::ZERO));
    }
    app
        .insert_resource(Time::<Fixed>::from_hz(Core::HZ))
        .insert_resource(cfg)
        .insert_resource(CoreRes { core: Core::default(), input: InputState::default(), started: t0 })
        .insert_resource(Stats::default())
        .add_systems(Startup, setup)
        .add_systems(FixedUpdate, tick_core)
        .add_systems(Update, (read_input, upload_fb, spin, finish).chain())
        .run();
}

#[allow(non_snake_case)]
fn DefaultPlugins_(present: PresentMode, size: (u32, u32), headless: bool) -> bevy::app::PluginGroupBuilder {
    // Minimal plugin set (no PBR, no UI, no scene).
    let b = bevy::app::PluginGroupBuilder::start::<bevy::app::PluginGroupBuilder>()
        .add(bevy::app::TaskPoolPlugin::default())
        .add(bevy::diagnostic::FrameCountPlugin)
        .add(bevy::time::TimePlugin)
        .add(bevy::transform::TransformPlugin)
        .add(bevy::input::InputPlugin)
        .add(bevy::log::LogPlugin::default())
        .add(bevy::window::WindowPlugin {
            primary_window: if headless { None } else { Some(Window {
                title: "tsfp bevy spike".into(),
                resolution: bevy::window::WindowResolution::new(size.0 as f32, size.1 as f32),
                present_mode: present,
                ..default()
            }) },
            exit_condition: if headless { bevy::window::ExitCondition::DontExit } else { bevy::window::ExitCondition::OnAllClosed },
            ..default()
        })
        .add(bevy::a11y::AccessibilityPlugin)
        .add(bevy::asset::AssetPlugin { file_path: "assets".into(), ..default() })
        .add(bevy::render::RenderPlugin::default())
        .add(bevy::render::texture::ImagePlugin::default_nearest())
        .add(bevy::render::pipelined_rendering::PipelinedRenderingPlugin)
        .add(bevy::core_pipeline::CorePipelinePlugin)
        .add(bevy::sprite::SpritePlugin::default())
        .add(bevy::audio::AudioPlugin::default())
        .add(bevy::gilrs::GilrsPlugin);
    if headless { b } else { b.add(bevy::winit::WinitPlugin::<bevy::winit::WakeUp>::default()) }
}

fn setup(
    mut commands: Commands, cfg: Res<Cfg>, mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<FixedFn>>,
    mut audio: ResMut<Assets<AudioSource>>,
) {
    if cfg.headless {
        let mut target = Image::new_fill(Extent3d { width: cfg.size.0, height: cfg.size.1, depth_or_array_layers: 1 }, TextureDimension::D2, &[0, 0, 0, 255], TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
        target.texture_descriptor.usage = bevy::render::render_resource::TextureUsages::COPY_SRC | bevy::render::render_resource::TextureUsages::COPY_DST | bevy::render::render_resource::TextureUsages::TEXTURE_BINDING | bevy::render::render_resource::TextureUsages::RENDER_ATTACHMENT;
        let th = images.add(target);
        commands.spawn((Camera2d, Camera { target: bevy::render::camera::RenderTarget::Image(th.clone().into()), ..default() }));
        commands.spawn(bevy::render::gpu_readback::Readback::texture(th)).observe(
            |t: Trigger<bevy::render::gpu_readback::ReadbackComplete>, mut done: Local<bool>| {
                if *done { return; }
                *done = true;
                let d = &t.event().0;
                let nz = d.chunks_exact(4).filter(|p| p[0] != 0 || p[1] != 0 || p[2] != 0).count();
                info!("READBACK bytes={} nonblack_pixels={}", d.len(), nz);
                let _ = std::fs::write(std::env::temp_dir().join("bevy-spike-readback.rgba"), d);
            });
    } else {
        commands.spawn(Camera2d);
    }
    // (d) CPU-provided framebuffer shown as a sprite.
    let fb = Image::new_fill(
        Extent3d { width: cfg.fb_w as u32, height: cfg.fb_h as u32, depth_or_array_layers: 1 },
        TextureDimension::D2, &[0, 0, 0, 255], TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    let h = images.add(fb);
    commands.insert_resource(FbHandle(h.clone()));
    commands.spawn((
        Sprite { image: h, custom_size: Some(Vec2::new(cfg.size.0 as f32, cfg.size.1 as f32)), ..default() },
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
    // (b) built-in test mesh (owner disc data not used) with custom WGSL material.
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[-1.0f32, -1.0, 0.0], [1.0, -1.0, 0.0], [1.0, 1.0, 0.0], [-1.0, 1.0, 0.0]]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0f32, 0.3, 0.3, 0.8], [0.3, 1.0, 0.3, 0.8], [0.3, 0.3, 1.0, 0.8], [1.0, 1.0, 0.3, 0.8]]);
    mesh.insert_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));
    let mut checker = Vec::new();
    for y in 0..8u8 { for x in 0..8u8 { let v = if (x + y) % 2 == 0 { 255 } else { 90 }; checker.extend_from_slice(&[v, v, v, 255]); } }
    let tex = images.add(Image::new(
        Extent3d { width: 8, height: 8, depth_or_array_layers: 1 }, TextureDimension::D2, checker,
        TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default()));
    commands.spawn((
        Mesh2d(meshes.add(mesh)),
        MeshMaterial2d(mats.add(FixedFn { texture: tex, modulate: LinearRgba::WHITE })),
        Transform::from_xyz(0.0, 0.0, 1.0).with_scale(Vec3::splat(150.0)),
        Spinner,
    ));
    // (e) generated 440 Hz tone as in-memory WAV.
    let rate = 44100u32; let n = rate; // 1 s
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF"); wav.extend_from_slice(&(36 + n * 2).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt "); wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes()); wav.extend_from_slice(&(rate * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes()); wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data"); wav.extend_from_slice(&(n * 2).to_le_bytes());
    for k in 0..n { let s = ((k as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 8000.0) as i16; wav.extend_from_slice(&s.to_le_bytes()); }
    commands.spawn(AudioPlayer::new(audio.add(AudioSource { bytes: wav.into() })));
}

// (c) 60 Hz fixed tick calls into the engine-free core.
fn tick_core(mut c: ResMut<CoreRes>) {
    let input = c.input;
    c.core.step(&input);
}

fn read_input(
    keys: Res<ButtonInput<KeyCode>>, mut pads: EventReader<GamepadEvent>,
    mut c: ResMut<CoreRes>, mut s: ResMut<Stats>,
) {
    c.input.left = keys.pressed(KeyCode::ArrowLeft);
    c.input.right = keys.pressed(KeyCode::ArrowRight);
    c.input.fire = keys.pressed(KeyCode::Space);
    s.key_events += keys.get_just_pressed().count() as u32;
    for _ in pads.read() { s.pad_events += 1; }
}

fn spin(c: Res<CoreRes>, mut q: Query<&mut Transform, With<Spinner>>) {
    for mut t in &mut q { t.rotation = Quat::from_rotation_z(c.core.angle); }
}

fn upload_fb(
    cfg: Res<Cfg>, c: Res<CoreRes>, h: Res<FbHandle>, mut images: ResMut<Assets<Image>>,
    mut s: ResMut<Stats>, time: Res<Time<Real>>,
) {
    if s.first_update.is_none() {
        s.first_update = Some(c.started.elapsed());
        s.wall0 = Some(Instant::now());
        s.cpu0 = cpu_seconds();
    }
    s.frame_times.push(time.delta_secs() * 1000.0);
    if cfg.noupd { s.frames += 1; return; }
    let mut buf = vec![0u8; cfg.fb_w * cfg.fb_h * 4];
    let t = Instant::now();
    c.core.render_fb(cfg.fb_w, cfg.fb_h, &mut buf);
    s.fill += t.elapsed();
    let t = Instant::now();
    if let Some(img) = images.get_mut(&h.0) { img.data = Some(buf); }
    s.upload += t.elapsed();
    s.frames += 1;
}

fn finish(cfg: Res<Cfg>, s: Res<Stats>, c: Res<CoreRes>, mut exit: EventWriter<AppExit>) {
    if s.frames < cfg.frames { return; }
    let wall = s.wall0.unwrap().elapsed().as_secs_f64();
    let cpu = cpu_seconds() - s.cpu0;
    let mut ft: Vec<f32> = s.frame_times.iter().skip(10).copied().collect();
    ft.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let med = ft[ft.len() / 2]; let p99 = ft[ft.len() * 99 / 100];
    let n = s.frames as f64;
    println!("RESULT fb={}x{} win={}x{} frames={} startup_to_first_update_ms={:.0} wall_s={:.2} fps={:.1} frame_ms_median={:.2} frame_ms_p99={:.2} cpu_pct={:.0} fill_us_per_frame={:.0} assign_us_per_frame={:.0} core_ticks={} key_events={} pad_events={}",
        cfg.fb_w, cfg.fb_h, cfg.size.0, cfg.size.1, s.frames, s.first_update.unwrap().as_secs_f64() * 1000.0,
        wall, n / wall, med, p99, cpu / wall * 100.0, s.fill.as_secs_f64() * 1e6 / n, s.upload.as_secs_f64() * 1e6 / n,
        c.core.tick, s.key_events, s.pad_events);
    exit.write(AppExit::Success);
}
