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

/// The adapter list in the order they are tried.
pub(crate) fn order(kinds: &[wgpu::DeviceType]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..kinds.len()).collect();
    idx.sort_by_key(|&i| rank(kinds[i]));
    idx
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
    for i in order(&kinds) {
        let adapter = &adapters[i];
        if surface.is_some_and(|s| !adapter.is_surface_supported(s)) {
            continue;
        }
        log::info!("Using graphics adapter {}", describe(&infos[i]));
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

    #[test]
    fn a_real_gpu_comes_before_a_virtual_one_and_software_comes_last() {
        assert_eq!(
            order(&[Cpu, VirtualGpu, IntegratedGpu, DiscreteGpu, Other]),
            vec![3, 2, 1, 0, 4]
        );
    }

    #[test]
    fn a_machine_with_only_software_rendering_still_gets_an_adapter_order() {
        assert_eq!(order(&[Cpu]), vec![0]);
        assert!(order(&[]).is_empty());
    }

    #[test]
    fn equal_kinds_keep_the_driver_order() {
        assert_eq!(order(&[IntegratedGpu, IntegratedGpu]), vec![0, 1]);
    }
}
