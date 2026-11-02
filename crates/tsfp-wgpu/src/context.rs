// SPDX-License-Identifier: GPL-3.0-or-later
//! Headless wgpu adapter selection. Which adapters work here is a measured fact, recorded in
//! docs/t-wgpu-renderer.md. `TSFP_WGPU_BACKENDS` (`vulkan`, `gl`, `all`) and
//! `TSFP_WGPU_ADAPTER` (a name substring, an index into the listing, or `software`) override the defaults; `WGPU_ADAPTER_NAME`
//! (wgpu's own variable) is honoured as a name substring when `TSFP_WGPU_ADAPTER` is not set.
//!
//! T2136d adapter policy: the default is the BEST REAL adapter ([`Preference::Any`]: discrete GPU first, then integrated, then
//! other hardware such as a GL driver, then virtual, a CPU adapter last; Vulkan before GL at equal rank). The software adapter
//! ([`Preference::Software`], llvmpipe or lavapipe) is only used when asked for explicitly (`--software`, `TSFP_WGPU_ADAPTER=software`,
//! or by the deterministic hash tests), because the golden hashes are pinned to it. The choice is a pure function
//! ([`select_adapter`]) over the adapter listing so it can be tested with a fake list.
use wgpu::{Adapter, Backends, Instance};

/// Which adapter [`GpuContext::new`] prefers.
#[derive(Clone, Debug)]
pub enum Preference {
    /// A CPU (software) adapter first: deterministic, what the golden hashes are pinned to.
    Software,
    /// The best real adapter (discrete, integrated, other hardware, virtual, CPU last), the default of the live paths.
    Any,
    /// The adapter at this index of the listing ([`list_adapters`] order).
    Index(usize),
    /// First adapter whose name contains the text (case insensitive).
    Named(String),
}

/// One enumerated adapter.
#[derive(Clone, Debug)]
pub struct AdapterReport {
    pub name: String,
    pub backend: String,
    pub device_type: String,
    pub driver: String,
}

pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub info: AdapterReport,
}

fn backends_from_env() -> Backends {
    match std::env::var("TSFP_WGPU_BACKENDS").ok().as_deref() {
        Some("vulkan") => Backends::VULKAN,
        Some("gl") => Backends::GL,
        _ => Backends::VULKAN | Backends::GL,
    }
}

fn report(adapter: &Adapter) -> AdapterReport {
    let info = adapter.get_info();
    AdapterReport {
        name: info.name,
        backend: format!("{:?}", info.backend),
        device_type: format!("{:?}", info.device_type),
        driver: format!("{} {}", info.driver, info.driver_info)
            .trim()
            .to_string(),
    }
}

/// All adapters of the selected backends (`TSFP_WGPU_BACKENDS`).
pub fn list_adapters() -> Vec<AdapterReport> {
    let instance = Instance::new(&wgpu::InstanceDescriptor {
        backends: backends_from_env(),
        ..Default::default()
    });
    instance
        .enumerate_adapters(backends_from_env())
        .iter()
        .map(report)
        .collect()
}

/// Rank of a device type (lower is better) for the default policy.
fn type_rank(device_type: &str) -> u32 {
    match device_type {
        "DiscreteGpu" => 0,
        "IntegratedGpu" => 1,
        "Other" => 2,
        "VirtualGpu" => 3,
        _ => 4, // Cpu and anything unknown
    }
}

fn backend_rank(backend: &str) -> u32 {
    match backend {
        "Vulkan" => 0,
        "Gl" => 1,
        _ => 2,
    }
}

