//! The state of the add-ons as plain JSON, for `kopercad-cli addons ... --json` and the Python
//! functions, so scripts see exactly what the Manage add-ons window shows.

use serde_json::{Value, json};

use crate::manifest::HomeKind;
use crate::store::{AddonSet, Entry, Inspection, Status};
use crate::trust::Trust;

fn status_name(s: &Status) -> (&'static str, Option<&str>) {
    match s {
        Status::Enabled => ("enabled", None),
        Status::Disabled => ("disabled", None),
        Status::Incompatible(why) => ("incompatible", Some(why)),
        Status::Failed(why) => ("failed", Some(why)),
    }
}

fn trust_json(t: &Trust) -> Value {
    match t {
        Trust::Signed { key_id, owner } => json!({"signed": true, "key": key_id, "owner": owner}),
        Trust::Unsigned { note } => json!({"signed": false, "note": note}),
    }
}

/// One installed add-on.
pub fn entry_json(e: &Entry) -> Value {
    let (status, reason) = status_name(&e.status);
    let m = e.manifest.as_ref();
    json!({
        "id": e.id,
        "name": e.display_name(),
        "version": e.active.to_string(),
        "previous": e.previous.map(|v| v.to_string()),
        "publisher": m.map(|m| m.addon.publisher.clone()),
        "description": m.map(|m| m.addon.description.clone()),
        "status": status,
        "reason": reason,
        "trust": trust_json(&e.trust),
        "accepted_unsigned": e.accepted_unsigned,
        "permissions": m.map(|m| m.addon.permissions.iter().map(|p| p.key()).collect::<Vec<_>>()),
        "commands": m.map(|m| m.commands.iter().map(|c| json!({
            "id": m.command_id(&c.id),
            "label": c.label,
            "home": match c.home { HomeKind::Menu => "menu", HomeKind::Rail => "rail", HomeKind::PaletteOnly => "palette-only" },
        })).collect::<Vec<_>>()),
        "panels": m.map(|m| m.panels.iter().map(|p| p.id.clone()).collect::<Vec<_>>()),
        "files": m.map(|m| m.files.len()),
        "fingerprint": e.fingerprint,
        "folder": e.dir.display().to_string(),
    })
}

/// Everything installed, with the warnings of the scan.
pub fn set_json(set: &AddonSet) -> Value {
    json!({
        "addons": set.entries.iter().map(entry_json).collect::<Vec<_>>(),
        "warnings": set.warnings,
    })
}

/// What installing a file would do, as the install prompt shows it.
pub fn inspection_json(i: &Inspection) -> Value {
    let m = &i.package.manifest;
    json!({
        "id": m.addon.id,
        "name": m.addon.name,
        "version": m.addon.version,
        "publisher": m.addon.publisher,
        "description": m.addon.description,
        "permissions": m.addon.permissions.iter().map(|p| p.key()).collect::<Vec<_>>(),
        "trust": trust_json(&i.package.trust),
        "needs_trust_tick": i.needs_trust_tick(),
        "fingerprint": i.package.fingerprint,
        "file_sha256": i.package.file_sha256,
        "files": m.files.len(),
        "blocked": i.blocked(),
        "change": format!("{:?}", i.change),
    })
}
