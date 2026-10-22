//! Field results as a file: what a person did at the machine travels as one
//! JSON document (`.kcfield`), by USB stick or mail, and is merged back into
//! the project on the desk. No server is involved.
//!
//! Merging is per decision: for every check the newer `at` wins, for every
//! punch item the one that moved further (or later) wins. Items are matched
//! by who raised them, when and the title, because two devices number their
//! items independently.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::build::BuildPlan;
use crate::check::{CheckKind, Target};
use crate::photo::{self, is_photo_name};
use crate::progress::targets;
use crate::punch::{PunchItem, PunchState};
use crate::redline::{Change, Redline, RedlineState};
use crate::store::CommissioningStore;

/// Marker in the file so other JSON is not taken for a field package.
const FORMAT: &str = "kopercad-field";
const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FieldError {
    #[error("this is not a KoperCAD field results file")]
    NotAFieldFile,
    #[error("the field results file is from a newer version ({0})")]
    TooNew(u32),
    #[error("the field results file is unreadable: {0}")]
    Unreadable(String),
}

/// Field results of one project, ready to write to a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldPackage {
    format: String,
    version: u32,
    /// The project it was taken from, shown when importing.
    pub project: String,
    /// Who wrote the package and when (ISO 8601 text).
    pub exported_by: String,
    pub exported_at: String,
    pub store: CommissioningStore,
    /// The tag or wire number every target had when exported. Object ids are
    /// only numbers, so a file from another project is told apart by these:
    /// a decision is merged only where the label still matches.
    labels: Vec<TargetLabel>,
    /// The pictures the punch items name, so photos taken on a tablet travel
    /// in the same file. Text-encoded (base64).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    photos: Vec<PhotoFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PhotoFile {
    name: String,
    data: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TargetLabel {
    target: Target,
    label: String,
}

/// What a merge did, for the message after an import.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergeReport {
    pub checks_added: usize,
    pub checks_updated: usize,
    /// Decisions in the file that were older than the ones already here.
    pub checks_kept: usize,
    /// Decisions about objects this project does not have.
    pub unknown_targets: usize,
    /// Decisions whose object has another tag or number here: the file is
    /// probably from a different project. They are not merged.
    pub mismatched_targets: usize,
    pub punch_added: usize,
    pub punch_updated: usize,
    pub redlines_added: usize,
    pub redlines_updated: usize,
    pub build_added: usize,
    pub build_updated: usize,
    pub claims_added: usize,
    pub claims_updated: usize,
    /// Photos attached to items that were already here.
    pub photos_added: usize,
}

impl MergeReport {
    pub fn changed(&self) -> bool {
        self.checks_added
            + self.checks_updated
            + self.punch_added
            + self.punch_updated
            + self.redlines_added
            + self.redlines_updated
            + self.build_added
            + self.build_updated
            + self.claims_added
            + self.claims_updated
            + self.photos_added
            > 0
    }

    pub fn summary(&self) -> String {
        let mut text = format!(
            "{} new and {} changed checks, {} new and {} changed punch items",
            self.checks_added, self.checks_updated, self.punch_added, self.punch_updated
        );
        if self.redlines_added + self.redlines_updated > 0 {
            text.push_str(&format!(
                ", {} new and {} changed redlines",
                self.redlines_added, self.redlines_updated
            ));
        }
        if self.build_added + self.build_updated > 0 {
            text.push_str(&format!(
                ", {} new and {} changed build steps",
                self.build_added, self.build_updated
            ));
        }
        if self.claims_added + self.claims_updated > 0 {
            text.push_str(&format!(
                ", {} new and {} changed step claims",
                self.claims_added, self.claims_updated
            ));
        }
        if self.photos_added > 0 {
            text.push_str(&format!(", {} new photos", self.photos_added));
        }
        if self.checks_kept > 0 {
            text.push_str(&format!(
                ", {} older decisions kept as they were",
                self.checks_kept
            ));
        }
        if self.mismatched_targets > 0 {
            text.push_str(&format!(
                ", {} skipped (tag does not match, other project?)",
                self.mismatched_targets
            ));
        }
        if self.unknown_targets > 0 {
            text.push_str(&format!(
                ", {} skipped (not in this project)",
                self.unknown_targets
            ));
        }
        text
    }
}