/// Index into `reports` of the adapter to open. `requested` is the text of `TSFP_WGPU_ADAPTER` / `WGPU_ADAPTER_NAME` (a name
/// substring, a decimal index, or `software`) and wins over `preference`. Pure: no wgpu object is touched.
pub fn select_adapter(
    reports: &[AdapterReport],
    preference: &Preference,
    requested: Option<&str>,
) -> Result<usize, String> {
    if reports.is_empty() {
        return Err("no wgpu adapter found".to_string());
    }
    let listing = || -> String {
        reports
            .iter()
            .enumerate()
            .map(|(index, a)| format!("[{index}] {} ({}, {})", a.name, a.backend, a.device_type))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let by_name = |text: &str| -> Result<usize, String> {
        let wanted = text.to_lowercase();
        reports
            .iter()
            .position(|a| a.name.to_lowercase().contains(&wanted))
            .ok_or_else(|| format!("no adapter named like {text:?} among {}", listing()))
    };
    let software = || -> Result<usize, String> {
        reports
            .iter()
            .position(|a| a.device_type == "Cpu")
            .ok_or_else(|| format!("no software adapter among {}", listing()))
    };
    let preference = match requested.map(str::trim).filter(|text| !text.is_empty()) {
        Some(text) if text.eq_ignore_ascii_case("software") => return software(),
        Some(text) => match text.parse::<usize>() {
            Ok(index) if index < reports.len() => return Ok(index),
            Ok(index) => {
                return Err(format!(
                    "adapter index {index} out of range, adapters: {}",
                    listing()
                ));
            }
            Err(_) => return by_name(text),
        },
        None => preference,
    };
    match preference {
        Preference::Software => software(),
        Preference::Index(index) => (*index < reports.len()).then_some(*index).ok_or_else(|| {
            format!(
                "adapter index {index} out of range, adapters: {}",
                listing()
            )
        }),
        Preference::Named(text) => by_name(text),
        Preference::Any => Ok((0..reports.len())
            .min_by_key(|&index| {
                let a = &reports[index];
                (type_rank(&a.device_type), backend_rank(&a.backend), index)
            })
            .expect("non empty")),
    }
}

impl GpuContext {
    /// One line naming what runs: `name=... backend=... type=... driver=... max_texture_2d=... max_buffer=... max_bind_groups=...`.
    pub fn describe(&self) -> String {
        let limits = self.device.limits();
        format!(
            "name={:?} backend={} type={} driver={:?} max_texture_2d={} max_buffer_mib={} max_bind_groups={} max_storage_buffers_per_stage={}",
            self.info.name,
            self.info.backend,
            self.info.device_type,
            self.info.driver,
            limits.max_texture_dimension_2d,
            limits.max_buffer_size >> 20,
            limits.max_bind_groups,
            limits.max_storage_buffers_per_shader_stage
        )
    }

    /// Wrap a device and queue somebody else created (T2136c): for example the ones Bevy's `RenderDevice` and `RenderQueue` hold,
    /// both on wgpu 24.0.5 as this crate, so the renderer's output texture can be copied into a Bevy image on the GPU.
    pub fn from_parts(device: wgpu::Device, queue: wgpu::Queue, info: AdapterReport) -> Self {
        Self {
            device,
            queue,
            info,
        }
    }

    /// Open the preferred adapter, or `Err` with the list of what was found.
    pub fn new(preference: &Preference) -> Result<Self, String> {
        let backends = backends_from_env();
        let instance = Instance::new(&wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });
        let mut adapters = instance.enumerate_adapters(backends);
        let reports: Vec<AdapterReport> = adapters.iter().map(report).collect();
        let requested = std::env::var("TSFP_WGPU_ADAPTER")
            .or_else(|_| std::env::var("WGPU_ADAPTER_NAME"))
            .ok();
        let chosen = select_adapter(&reports, preference, requested.as_deref())?;
        let adapter = adapters.swap_remove(chosen);
        let info = report(&adapter);
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("tsfp-wgpu"),
                required_features: wgpu::Features::empty(),
                // Slice 8: an Arcade level binds a 4096 wide texture, above the downlevel default of 2048 (the Story frames never did).
                required_limits: wgpu::Limits {
                    max_texture_dimension_2d: adapter.limits().max_texture_dimension_2d.min(8192),
                    ..wgpu::Limits::downlevel_defaults()
                },
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))
        .map_err(|error| format!("request_device on {info:?}: {error}"))?;
        Ok(Self {
            device,
            queue,
            info,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake(name: &str, backend: &str, device_type: &str) -> AdapterReport {
        AdapterReport {
            name: name.to_string(),
            backend: backend.to_string(),
            device_type: device_type.to_string(),
            driver: String::new(),
        }
    }

    /// The listing of the owner's machine (T2136d report): a discrete RADV GPU and a GL driver, no software adapter.
    fn owner() -> Vec<AdapterReport> {
        vec![
            fake(
                "AMD Radeon RX 9070 XT (RADV GFX1201)",
                "Vulkan",
                "DiscreteGpu",
            ),
            fake("AMD radeonsi", "Gl", "Other"),
        ]
    }

    #[test]
    fn the_default_is_the_best_real_adapter() {
        assert_eq!(select_adapter(&owner(), &Preference::Any, None), Ok(0));
        let mixed = vec![
            fake("llvmpipe", "Vulkan", "Cpu"),
            fake("Intel UHD", "Vulkan", "IntegratedGpu"),
            fake("NVIDIA RTX", "Vulkan", "DiscreteGpu"),
        ];
        assert_eq!(select_adapter(&mixed, &Preference::Any, None), Ok(2));
    }

    #[test]
    fn real_hardware_on_gl_beats_a_cpu_adapter_on_vulkan() {
        let list = vec![
            fake("lavapipe", "Vulkan", "Cpu"),
            fake("radeonsi", "Gl", "Other"),
        ];
        assert_eq!(select_adapter(&list, &Preference::Any, None), Ok(1));
        // equal rank: Vulkan before GL
        let tie = vec![
            fake("radeonsi", "Gl", "Other"),
            fake("some vk", "Vulkan", "Other"),
        ];
        assert_eq!(select_adapter(&tie, &Preference::Any, None), Ok(1));
    }

    #[test]
    fn a_cpu_adapter_is_only_picked_when_nothing_else_exists_or_when_asked() {
        let only = vec![fake("llvmpipe", "Vulkan", "Cpu")];
        assert_eq!(select_adapter(&only, &Preference::Any, None), Ok(0));
        let both = vec![
            fake("NVIDIA RTX", "Vulkan", "DiscreteGpu"),
            fake("llvmpipe", "Vulkan", "Cpu"),
        ];
        assert_eq!(select_adapter(&both, &Preference::Software, None), Ok(1));
        assert_eq!(
            select_adapter(&both, &Preference::Any, Some("software")),
            Ok(1)
        );
    }

    #[test]
    fn software_without_a_cpu_adapter_is_an_error_naming_the_listing() {
        let error = select_adapter(&owner(), &Preference::Software, None).unwrap_err();
        assert!(error.contains("no software adapter"), "{error}");
        assert!(error.contains("RADV"), "{error}");
    }

    #[test]
    fn the_override_text_is_a_name_an_index_or_software() {
        let list = vec![
            fake("NVIDIA RTX", "Vulkan", "DiscreteGpu"),
            fake("Intel UHD", "Vulkan", "IntegratedGpu"),
            fake("llvmpipe", "Vulkan", "Cpu"),
        ];
        assert_eq!(
            select_adapter(&list, &Preference::Any, Some("intel")),
            Ok(1)
        );
        assert_eq!(select_adapter(&list, &Preference::Any, Some("2")), Ok(2));
        assert_eq!(
            select_adapter(&list, &Preference::Any, Some("SOFTWARE")),
            Ok(2)
        );
        assert!(select_adapter(&list, &Preference::Any, Some("9")).is_err());
        assert!(select_adapter(&list, &Preference::Any, Some("amd")).is_err());
        // an empty override is no override
        assert_eq!(select_adapter(&list, &Preference::Any, Some(" ")), Ok(0));
        assert_eq!(select_adapter(&list, &Preference::Index(1), None), Ok(1));
        assert_eq!(
            select_adapter(&list, &Preference::Named("RTX".into()), None),
            Ok(0)
        );
    }

    #[test]
    fn no_adapters_is_an_error() {
        assert!(select_adapter(&[], &Preference::Any, None).is_err());
    }
}
