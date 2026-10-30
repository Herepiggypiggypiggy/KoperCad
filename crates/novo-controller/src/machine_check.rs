//! Binding checks for machine targets (lane L5b): the module and port exist, the signal fits the
//! kind of port, one module is not commanded by controllers that are not all `shared`.
//!
//! Ownership of a single port by several controllers, a chain that does not combine as `and` and a
//! target that mixes `and` with ordinary signals are already found by
//! [`BindingTable::check`] because a machine target is a target like any other.

use std::collections::{BTreeMap, BTreeSet};

use crate::binding::{Binding, BindingFinding, BindingProblem, BindingTable, SignalKey, Target};
use crate::machine::{MachineCatalog, MachinePath, PortKind};
use crate::profile::SignalKind;
use crate::set::ControllerSet;
use crate::validate::Severity;

fn names(ports: &[crate::machine::PortEntry]) -> String {
    ports
        .iter()
        .map(|p| p.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

impl BindingTable {
    /// The machine part of the checks, for the bindings whose target is a `ModulePath`.
    pub fn check_machine(
        &self,
        set: &ControllerSet,
        catalog: &MachineCatalog,
    ) -> Vec<BindingFinding> {
        let mut out: Vec<BindingFinding> = Vec::new();
        let mut finding = |problem, severity, key: &SignalKey, message: String| {
            out.push(BindingFinding {
                problem,
                severity,
                key: key.clone(),
                message,
            });
        };
        // canonical port -> the bindings that reach it, module -> controllers that command it
        let mut reached: BTreeMap<MachinePath, Vec<&Binding>> = BTreeMap::new();
        let mut commanded: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
        for b in &self.bindings {
            let Target::ModulePath(text) = &b.target else {
                continue;
            };
            let path = match MachinePath::parse(text) {
                Ok(p) => p,
                Err(e) => {
                    finding(
                        BindingProblem::MachineSyntax,
                        Severity::Error,
                        &b.key,
                        format!("{} -> {}: {e}", b.key, b.target),
                    );
                    continue;
                }
            };
            let Some(module) = catalog.module(&path) else {
                let known: Vec<&str> = catalog.modules.iter().map(|m| m.path.as_str()).collect();
                let hint = if known.is_empty() {
                    "the drawing has no hoist or travel axis with its mechanical data yet"
                        .to_owned()
                } else {
                    format!("known: {}", known.join(", "))
                };
                finding(
                    BindingProblem::MachineUnknownModule,
                    Severity::Error,
                    &b.key,
                    format!(
                        "{} -> {}: no module {} in the machine model ({hint})",
                        b.key,
                        b.target,
                        path.module()
                    ),
                );
                continue;
            };
            let Some(port_name) = path.port() else {
                finding(
                    BindingProblem::MachineNoPort,
                    Severity::Error,
                    &b.key,
                    format!(
                        "{} -> {}: names the module {} and no port; write {}@ and one of: {}",
                        b.key,
                        b.target,
                        module.path,
                        module.path,
                        names(&module.ports)
                    ),
                );
                continue;
            };
            let Some(port) = module.port(port_name) else {
                finding(
                    BindingProblem::MachineUnknownPort,
                    Severity::Error,
                    &b.key,
                    format!(
                        "{} -> {}: {} has no port {port_name} (it has: {})",
                        b.key,
                        b.target,
                        module.path,
                        names(&module.ports)
                    ),
                );
                continue;
            };
            if let Some(def) = set
                .instance(&b.key.controller)
                .and_then(|i| i.controller.profile().signal(&b.key.signal))
            {
                let analog = def.kind == SignalKind::Analog;
                let wrong = match port.kind {
                    PortKind::Command if def.ungated => Some((
                        Severity::Error,
                        "an ungated chain signal (E-stop, main contact) would command a motion; bind it to a chain port",
                    )),
                    PortKind::Chain if !def.ungated => Some((
                        Severity::Warning,
                        "a motion signal is high only while its button is held, so the module would be permitted only then; a chain port wants the E-stop chain signal",
                    )),
                    _ if analog => Some((
                        Severity::Warning,
                        "an analog value drives a switching port; any value other than 0 counts as on",
                    )),
                    _ => None,
                };
                if let Some((severity, why)) = wrong {
                    finding(
                        BindingProblem::MachineWrongKind,
                        severity,
                        &b.key,
                        format!(
                            "{} -> {}: {why} ({} port)",
                            b.key,
                            b.target,
                            port.kind.word()
                        ),
                    );
                }
            }
            if let Ok(canonical) = MachinePath::new(&module.path, port_name) {
                reached.entry(canonical).or_default().push(b);
            }
            if port.kind == PortKind::Command {
                commanded
                    .entry(module.path.clone())
                    .or_default()
                    .insert(b.key.controller.as_str());
            }
        }
        // Two spellings of one port (`hoist/=A-M1@up` and `=A-M1@up`) are two targets to the table
        // but one port to the machine.
        for (canonical, list) in &reached {
            let spellings: BTreeSet<&str> = list
                .iter()
                .map(|b| match &b.target {
                    Target::ModulePath(t) => t.as_str(),
                    _ => "",
                })
                .collect();
            let controllers: BTreeSet<&str> =
                list.iter().map(|b| b.key.controller.as_str()).collect();
            if spellings.len() > 1 {
                finding(
                    BindingProblem::TargetConflict,
                    Severity::Warning,
                    &list[0].key,
                    format!(
                        "{canonical} is written as {}: use one spelling so the checks see one target{}",
                        spellings.into_iter().collect::<Vec<_>>().join(" and "),
                        if controllers.len() > 1 {
                            " (several controllers reach it)"
                        } else {
                            ""
                        }
                    ),
                );
            }
        }
        for (module, controllers) in commanded {
            if controllers.len() > 1 && controllers.iter().any(|c| !set.is_shared(c)) {
                let first = self
                    .bindings
                    .iter()
                    .find(|b| controllers.contains(b.key.controller.as_str()))
                    .map(|b| b.key.clone());
                if let Some(key) = first {
                    finding(
                        BindingProblem::MachineTwoOwners,
                        Severity::Error,
                        &key,
                        format!(
                            "module {module} is commanded by controllers {} and not all are marked shared",
                            controllers.into_iter().collect::<Vec<_>>().join(", ")
                        ),
                    );
                }
            }
        }
        out
    }
}