impl FieldPackage {
    pub fn new(
        project_name: &str,
        by: &str,
        at: &str,
        store: &CommissioningStore,
        project: &novo_core::Project,
    ) -> Self {
        let labels = targets(project)
            .into_iter()
            .map(|t| TargetLabel {
                target: t.target,
                label: t.label,
            })
            .collect();
        FieldPackage {
            labels,
            format: FORMAT.to_owned(),
            version: VERSION,
            project: project_name.to_owned(),
            exported_by: by.to_owned(),
            exported_at: at.to_owned(),
            store: store.clone(),
            photos: Vec::new(),
        }
    }

    /// Packs the picture files the punch items name. `read` returns the
    /// bytes of a photo by name (None when the file is missing); pictures
    /// nobody names are left out.
    pub fn attach_photos(&mut self, mut read: impl FnMut(&str) -> Option<Vec<u8>>) {
        let mut names: Vec<&str> = self
            .store
            .punch_items()
            .flat_map(|p| p.photos.iter().map(|ph| ph.name.as_str()))
            .collect();
        names.sort_unstable();
        names.dedup();
        self.photos = names
            .into_iter()
            .filter(|n| is_photo_name(n))
            .filter_map(|n| {
                read(n).map(|bytes| PhotoFile {
                    name: n.to_owned(),
                    data: photo::encode(&bytes),
                })
            })
            .collect();
    }

    /// The picture files in the package as (name, bytes). Names that could
    /// not have come from [`photo_name`], unreadable data and pictures whose
    /// bytes do not hash to their name are dropped.
    pub fn photo_files(&self) -> Vec<(String, Vec<u8>)> {
        self.photos
            .iter()
            .filter(|p| is_photo_name(&p.name))
            .filter_map(|p| photo::decode(&p.data).map(|b| (p.name.clone(), b)))
            // A hand-edited file cannot plant another picture under a real name.
            .filter(|(name, bytes)| photo::name_matches(name, bytes))
            .collect()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("the package is plain data")
    }

