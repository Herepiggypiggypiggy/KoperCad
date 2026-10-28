//! Choosing the graphics adapter at startup.
//!
//! eframe's default asks wgpu for the "best" adapter and fails with one line
//! when it finds none. This picks one on purpose: a real GPU first, then a
//! virtual one (remote desktop, virtual machines), and last a software
//! renderer (the Windows "Basic Render Driver"), so a PC without a usable
//! GPU driver still starts, slowly, instead of not at all. An adapter that
//! cannot present to the window is never chosen.
//!
//! On Windows the same GPU is usually listed twice, as DirectX 12 and as Vulkan.
//! DirectX 12 is tried first: the Vulkan driver of some Intel integrated GPUs
//! crashes the whole process (access violation, no window, no message) while
//! the window support or the device is probed, and a crash cannot be caught.
//! Before each probe the adapter is written to the log and the start marker
//! (see [`crate::startup`]); an adapter that stopped a start is tried last
//! from the next start on.

use eframe::egui_wgpu::wgpu;

/// Lower is better.
pub(crate) fn rank(kind: wgpu::DeviceType) -> u8 {
    match kind {
        wgpu::DeviceType::DiscreteGpu => 0,
        wgpu::DeviceType::IntegratedGpu => 1,
        wgpu::DeviceType::VirtualGpu => 2,
        wgpu::DeviceType::Cpu => 3,
        wgpu::DeviceType::Other => 4,
    }
}

/// One line per adapter, for the log and the start-up message.
pub(crate) fn describe(info: &wgpu::AdapterInfo) -> String {
    format!("{} ({:?}, {:?})", info.name, info.device_type, info.backend)
}

/// What a tester can ask for with the environment variables wgpu documents.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct Wish {
    /// `WGPU_ADAPTER_NAME`: part of an adapter's name, any case. Adapters
    /// that match are tried first.
    pub(crate) name: Option<String>,
    /// `WGPU_POWER_PREF=low`: an integrated GPU before a discrete one.
    pub(crate) low_power: bool,
    /// `--safe-graphics`: the software renderer first. Slow, but it starts
    /// where a broken GPU driver does not.
    pub(crate) software: bool,
    /// Windows: DirectX 12 before Vulkan before OpenGL for the same kind of
    /// adapter. Elsewhere the driver order stands.
    pub(crate) prefer_dx12: bool,
    /// Adapters that stopped an earlier start ([`crate::startup::key`]s):
    /// tried after all others, still tried.
    pub(crate) avoid: Vec<String>,
}

