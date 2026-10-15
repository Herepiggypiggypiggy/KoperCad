//! Blocks: parametric circuits captured once and placed many times
//! (DESIGN §7). A [`BlockDefinition`] holds template records; placing it
//! copies them onto a page with fresh ids, allocated tags and parameter
//! values filled in, and leaves a [`BlockInstance`] that remembers which
//! drawn object came from which template member.

use std::collections::{BTreeMap, BTreeSet};

use crate::entity::{
    BlockDefinition, BlockInstance, BlockMember, BlockOverride, BlockParameter, Device, Endpoint,
    Junction, Properties, SymbolInstance, SymbolRole, Text, TextContent, Value, Wire,
};
use crate::error::{ModelError, Result};
use crate::geom::Point;
use crate::id::{
    AnyId, BlockDefinitionId, BlockInstanceId, DeviceId, JunctionId, PageId, SymbolId, WireId,
};
use crate::record::{Record, endpoint_owner};
use crate::tag::DeviceTag;
use crate::tx::Tx;

/// The page id template members carry: they sit on no page.
const NO_PAGE: PageId = PageId(0);

impl Tx<'_> {
    /// Captures a block definition from the drawing: the given symbols,
    /// junctions and free texts, the devices of those symbols, the wires
    /// between them and every text attached to any of them. Positions are
    /// stored relative to `origin`. Wires to objects outside the selection
    /// are left out (they are how the block connects). References to
    /// project-wide objects (parts, potentials, feeds) are not captured.
    /// Terminal symbols, potential feeds and interruption points cannot be
    /// part of a block: an interruption point joins nets by name across the
    /// project, so two placements would be tied together.
    pub fn define_block(
        &mut self,
        name: impl Into<String>,
        items: &[AnyId],
        origin: Point,
        parameters: Vec<BlockParameter>,
    ) -> Result<BlockDefinitionId> {
        let project = self.project();
        let mut picked: BTreeSet<AnyId> = BTreeSet::new();
        for &item in items {
            match item {
                AnyId::Symbol(s) => {
                    let symbol = project.symbol(s).ok_or(ModelError::NotFound(item))?;
                    match symbol.role {
                        SymbolRole::Terminal { .. }
                        | SymbolRole::PotentialFeed { .. }
                        | SymbolRole::Interruption { .. } => {
                            return Err(ModelError::NotInBlock(item));
                        }
                        SymbolRole::Device { device, .. } => {
                            picked.insert(device.into());
                        }
                        SymbolRole::Unassigned => {}
                    }
                    picked.insert(item);
                }
                AnyId::Junction(j) if project.junction(j).is_some() => {
                    picked.insert(item);
                }
                AnyId::Text(t) if project.text(t).is_some() => {
                    picked.insert(item);
                }
                AnyId::Junction(_) | AnyId::Text(_) => return Err(ModelError::NotFound(item)),
                other => return Err(ModelError::NotInBlock(other)),
            }
        }
        let wires: Vec<WireId> = picked
            .iter()
            .flat_map(|&id| project.referrers(id))
            .filter_map(|r| match r {
                AnyId::Wire(w) => project.wire(w),
                _ => None,
            })
            .filter(|w| {
                w.ends()
                    .into_iter()
                    .all(|end| picked.contains(&endpoint_owner(end)))
            })
            .map(|w| w.id)
            .collect();
        picked.extend(wires.iter().map(|&w| AnyId::from(w)));
        let attached: Vec<AnyId> = picked
            .iter()
            .flat_map(|&id| project.referrers(id))
            .filter(|r| matches!(r, AnyId::Text(_)))
            .collect();
        picked.extend(attached);

        let mut members = Vec::with_capacity(picked.len());
        for id in picked {
            let mut record = project.record(id).ok_or(ModelError::NotFound(id))?;
            match &mut record {
                Record::Device(d) => d.parts.clear(),
                Record::Symbol(s) => {
                    s.page = NO_PAGE;
                    s.placement.origin = s.placement.origin - origin;
                    s.feeds.clear();
                }
                Record::Junction(j) => {
                    j.page = NO_PAGE;
                    j.position = j.position - origin;
                }
                Record::Wire(w) => {
                    w.page = NO_PAGE;
                    w.potential = None;
                    for point in &mut w.route {
                        *point = *point - origin;
                    }
                }
                Record::Text(t) => {
                    t.page = NO_PAGE;
                    if t.attached_to.is_none() {
                        t.placement.offset = t.placement.offset - origin;
                    }
                }
                _ => return Err(ModelError::NotInBlock(id)),
            }
            members.push(record);
        }
        let name = name.into();
        self.insert(|id| BlockDefinition {
            id,
            name,
            description: String::new(),
            parameters,
            members,
            source: None,
            properties: Properties::default(),
        })
    }

    /// Places a block: copies the definition's members onto `page` at
    /// `origin` with fresh ids, gives each device the next free number for
    /// its letter code in the page's structure, and fills in parameter
    /// values (`values` overrides the defaults). One undo step.
    pub fn place_block(
        &mut self,
        definition: BlockDefinitionId,
        page: PageId,
        origin: Point,
        values: Vec<(String, Value)>,
    ) -> Result<BlockInstanceId> {
        let def = self
            .project()
            .block_definition(definition)
            .ok_or(ModelError::NotFound(definition.into()))?
            .clone();
        let structure = self
            .project()
            .page(page)
            .ok_or(ModelError::NotFound(page.into()))?
            .structure
            .clone();
        let values = sorted_values(values);
        let filled = resolve(&def.parameters, &values);

        let mut templates: Vec<&Record> = def.members.iter().collect();
        templates.sort_by_key(|r| (Record::rank(r.kind()), r.id()));
        let mut map: BTreeMap<AnyId, AnyId> = BTreeMap::new();
        for template in templates {
            let actual: AnyId = match template {
                Record::Device(t) => {
                    let tag = self.allocate_tag(&structure, &t.tag);
                    let mut d = t.clone();
                    fill_device(&mut d, t, &filled);
                    self.insert(|id| Device { id, tag, ..d })?.into()
                }
                Record::Symbol(t) => {
                    let mut s = t.clone();
                    s.page = page;
                    s.placement.origin = t.placement.origin + origin;
                    if let SymbolRole::Device { device, .. } = &mut s.role {
                        *device = DeviceId(raw(&map, (*device).into())?);
                    }
                    s.properties = fill_properties(&t.properties, &filled);
                    self.insert(|id| SymbolInstance { id, ..s })?.into()
                }
                Record::Junction(t) => {
                    let position = t.position + origin;
                    self.insert(|id| Junction { id, page, position })?.into()
                }
                Record::Wire(t) => {
                    let mut w = t.clone();
                    w.page = page;
                    w.from = remap_end(&map, t.from)?;
                    w.to = remap_end(&map, t.to)?;
                    for point in &mut w.route {
                        *point = *point + origin;
                    }
                    w.properties = fill_properties(&t.properties, &filled);
                    self.insert(|id| Wire { id, ..w })?.into()
                }
                Record::Text(t) => {
                    let mut x = t.clone();
                    x.page = page;
                    match t.attached_to {
                        Some(owner) => x.attached_to = Some(remap(&map, owner)?),
                        None => x.placement.offset = t.placement.offset + origin,
                    }
                    fill_text(&mut x, t, &filled);
                    self.insert(|id| Text { id, ..x })?.into()
                }
                other => return Err(ModelError::NotInBlock(other.id())),
            };
            map.insert(template.id(), actual);
        }
        let members = map
            .into_iter()
            .map(|(template, actual)| BlockMember {
                template,
                actual,
                overrides: Vec::new(),
            })
            .collect();
        self.insert(|id| BlockInstance {
            id,
            definition,
            page,
            origin,
            values,
            members,
            detached: None,
            properties: Properties::default(),
        })
    }

    /// Sets a placed block's parameter values and fills them in again:
    /// device descriptions and properties, symbol and wire properties and
    /// literal texts follow the definition with the new values, except
    /// members the user edited by hand ([`BlockOverride::Value`]).
    /// Properties the user added that the definition does not have stay.
    pub fn set_block_values(
        &mut self,
        instance: BlockInstanceId,
        values: Vec<(String, Value)>,
    ) -> Result<()> {
        let values = sorted_values(values);
        self.update::<BlockInstance>(instance, |b| b.values = values)?;
        let block = self.project().block_instance(instance).cloned();
        let block = block.ok_or(ModelError::NotFound(instance.into()))?;
        let def = self
            .project()
            .block_definition(block.definition)
            .ok_or(ModelError::NotFound(block.definition.into()))?
            .clone();
        let filled = resolve(&def.parameters, &block.values);
        for member in &block.members {
            if member.overrides.contains(&BlockOverride::Value) {
                continue;
            }
            let Some(template) = def.members.iter().find(|t| t.id() == member.template) else {
                continue;
            };
            let Some(mut record) = self.project().record(member.actual) else {
                continue;
            };
            match (&mut record, template) {
                (Record::Device(d), Record::Device(t)) => fill_device(d, t, &filled),
                (Record::Text(x), Record::Text(t)) => fill_text(x, t, &filled),
                (Record::Symbol(s), Record::Symbol(t)) => {
                    refill(&mut s.properties, &t.properties, &filled)
                }
                (Record::Wire(w), Record::Wire(t)) => {
                    refill(&mut w.properties, &t.properties, &filled)
                }
                _ => continue,
            }
            self.replace(record)?;
        }
        Ok(())
    }

    /// Marks (`on`) or clears a hand edit on a block member, so parameter
    /// edits and definition updates keep (or retake) that field.
    pub fn set_block_override(
        &mut self,
        instance: BlockInstanceId,
        member: AnyId,
        which: BlockOverride,
        on: bool,
    ) -> Result<()> {
        let block = self
            .project()
            .block_instance(instance)
            .ok_or(ModelError::NotFound(instance.into()))?;
        if block.member(member).is_none() {
            return Err(ModelError::NotFound(member));
        }
        self.update::<BlockInstance>(instance, |b| {
            if let Some(m) = b.members.iter_mut().find(|m| m.actual == member) {
                m.overrides.retain(|&o| o != which);
                if on {
                    m.overrides.push(which);
                    m.overrides.sort();
                }
            }
        })
    }

    /// Stops a placed block from following its definition, with the reason
    /// the inspector shows. The drawing is unchanged.
    pub fn detach_block(
        &mut self,
        instance: BlockInstanceId,
        reason: impl Into<String>,
    ) -> Result<()> {
        let reason = reason.into();
        self.update::<BlockInstance>(instance, |b| b.detached = Some(reason))
    }

    /// Called when a wire is drawn: a new connection between two members
    /// of the same placed block is a structural edit and detaches it. A
    /// wire from a member out to the rest of the circuit is how a block is
    /// used and keeps the link.
    pub(crate) fn detach_on_connect(&mut self, from: Endpoint, to: Endpoint) -> Result<()> {
        let (a, b) = (endpoint_owner(from), endpoint_owner(to));
        let blocks: Vec<BlockInstanceId> = self
            .project()
            .block_instances()
            .filter(|b| b.detached.is_none())
            .filter(|bl| {
                let has = |id| bl.members.iter().any(|m| m.actual == id);
                has(a) && has(b)
            })
            .map(|b| b.id)
            .collect();
        if blocks.is_empty() {
            return Ok(());
        }
        let reason = format!(
            "connection {} to {} added",
            self.endpoint_name(from),
            self.endpoint_name(to)
        );
        for block in blocks {
            self.detach_block(block, reason.clone())?;
        }
        Ok(())
    }

    /// Called at the end of every edit: a device outside any block whose
    /// wires (two or more) all land on members of one placed block was
    /// added inside it, a structural edit that detaches it. A device with
    /// only its first wire in stays linked until it is clearly inside.
    pub(crate) fn detach_enclosed_devices(&mut self) -> Result<()> {
        let added = self.added_wires();
        if added.is_empty() || self.project().block_instances().next().is_none() {
            return Ok(());
        }
        let member_of = |project: &crate::Project, id: AnyId| {
            project
                .block_instances()
                .find(|b| b.members.iter().any(|m| m.actual == id))
                .map(|b| b.id)
        };
        let device_of = |project: &crate::Project, id: AnyId| match id {
            AnyId::Symbol(s) => match project.symbol(s).map(|s| &s.role) {
                Some(SymbolRole::Device { device, .. }) => Some(*device),
                _ => None,
            },
            _ => None,
        };
        let mut candidates = BTreeSet::new();
        for wire in added {
            let Some(w) = self.project().wire(wire) else {
                continue;
            };
            for end in w.ends() {
                let owner = endpoint_owner(end);
                if let Some(device) = device_of(self.project(), owner)
                    && member_of(self.project(), device.into()).is_none()
                {
                    candidates.insert(device);
                }
            }
        }
        let mut detach = Vec::new();
        for device in candidates {
            let project = self.project();
            let symbols: BTreeSet<AnyId> = project
                .symbols_of_device(device)
                .iter()
                .map(|s| AnyId::Symbol(s.id))
                .collect();
            let mut blocks = BTreeSet::new();
            let mut count = 0;
            let mut outside = false;
            for &symbol in &symbols {
                for referrer in project.referrers(symbol) {
                    let AnyId::Wire(w) = referrer else { continue };
                    let Some(w) = project.wire(w) else { continue };
                    for end in w.ends() {
                        let owner = endpoint_owner(end);
                        if symbols.contains(&owner) {
                            continue;
                        }
                        count += 1;
                        match member_of(project, owner) {
                            Some(b) => {
                                blocks.insert(b);
                            }
                            None => outside = true,
                        }
                    }
                }
            }
            if outside || count < 2 || blocks.len() != 1 {
                continue;
            }
            let block = blocks.into_iter().next().expect("one block");
            if project
                .block_instance(block)
                .is_some_and(|b| b.detached.is_none())
            {
                let tag = project
                    .device(device)
                    .map(|d| d.tag.to_string())
                    .filter(|t| !t.is_empty())
                    .unwrap_or_else(|| device.to_string());
                detach.push((block, format!("device {tag} added inside the block")));
            }
        }
        for (block, reason) in detach {
            self.detach_block(block, reason)?;
        }
        Ok(())
    }

    /// How a wire end reads to the user: `-K1:13` for a pin of a tagged
    /// device, the id otherwise.
    fn endpoint_name(&self, end: Endpoint) -> String {
        let Endpoint::Pin { symbol, pin } = end else {
            return endpoint_owner(end).to_string();
        };
        let Some(s) = self.project().symbol(symbol) else {
            return symbol.to_string();
        };
        let owner = match &s.role {
            SymbolRole::Device { device, .. } => self
                .project()
                .device(*device)
                .map(|d| d.tag.to_string())
                .filter(|t| !t.is_empty()),
            _ => None,
        }
        .unwrap_or_else(|| symbol.to_string());
        match s.pins.get(usize::from(pin)) {
            Some(p) if !p.name.is_empty() => format!("{owner}:{}", p.name),
            _ => format!("{owner}:{pin}"),
        }
    }

    /// Called before an object is deleted: drops it from the block it
    /// belongs to and detaches that block (a structural edit).
    pub(crate) fn forget_block_member(&mut self, id: AnyId) -> Result<()> {
        let blocks: Vec<BlockInstanceId> = self
            .project()
            .block_instances()
            .filter(|b| b.members.iter().any(|m| m.actual == id))
            .map(|b| b.id)
            .collect();
        if blocks.is_empty() {
            return Ok(());
        }
        let what = match id {
            AnyId::Device(d) => self
                .project()
                .device(d)
                .map(|d| d.tag.to_string())
                .filter(|t| !t.is_empty()),
            _ => None,
        }
        .unwrap_or_else(|| id.to_string());
        for block in blocks {
            self.update::<BlockInstance>(block, |b| {
                b.members.retain(|m| m.actual != id);
                if b.detached.is_none() {
                    b.detached = Some(format!("{what} was deleted"));
                }
            })?;
        }
        Ok(())
    }

    /// The tag a placed device gets: the template's product levels in the
    /// page's structure, numbered next free for its letter code. A device
    /// without a tag in the template stays without one.
    // TODO: follow the project's numbering rule (DESIGN §7) once per-page
    // numbering (-K301) is live; this always numbers per letter code.
    fn allocate_tag(&self, structure: &DeviceTag, template: &DeviceTag) -> DeviceTag {
        if template.product.is_empty() {
            return DeviceTag::default();
        }
        let mut tag = structure.clone();
        tag.product = template.product.clone();
        let number = self.project().next_tag_number(&tag, tag.letter_code());
        tag.with_number(number)
    }
}

