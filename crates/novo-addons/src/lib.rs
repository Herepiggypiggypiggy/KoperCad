//! Add-on manager core (lane C0b): `.kcaddon` packages, the per-user install registry,
//! version compatibility, permissions and the sandboxed add-on host. Plan:
//! `plans/controller-live-view.md` section 5b. Pure: no UI, no network, no admin rights.
//! This commit only registers the crate; C0b-1 fills it.