static SAFE_GRAPHICS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Records the `--safe-graphics` flag for this run; call before the window opens.
pub(crate) fn set_safe_graphics(on: bool) {
    SAFE_GRAPHICS.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// Whether this run was started with `--safe-graphics`.
pub(crate) fn safe_graphics() -> bool {
    SAFE_GRAPHICS.load(std::sync::atomic::Ordering::Relaxed)
}

impl Wish {
    pub(crate) fn from_env() -> Wish {
        Wish {
            software: safe_graphics(),
            prefer_dx12: cfg!(windows),
            avoid: crate::startup::avoided(),
            ..Wish::parse(
                std::env::var("WGPU_ADAPTER_NAME").ok().as_deref(),
                std::env::var("WGPU_POWER_PREF").ok().as_deref(),
            )
        }
    }

    pub(crate) fn parse(name: Option<&str>, power: Option<&str>) -> Wish {
        Wish {
            name: name
                .map(|n| n.trim().to_lowercase())
                .filter(|n| !n.is_empty()),
            low_power: power.is_some_and(|p| p.trim().eq_ignore_ascii_case("low")),
            ..Wish::default()
        }
    }

    fn matches(&self, adapter_name: &str) -> bool {
        self.name
            .as_deref()
            .is_some_and(|n| adapter_name.to_lowercase().contains(n))
    }
}

/// What the order needs to know about one adapter.
#[derive(Debug, Clone)]
pub(crate) struct Entry {
    pub(crate) name: String,
    pub(crate) kind: wgpu::DeviceType,
    pub(crate) backend: wgpu::Backend,
}

impl Entry {
    pub(crate) fn of(info: &wgpu::AdapterInfo) -> Entry {
        Entry {
            name: info.name.clone(),
            kind: info.device_type,
            backend: info.backend,
        }
    }

    fn key(&self) -> String {
        crate::startup::key(&self.name, &format!("{:?}", self.backend))
    }
}

/// Lower is better; only used when [`Wish::prefer_dx12`] is set.
fn backend_rank(backend: wgpu::Backend) -> u8 {
    match backend {
        wgpu::Backend::Dx12 => 0,
        wgpu::Backend::Vulkan => 1,
        wgpu::Backend::Gl => 2,
        _ => 3,
    }
}

/// The adapter list in the order they are tried: the named adapters first,
/// then real GPUs before software renderers, within those the adapters that
/// did not stop an earlier start, then by [`rank`] (integrated before discrete
/// when low power is asked), then by backend.
pub(crate) fn order(entries: &[Entry], wish: &Wish) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..entries.len()).collect();
    idx.sort_by_key(|&i| {
        let e = &entries[i];
        let rank = match (wish.low_power, e.kind) {
            _ if wish.software && e.kind == wgpu::DeviceType::Cpu => 0,
            (true, wgpu::DeviceType::IntegratedGpu) => 0,
            (true, wgpu::DeviceType::DiscreteGpu) => 1,
            (_, kind) => rank(kind) + 1,
        };
        let backend = if wish.prefer_dx12 {
            backend_rank(e.backend)
        } else {
            0
        };
        // An avoided GPU goes behind the other GPUs but not behind the software
        // renderer: it may have been a false alarm, and a start that works on
        // the software renderer would never learn that.
        let software = matches!(e.kind, wgpu::DeviceType::Cpu | wgpu::DeviceType::Other)
            && !(wish.software && e.kind == wgpu::DeviceType::Cpu);
        (
            !wish.matches(&e.name),
            software,
            wish.avoid.contains(&e.key()),
            rank,
            backend,
        )
    });
    idx
}

/// Why the chosen adapter won, for the log: a name asked for, low power
/// asked for, or simply the best ranked that can draw to the window.
pub(crate) fn why(name: &str, kind: wgpu::DeviceType, wish: &Wish) -> String {
    if wish.matches(name) {
        "named by WGPU_ADAPTER_NAME".to_owned()
    } else if wish.software && kind == wgpu::DeviceType::Cpu {
        "--safe-graphics asks for the software renderer".to_owned()
    } else if wish.low_power && kind == wgpu::DeviceType::IntegratedGpu {
        "WGPU_POWER_PREF asks for low power".to_owned()
    } else {
        "best ranked adapter that can draw to the window".to_owned()
    }
}

/// The adapter selector handed to eframe.
pub(crate) fn select(
    adapters: &[wgpu::Adapter],
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<wgpu::Adapter, String> {
    let infos: Vec<wgpu::AdapterInfo> = adapters.iter().map(|a| a.get_info()).collect();
    log::info!(
        "Graphics adapters found: {}",
        if infos.is_empty() {
            "none".to_owned()
        } else {
            infos.iter().map(describe).collect::<Vec<_>>().join("; ")
        }
    );
    let entries: Vec<Entry> = infos.iter().map(Entry::of).collect();
    let wish = Wish::from_env();
    for i in order(&entries, &wish) {
        let adapter = &adapters[i];
        let start_adapter = crate::startup::Adapter {
            name: infos[i].name.clone(),
            backend: format!("{:?}", infos[i].backend),
        };
        // The next two calls go into the graphics driver. A driver that
        // crashes the process there leaves no panic and no message, so say
        // what is about to happen first (log and start marker).
        if let Some(s) = surface {
            crate::startup::adapter(&start_adapter, crate::startup::STAGE_CHECK);
            if !adapter.is_surface_supported(s) {
                log::warn!(
                    "Skipping {}: cannot draw to the window",
                    describe(&infos[i])
                );
                continue;
            }
        }
        // The driver can list an adapter and still refuse to open it (old
        // drivers, hybrid laptops); try the next one instead of giving up.
        crate::startup::adapter(&start_adapter, crate::startup::STAGE_OPEN);
        if let Err(e) = pollster::block_on(adapter.request_device(&device_descriptor(adapter))) {
            log::warn!("Skipping {}: cannot open it: {e}", describe(&infos[i]));
            continue;
        }
        log::info!(
            "Using graphics adapter {} ({})",
            describe(&infos[i]),
            why(&entries[i].name, entries[i].kind, &wish)
        );
        crate::startup::adapter(&start_adapter, crate::startup::STAGE_FRAMES);
        return Ok(adapter.clone());
    }
    Err(if infos.is_empty() {
        "No graphics adapter was found. The graphics driver may be missing, or the session (for example a remote desktop or a virtual machine) offers no graphics support.".to_owned()
    } else {
        format!(
            "None of the graphics adapters can draw to the window: {}.",
            infos.iter().map(describe).collect::<Vec<_>>().join("; ")
        )
    })
}

/// eframe's device request, but never asking for more than the adapter has:
/// eframe's own asks for the full default limits and 8192 pixel textures,
/// which an older integrated GPU or driver (typical in laptops) refuses.
pub(crate) fn device_descriptor(adapter: &wgpu::Adapter) -> wgpu::DeviceDescriptor<'static> {
    let base = if adapter.get_info().backend == wgpu::Backend::Gl {
        wgpu::Limits::downlevel_webgl2_defaults()
    } else {
        wgpu::Limits::default()
    };
    let wanted = wgpu::Limits {
        max_texture_dimension_2d: 8192,
        ..base
    };
    wgpu::DeviceDescriptor {
        label: Some("KoperCAD device"),
        required_limits: wanted.or_worse_values_from(&adapter.limits()),
        ..Default::default()
    }
}