    pub fn from_json(json: &str) -> Result<Self, FieldError> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| FieldError::Unreadable(e.to_string()))?;
        if value.get("format").and_then(|f| f.as_str()) != Some(FORMAT) {
            return Err(FieldError::NotAFieldFile);
        }
        let version = value.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        if version > VERSION {
            return Err(FieldError::TooNew(version));
        }
        serde_json::from_value(value).map_err(|e| FieldError::Unreadable(e.to_string()))
    }

    /// Merges the package into `store`. `project` tells which objects exist,
    /// so decisions about deleted ones are skipped. Nothing is lost: a
    /// decision already here that is newer stays.
    pub fn merge_into(
        &self,
        store: &mut CommissioningStore,
        project: &novo_core::Project,
    ) -> MergeReport {
        let mut report = MergeReport::default();
        let here: BTreeMap<Target, String> = targets(project)
            .into_iter()
            .map(|t| (t.target, t.label))
            .collect();
        let there: BTreeMap<Target, &str> = self
            .labels
            .iter()
            .map(|l| (l.target, l.label.as_str()))
            .collect();
        let verdict = |target: Target| -> Verdict {
            match (here.get(&target), there.get(&target)) {
                (None, _) if !target_exists(project, target) => Verdict::Unknown,
                (Some(a), Some(b)) if a == b => Verdict::Same,
                _ => Verdict::Other,
            }
        };
        for incoming in self.store.records() {
            match verdict(incoming.target) {
                Verdict::Same => {}
                Verdict::Unknown => {
                    report.unknown_targets += 1;
                    continue;
                }
                Verdict::Other => {
                    report.mismatched_targets += 1;
                    continue;
                }
            }
            match store.record(incoming.target, incoming.check) {
                None => {
                    store.put_record(incoming.clone());
                    report.checks_added += 1;
                }
                Some(have) if have == incoming => {}
                Some(have) if have.at < incoming.at => {
                    store.put_record(incoming.clone());
                    report.checks_updated += 1;
                }
                Some(_) => report.checks_kept += 1,
            }
        }
        for incoming in self.store.punch_items() {
            // An item about an object that is not the same here is kept as a
            // free-standing one, so the finding is not lost.
            let mut incoming = incoming.clone();
            if incoming.target.is_some_and(|t| verdict(t) != Verdict::Same) {
                incoming.target = None;
                incoming.check = None;
            }
            let incoming = &incoming;
            let key = punch_key(incoming);
            let found = store
                .punch_items()
                .find(|p| punch_key(p) == key)
                .map(|p| (p.id, p.clone()));
            match found {
                None => {
                    store.add_punch(incoming.clone());
                    report.punch_added += 1;
                }
                Some((id, have)) => {
                    let mut item = if rank(incoming) > rank(&have) {
                        report.punch_updated += 1;
                        incoming.clone()
                    } else {
                        have.clone()
                    };
                    item.id = id;
                    // Photos from both sides are kept.
                    for photo in have.photos.iter().chain(&incoming.photos) {
                        if !item.photos.iter().any(|p| p.name == photo.name) {
                            item.photos.push(photo.clone());
                        }
                    }
                    report.photos_added += incoming
                        .photos
                        .iter()
                        .filter(|ph| !have.photos.iter().any(|p| p.name == ph.name))
                        .count();
                    if item != have {
                        store.put_punch(item);
                    }
                }
            }
        }
        for incoming in self.store.redlines() {
            match verdict(incoming.target) {
                Verdict::Same => {}
                Verdict::Unknown => {
                    report.unknown_targets += 1;
                    continue;
                }
                Verdict::Other => {
                    report.mismatched_targets += 1;
                    continue;
                }
            }
            let found = store
                .redlines()
                .find(|r| redline_key(r) == redline_key(incoming))
                .cloned();
            match found {
                None => {
                    if store.add_redline(incoming.clone()).is_ok() {
                        report.redlines_added += 1;
                    }
                }
                Some(have) => {
                    if have.state == RedlineState::Open && incoming.state != RedlineState::Open {
                        let mut redline = incoming.clone();
                        redline.id = have.id;
                        store.put_redline(redline);
                        report.redlines_updated += 1;
                    }
                }
            }
        }
        if self.store.build_ticks().next().is_some() || self.store.build_claims().next().is_some() {
            // Computing the plan routes every wire: only when ticks came.
            let plan = BuildPlan::of(project);
            for incoming in self.store.build_ticks() {
                let Some((_, step)) = plan.find(incoming.key) else {
                    report.unknown_targets += 1;
                    continue;
                };
                if step.mark != incoming.mark {
                    report.mismatched_targets += 1;
                    continue;
                }
                match store.build_tick(incoming.key) {
                    None => {
                        store.put_build(incoming.clone());
                        report.build_added += 1;
                    }
                    Some(have) if have == incoming => {}
                    Some(have) if have.at < incoming.at => {
                        store.put_build(incoming.clone());
                        report.build_updated += 1;
                    }
                    Some(_) => report.checks_kept += 1,
                }
            }
            // A claim is a signal: the newest record wins, a release included.
            for incoming in self.store.build_claims() {
                let Some((_, step)) = plan.find(incoming.key) else {
                    report.unknown_targets += 1;
                    continue;
                };
                if step.mark != incoming.mark {
                    report.mismatched_targets += 1;
                    continue;
                }
                match store.build_claim_record(incoming.key) {
                    None => {
                        store.put_claim(incoming.clone());
                        report.claims_added += 1;
                    }
                    Some(have) if have == incoming => {}
                    Some(have) if have.at < incoming.at => {
                        store.put_claim(incoming.clone());
                        report.claims_updated += 1;
                    }
                    Some(_) => {}
                }
            }
        }
        report
    }
}

fn redline_key(r: &Redline) -> (String, String, Target, Change) {
    (r.at.clone(), r.by.clone(), r.target, r.change.clone())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Same,
    Other,
    Unknown,
}

fn target_exists(project: &novo_core::Project, target: Target) -> bool {
    match target {
        Target::Wire(id) => project.wire(id).is_some(),
        Target::Terminal(id) => project.terminal(id).is_some(),
        Target::Device(id) => project.device(id).is_some(),
    }
}

type PunchKey = (String, String, String, Option<(Target, Option<CheckKind>)>);

fn punch_key(item: &PunchItem) -> PunchKey {
    (
        item.raised_at.clone(),
        item.raised_by.clone(),
        item.title.clone(),
        item.target.map(|t| (t, item.check)),
    )
}

/// Which copy of an item is further along: closed beats fixed beats open,
/// then the later closing time.
fn rank(item: &PunchItem) -> (u8, String) {
    let step = match item.state {
        PunchState::Open => 0,
        PunchState::Fixed => 1,
        PunchState::Verified | PunchState::WontFix => 2,
    };
    (step, item.closed_at.clone().unwrap_or_default())
}