fn sorted_values(mut values: Vec<(String, Value)>) -> Vec<(String, Value)> {
    values.sort_by(|a, b| a.0.cmp(&b.0));
    values.dedup_by(|later, earlier| later.0 == earlier.0);
    values
}

/// Every parameter with its value for a placement.
fn resolve(parameters: &[BlockParameter], values: &[(String, Value)]) -> Vec<(String, Value)> {
    parameters
        .iter()
        .map(|p| {
            let value = values
                .iter()
                .find(|(n, _)| *n == p.name)
                .map_or(&p.default, |(_, v)| v);
            (p.name.clone(), value.clone())
        })
        .collect()
}

/// `{name}` replaced by the parameter's value in a text.
fn fill_str(text: &str, values: &[(String, Value)]) -> String {
    let mut out = text.to_owned();
    for (name, value) in values {
        let key = format!("{{{name}}}");
        if out.contains(&key) {
            out = out.replace(&key, &value.to_string());
        }
    }
    out
}

/// A property value with parameters filled in. A text that is exactly
/// `{name}` takes the parameter's value with its type (a number stays a
/// number); other texts get the value written in.
fn fill_value(value: &Value, values: &[(String, Value)]) -> Value {
    match value {
        Value::Text(text) => {
            let whole = text
                .strip_prefix('{')
                .and_then(|t| t.strip_suffix('}'))
                .and_then(|name| values.iter().find(|(n, _)| n == name));
            match whole {
                Some((_, v)) => v.clone(),
                None => Value::Text(fill_str(text, values)),
            }
        }
        other => other.clone(),
    }
}

