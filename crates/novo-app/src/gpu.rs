//! Choosing the graphics adapter at startup.
//!
//! eframe's default asks wgpu for the "best" adapter and fails with one line
//! when it finds none. This picks one on purpose: a real GPU first, then a
//! virtual one (remote desktop, virtual machines), and last a software
//! renderer (the Windows "Basic Render Driver"), so a PC without a usable
//! GPU driver still starts, slowly, instead of not at all. An adapter that
//! cannot present to the window is never chosen.

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
}

impl Wish {
    pub(crate) fn from_env() -> Wish {
        Wish::parse(
            std::env::var("WGPU_ADAPTER_NAME").ok().as_deref(),
            std::env::var("WGPU_POWER_PREF").ok().as_deref(),
        )
    }

    pub(crate) fn parse(name: Option<&str>, power: Option<&str>) -> Wish {
        Wish {
            name: name
                .map(|n| n.trim().to_lowercase())
                .filter(|n| !n.is_empty()),
            low_power: power.is_some_and(|p| p.trim().eq_ignore_ascii_case("low")),
        }
    }

    fn matches(&self, adapter_name: &str) -> bool {
        self.name
            .as_deref()
            .is_some_and(|n| adapter_name.to_lowercase().contains(n))
    }
}

/// The adapter list in the order they are tried: the named adapters first,
/// then by [`rank`] (integrated before discrete when low power is asked).
pub(crate) fn order(kinds: &[wgpu::DeviceType], names: &[String], wish: &Wish) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..kinds.len()).collect();
    idx.sort_by_key(|&i| {
        let rank = match (wish.low_power, kinds[i]) {
            (true, wgpu::DeviceType::IntegratedGpu) => 0,
            (true, wgpu::DeviceType::DiscreteGpu) => 1,
            (_, kind) => rank(kind) + 1,
        };
        (!wish.matches(&names[i]), rank)
    });
    idx
}

/// Why the chosen adapter won, for the log: a name asked for, low power
/// asked for, or simply the best ranked that can draw to the window.
pub(crate) fn why(name: &str, kind: wgpu::DeviceType, wish: &Wish) -> String {
    if wish.matches(name) {
        "named by WGPU_ADAPTER_NAME".to_owned()
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
    let kinds: Vec<wgpu::DeviceType> = infos.iter().map(|i| i.device_type).collect();
    let names: Vec<String> = infos.iter().map(|i| i.name.clone()).collect();
    let wish = Wish::from_env();
    for i in order(&kinds, &names, &wish) {
        let adapter = &adapters[i];
        if surface.is_some_and(|s| !adapter.is_surface_supported(s)) {
            continue;
        }
        log::info!(
            "Using graphics adapter {} ({})",
            describe(&infos[i]),
            why(&names[i], infos[i].device_type, &wish)
        );
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

/// eframe's wgpu setup with [`select`] as the adapter choice.
pub(crate) fn configuration() -> eframe::egui_wgpu::WgpuConfiguration {
    let mut config = eframe::egui_wgpu::WgpuConfiguration::default();
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(create) = &mut config.wgpu_setup {
        create.native_adapter_selector = Some(std::sync::Arc::new(select));
    }
    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::DeviceType::*;

    fn names(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("Adapter {i}")).collect()
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
                &[Cpu, VirtualGpu, IntegratedGpu, DiscreteGpu, Other],
                &names(5),
                &Wish::default()
            ),
            vec![3, 2, 1, 0, 4]
        );
    }

    #[test]
    fn a_machine_with_only_software_rendering_still_gets_an_adapter_order() {
        assert_eq!(order(&[Cpu], &names(1), &Wish::default()), vec![0]);
        assert!(order(&[], &[], &Wish::default()).is_empty());
    }

    #[test]
    fn equal_kinds_keep_the_driver_order() {
        assert_eq!(
            order(&[IntegratedGpu, IntegratedGpu], &names(2), &Wish::default()),
            vec![0, 1]
        );
    }

    #[test]
    fn a_named_adapter_is_tried_first_whatever_its_kind() {
        let names = vec![
            "NVIDIA RTX".to_owned(),
            "Microsoft Basic Render Driver".to_owned(),
        ];
        let wish = Wish::parse(Some("basic render"), None);
        assert_eq!(order(&[DiscreteGpu, Cpu], &names, &wish), vec![1, 0]);
    }

    #[test]
    fn low_power_prefers_the_integrated_gpu_and_keeps_the_rest_in_order() {
        let wish = Wish::parse(None, Some("LOW"));
        assert_eq!(
            order(&[DiscreteGpu, Cpu, IntegratedGpu], &names(3), &wish),
            vec![2, 0, 1]
        );
    }

    #[test]
    fn empty_or_unknown_values_change_nothing() {
        assert_eq!(Wish::parse(Some("  "), Some("high")), Wish::default());
        assert_eq!(Wish::parse(None, None), Wish::default());
    }
}
