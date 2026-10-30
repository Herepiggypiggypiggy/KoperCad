//! The generic host input service (lane C5): reads USB gamepads and joysticks, offline, with no
//! driver and no administrator rights, and hands each one's state to the app by stable axis and
//! button names (`novo_controller::gamepad::InputState`).
//!
//! It is not controller specific: add-ons that ask for input devices get the same states.
//! The reading itself sits behind [`InputBackend`], so tests (and the visual test user,
//! `--control`) inject input with the [`FakeBackend`] and no hardware is needed.
//!
//! Backends:
//! - Windows: `gilrs` (the Windows Gaming Input API), see `input_gilrs.rs`. It is
//!   part of Windows and needs no driver or elevation.
//! - Other systems: none yet. `gilrs` needs the system library `libudev` on Linux, which the CI
//!   image and a user's PC cannot be assumed to have, so it is not linked there; the app says
//!   plainly that gamepad input is for the Windows build. Under `--control` the fake backend
//!   stands in everywhere.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

use novo_controller::gamepad::{AXES, BUTTONS, InputState};

/// One input device as the backend sees it now.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Device {
    /// Stable for the session while the device stays known (`pad0`).
    pub(super) id: String,
    /// The name the system reports.
    pub(super) name: String,
    pub(super) state: InputState,
}

/// Something that can list the devices and their state. Polled once per frame.
pub(super) trait InputBackend {
    /// Short name of the way devices are read, for the card.
    fn label(&self) -> &str;
    /// Every device known, connected or not, in a stable order.
    fn poll(&mut self) -> Vec<Device>;
    /// Why this backend cannot read devices, when it cannot.
    fn unavailable(&self) -> Option<&str> {
        None
    }
}

/// The backend of a system without a gamepad reader.
pub(super) struct NoBackend(pub(super) String);

impl InputBackend for NoBackend {
    fn label(&self) -> &str {
        "no gamepad reader"
    }
    fn poll(&mut self) -> Vec<Device> {
        Vec::new()
    }
    fn unavailable(&self) -> Option<&str> {
        Some(&self.0)
    }
}

/// The state a [`FakeHandle`] edits and a [`FakeBackend`] reports.
#[derive(Debug, Clone)]
struct FakeState {
    name: String,
    present: bool,
    axes: BTreeMap<String, f64>,
    buttons: BTreeMap<String, bool>,
}

impl Default for FakeState {
    fn default() -> Self {
        FakeState {
            name: "Test gamepad (fake)".to_owned(),
            present: true,
            axes: AXES.iter().map(|a| (a.to_string(), 0.0)).collect(),
            buttons: BUTTONS.iter().map(|b| (b.to_string(), false)).collect(),
        }
    }
}

/// The hand that moves a fake pad: tests, and the `pad` command of the control mode.
#[derive(Clone)]
pub(crate) struct FakeHandle(Arc<Mutex<FakeState>>);

impl FakeHandle {
    fn with<R>(&self, f: impl FnOnce(&mut FakeState) -> R) -> R {
        let mut s = self.0.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut s)
    }

    pub(crate) fn set_axis(&self, name: &str, value: f64) {
        self.with(|s| {
            s.axes.insert(name.to_owned(), value);
        });
    }

    pub(crate) fn set_button(&self, name: &str, down: bool) {
        self.with(|s| {
            s.buttons.insert(name.to_owned(), down);
        });
    }

    /// Plug the pad in or pull it out. Sticks and buttons keep where they were, so a pad can be
    /// plugged in with the stick held over (`centre` first for a pad at rest).
    pub(crate) fn connect(&self, present: bool) {
        self.with(|s| s.present = present);
    }

    /// Everything back to rest; the pad stays plugged in.
    pub(crate) fn centre(&self) {
        self.with(|s| {
            for v in s.axes.values_mut() {
                *v = 0.0;
            }
            for b in s.buttons.values_mut() {
                *b = false;
            }
        });
    }
}

/// A scripted pad. Cheap to make; each one is its own device, `pad0`.
pub(crate) struct FakeBackend(FakeHandle);

impl FakeBackend {
    /// A fresh fake pad (plugged in, everything at rest) and the hand that moves it.
    #[cfg(test)]
    pub(crate) fn new() -> (FakeBackend, FakeHandle) {
        let handle = FakeHandle(Arc::new(Mutex::new(FakeState::default())));
        (FakeBackend(handle.clone()), handle)
    }
}

impl InputBackend for FakeBackend {
    fn label(&self) -> &str {
        "test device"
    }

    fn poll(&mut self) -> Vec<Device> {
        self.0.with(|s| {
            vec![Device {
                id: "pad0".to_owned(),
                name: s.name.clone(),
                state: InputState {
                    connected: s.present,
                    axes: s.axes.clone(),
                    buttons: s.buttons.clone(),
                },
            }]
        })
    }
}

/// The fake pad shared by the control mode (`kopercad --control`): the visual test user moves it
/// with the `pad` command and the app reads it instead of hardware, so QA runs anywhere.
pub(crate) fn control_fake() -> FakeHandle {
    static SHARED: OnceLock<FakeHandle> = OnceLock::new();
    SHARED
        .get_or_init(|| FakeHandle(Arc::new(Mutex::new(FakeState::default()))))
        .clone()
}

/// Whether the app reads the shared fake pad instead of hardware.
pub(crate) fn use_control_fake() -> bool {
    crate::control::requested_port().is_some()
}