/// eframe's wgpu setup with [`select`] as the adapter choice.
pub(crate) fn configuration() -> eframe::egui_wgpu::WgpuConfiguration {
    let mut config = eframe::egui_wgpu::WgpuConfiguration::default();
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(create) = &mut config.wgpu_setup {
        create.native_adapter_selector = Some(std::sync::Arc::new(select));
        create.device_descriptor = std::sync::Arc::new(device_descriptor);
    }
    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::Backend::{Dx12, Gl, Vulkan};
    use wgpu::DeviceType::*;

    fn entry(name: &str, kind: wgpu::DeviceType, backend: wgpu::Backend) -> Entry {
        Entry {
            name: name.to_owned(),
            kind,
            backend,
        }
    }

    /// Adapters of the given kinds, all Vulkan, named "Adapter 0", "Adapter 1", ...
    fn kinds(kinds: &[wgpu::DeviceType]) -> Vec<Entry> {
        kinds
            .iter()
            .enumerate()
            .map(|(i, &k)| entry(&format!("Adapter {i}"), k, Vulkan))
            .collect()
    }

    #[test]
    fn the_log_says_why_an_adapter_was_chosen() {
        let named = Wish::parse(Some("intel"), None);
        assert!(why("Intel UHD", DiscreteGpu, &named).contains("WGPU_ADAPTER_NAME"));
        assert!(why("NVIDIA", DiscreteGpu, &named).contains("best ranked"));
        let low = Wish::parse(None, Some("low"));
        assert!(why("Intel UHD", IntegratedGpu, &low).contains("WGPU_POWER_PREF"));
        assert!(why("Intel UHD", IntegratedGpu, &Wish::default()).contains("best ranked"));
    }

    #[test]
    fn a_real_gpu_comes_before_a_virtual_one_and_software_comes_last() {
        assert_eq!(
            order(
                &kinds(&[Cpu, VirtualGpu, IntegratedGpu, DiscreteGpu, Other]),
                &Wish::default()
            ),
            vec![3, 2, 1, 0, 4]
        );
    }

    #[test]
    fn a_machine_with_only_software_rendering_still_gets_an_adapter_order() {
        assert_eq!(order(&kinds(&[Cpu]), &Wish::default()), vec![0]);
        assert!(order(&[], &Wish::default()).is_empty());
    }

    #[test]
    fn equal_kinds_keep_the_driver_order() {
        assert_eq!(
            order(&kinds(&[IntegratedGpu, IntegratedGpu]), &Wish::default()),
            vec![0, 1]
        );
    }

    #[test]
    fn a_named_adapter_is_tried_first_whatever_its_kind() {
        let entries = vec![
            entry("NVIDIA RTX", DiscreteGpu, Vulkan),
            entry("Microsoft Basic Render Driver", Cpu, Dx12),
        ];
        let wish = Wish::parse(Some("basic render"), None);
        assert_eq!(order(&entries, &wish), vec![1, 0]);
    }

    #[test]
    fn low_power_prefers_the_integrated_gpu_and_keeps_the_rest_in_order() {
        let wish = Wish::parse(None, Some("LOW"));
        assert_eq!(
            order(&kinds(&[DiscreteGpu, Cpu, IntegratedGpu]), &wish),
            vec![2, 0, 1]
        );
    }

    #[test]
    fn safe_graphics_puts_the_software_renderer_first() {
        let wish = Wish {
            software: true,
            ..Wish::default()
        };
        assert_eq!(
            order(&kinds(&[DiscreteGpu, IntegratedGpu, Cpu]), &wish),
            vec![2, 0, 1]
        );
        assert!(why("Basic Render Driver", Cpu, &wish).contains("--safe-graphics"));
    }

    #[test]
    fn empty_or_unknown_values_change_nothing() {
        assert_eq!(Wish::parse(Some("  "), Some("high")), Wish::default());
        assert_eq!(Wish::parse(None, None), Wish::default());
    }

    /// The laptop of the hardware report: Intel UHD listed as Vulkan first,
    /// then DirectX 12, then OpenGL, plus the Microsoft software adapter.
    fn intel_laptop() -> Vec<Entry> {
        vec![
            entry("Intel(R) UHD Graphics", IntegratedGpu, Vulkan),
            entry("Intel(R) UHD Graphics", IntegratedGpu, Dx12),
            entry("Intel(R) UHD Graphics", IntegratedGpu, Gl),
            entry("Microsoft Basic Render Driver", Cpu, Dx12),
        ]
    }

    fn windows() -> Wish {
        Wish {
            prefer_dx12: true,
            ..Wish::default()
        }
    }

    #[test]
    fn on_windows_directx_12_is_tried_before_vulkan_for_the_same_gpu() {
        assert_eq!(order(&intel_laptop(), &windows()), vec![1, 0, 2, 3]);
        // Elsewhere the driver order stands.
        assert_eq!(order(&intel_laptop(), &Wish::default()), vec![0, 1, 2, 3]);
    }

    #[test]
    fn directx_12_does_not_put_the_software_adapter_before_a_real_gpu() {
        // WARP is a DirectX 12 adapter too; the kind still decides first.
        let entries = vec![
            entry("Microsoft Basic Render Driver", Cpu, Dx12),
            entry("NVIDIA Quadro P1000", DiscreteGpu, Vulkan),
        ];
        assert_eq!(order(&entries, &windows()), vec![1, 0]);
    }

    #[test]
    fn a_discrete_gpu_still_beats_an_integrated_one_on_either_backend() {
        let entries = vec![
            entry("Intel UHD", IntegratedGpu, Dx12),
            entry("NVIDIA Quadro P1000", DiscreteGpu, Vulkan),
            entry("NVIDIA Quadro P1000", DiscreteGpu, Dx12),
        ];
        assert_eq!(order(&entries, &windows()), vec![2, 1, 0]);
    }

    #[test]
    fn safe_graphics_on_windows_still_starts_with_the_software_adapter() {
        let wish = Wish {
            software: true,
            ..windows()
        };
        assert_eq!(order(&intel_laptop(), &wish)[0], 3);
    }

    #[test]
    fn an_adapter_that_stopped_a_start_is_tried_last_but_still_tried() {
        let entries = intel_laptop();
        let wish = Wish {
            avoid: vec![entries[1].key()], // DirectX 12 crashed this time
            ..windows()
        };
        assert_eq!(order(&entries, &wish), vec![0, 2, 1, 3]);
        // Case does not matter for the match.
        assert_eq!(
            entries[1].key(),
            crate::startup::key("INTEL(R) uhd graphics", "dx12")
        );
        // A name asked for by the tester still wins over the list.
        let named = Wish {
            name: Some("intel".to_owned()),
            avoid: vec![entries[0].key()],
            ..windows()
        };
        assert_eq!(order(&entries, &named)[0], 1);
    }
}