fn fill_properties(template: &Properties, values: &[(String, Value)]) -> Properties {
    template
        .iter()
        .map(|(k, v)| (k.clone(), fill_value(v, values)))
        .collect()
}

/// Sets the template's properties again on a placed object, keeping any
/// property the template does not have.
fn refill(properties: &mut Properties, template: &Properties, values: &[(String, Value)]) {
    for (key, value) in template {
        properties.insert(key.clone(), fill_value(value, values));
    }
}

fn fill_device(device: &mut Device, template: &Device, values: &[(String, Value)]) {
    device.description = fill_str(&template.description, values);
    refill(&mut device.properties, &template.properties, values);
}

fn fill_text(text: &mut Text, template: &Text, values: &[(String, Value)]) {
    if let TextContent::Literal { text: t } = &template.content {
        text.content = TextContent::Literal {
            text: fill_str(t, values),
        };
    }
    refill(&mut text.properties, &template.properties, values);
}

fn remap(map: &BTreeMap<AnyId, AnyId>, id: AnyId) -> Result<AnyId> {
    map.get(&id).copied().ok_or(ModelError::NotFound(id))
}

fn raw(map: &BTreeMap<AnyId, AnyId>, id: AnyId) -> Result<u64> {
    Ok(remap(map, id)?.raw())
}

fn remap_end(map: &BTreeMap<AnyId, AnyId>, end: Endpoint) -> Result<Endpoint> {
    Ok(match end {
        Endpoint::Pin { symbol, pin } => Endpoint::Pin {
            symbol: SymbolId(raw(map, symbol.into())?),
            pin,
        },
        Endpoint::Junction { junction } => Endpoint::Junction {
            junction: JunctionId(raw(map, junction.into())?),
        },
    })
}