/// The devices of one backend, with the choice of which one drives the controller.
pub(super) struct InputService {
    backend: Box<dyn InputBackend>,
    devices: Vec<Device>,
    chosen: Option<String>,
}

impl InputService {
    pub(super) fn new(backend: Box<dyn InputBackend>) -> Self {
        InputService {
            backend,
            devices: Vec::new(),
            chosen: None,
        }
    }

    /// The service for this run: the shared fake pad under the control mode, `gilrs` on Windows,
    /// nothing elsewhere.
    pub(super) fn platform() -> Self {
        if use_control_fake() {
            return Self::new(Box::new(FakeBackend(control_fake())));
        }
        #[cfg(windows)]
        {
            match super::input_gilrs::GilrsBackend::new() {
                Ok(b) => Self::new(Box::new(b)),
                Err(e) => Self::new(Box::new(NoBackend(format!(
                    "Gamepad input could not start: {e}"
                )))),
            }
        }
        #[cfg(not(windows))]
        {
            Self::new(Box::new(NoBackend(
                "Gamepad input is available in the Windows build.".to_owned(),
            )))
        }
    }

    /// Read the devices. The first connected device is chosen when none is yet; a chosen device
    /// stays chosen when it disappears (it then reads as disconnected) until another is picked.
    pub(super) fn poll(&mut self) {
        self.devices = self.backend.poll();
        if self.chosen.is_none() {
            self.chosen = self
                .devices
                .iter()
                .find(|d| d.state.connected)
                .map(|d| d.id.clone());
        }
    }

    pub(super) fn label(&self) -> &str {
        self.backend.label()
    }

    pub(super) fn unavailable(&self) -> Option<&str> {
        self.backend.unavailable()
    }

    pub(super) fn devices(&self) -> &[Device] {
        &self.devices
    }

    pub(super) fn choose(&mut self, id: &str) {
        self.chosen = Some(id.to_owned());
    }

    pub(super) fn chosen(&self) -> Option<&Device> {
        let id = self.chosen.as_deref()?;
        self.devices.iter().find(|d| d.id == id)
    }

    /// The state of the chosen device; a disconnected one when it is gone.
    pub(super) fn state(&self) -> InputState {
        self.chosen()
            .map(|d| d.state.clone())
            .unwrap_or_else(InputState::disconnected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> (InputService, FakeHandle) {
        let (backend, hand) = FakeBackend::new();
        (InputService::new(Box::new(backend)), hand)
    }

    #[test]
    fn the_fake_pad_reports_every_standard_input_at_rest() {
        let (mut svc, _hand) = service();
        svc.poll();
        let d = svc.chosen().expect("chosen at once");
        assert!(d.state.connected);
        assert_eq!(d.state.axes.len(), AXES.len());
        assert_eq!(d.state.buttons.len(), BUTTONS.len());
        assert!(d.state.axes.values().all(|v| *v == 0.0));
    }

    #[test]
    fn injected_input_shows_in_the_next_poll() {
        let (mut svc, hand) = service();
        svc.poll();
        hand.set_axis("left_y", 0.75);
        hand.set_button("south", true);
        svc.poll();
        let s = svc.state();
        assert_eq!(s.axis("left_y"), 0.75);
        assert!(s.button("south"));
        hand.centre();
        svc.poll();
        assert_eq!(svc.state().axis("left_y"), 0.0);
        assert!(svc.state().connected);
    }

    #[test]
    fn unplugging_reads_as_disconnected_and_plugging_back_keeps_what_was_held() {
        let (mut svc, hand) = service();
        svc.poll();
        hand.set_axis("left_x", 1.0);
        hand.connect(false);
        svc.poll();
        assert!(!svc.state().connected);
        assert!(svc.chosen().is_some(), "still the chosen one, now gone");
        hand.connect(true);
        svc.poll();
        assert!(svc.state().connected);
        assert_eq!(svc.state().axis("left_x"), 1.0);
        hand.centre();
        svc.poll();
        assert_eq!(svc.state().axis("left_x"), 0.0);
    }

    #[test]
    fn a_service_without_a_backend_says_why_and_reads_nothing() {
        let mut svc = InputService::new(Box::new(NoBackend("not here".into())));
        svc.poll();
        assert!(svc.devices().is_empty());
        assert!(!svc.state().connected);
        assert_eq!(svc.unavailable(), Some("not here"));
    }

    #[test]
    fn a_second_device_does_not_take_over() {
        struct Two(bool);
        impl InputBackend for Two {
            fn label(&self) -> &str {
                "two"
            }
            fn poll(&mut self) -> Vec<Device> {
                let mk = |id: &str, on: bool| Device {
                    id: id.to_owned(),
                    name: id.to_owned(),
                    state: InputState {
                        connected: on,
                        ..InputState::default()
                    },
                };
                if self.0 {
                    vec![mk("a", true), mk("b", true)]
                } else {
                    vec![mk("b", true)]
                }
            }
        }
        let mut svc = InputService::new(Box::new(Two(true)));
        svc.poll();
        assert_eq!(svc.chosen().unwrap().id, "a");
        svc.backend = Box::new(Two(false));
        svc.poll();
        assert!(
            svc.chosen().is_none(),
            "a is gone; b is not picked behind the user's back"
        );
        svc.choose("b");
        assert!(svc.chosen().unwrap().state.connected);
    }
}
