//! The scan: read inputs, run the program once, write outputs.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::address::{Address, Size, Space};
use super::ast::*;
use super::blocks::{FbState, Field};
use super::model::Program;
use super::ops;
use super::value::{DataType, Value};

/// A fault that stops a scan (the controller would go to STOP).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeError {
    pub line: u32,
    pub message: String,
}

/// Longest a single loop may run, and most statements one scan may execute,
/// before the watchdog stops the program.
const LOOP_LIMIT: u32 = 100_000;
const SCAN_BUDGET: u64 = 5_000_000;

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for RuntimeError {}

#[derive(Debug, Clone)]
enum Cell {
    Val(DataType, Value),
    Block(Box<FbState>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    Next,
    Exit,
    Continue,
    Return,
}

/// One row of a watch table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watch {
    pub name: String,
    pub ty: String,
    pub value: String,
    pub address: Option<Address>,
}

/// A program with its state: variables, block instances, the I/O image and
/// the clock.
#[derive(Debug, Clone)]
pub struct Runtime {
    program: Arc<Program>,
    cells: Vec<Cell>,
    probes: Vec<bool>,
    inputs: BTreeMap<Address, Value>,
    outputs: BTreeMap<Address, Value>,
    memory: BTreeMap<Address, Value>,
    now_us: i64,
    scans: u64,
    steps: u64,
}

fn fresh_cells(program: &Program) -> Vec<Cell> {
    program
        .vars
        .iter()
        .map(|v| match &v.ty {
            VarType::Elem(t) => Cell::Val(*t, v.init),
            VarType::Block(kind) => {
                let mut state = FbState::new(*kind);
                for &(i, value) in &v.block_init {
                    state.inputs[i] = value;
                }
                Cell::Block(Box::new(state))
            }
        })
        .collect()
}

fn zero_of(a: Address) -> Value {
    if a.is_bit() {
        Value::Bool(false)
    } else {
        Value::Int(0)
    }
}

/// An image value as the address's size stores it.
fn image_value(a: Address, v: Value) -> Value {
    match a.size {
        Size::Bit => Value::Bool(v.as_bool()),
        Size::Byte => Value::Int(v.as_int() & 0xFF),
        Size::Word => Value::Int(v.as_int() & 0xFFFF),
        Size::DWord => Value::Int(v.as_int() & 0xFFFF_FFFF),
    }
}

impl Runtime {
    pub fn new(program: Program) -> Runtime {
        Self::from_shared(Arc::new(program))
    }

    pub fn from_shared(program: Arc<Program>) -> Runtime {
        Runtime {
            cells: fresh_cells(&program),
            probes: vec![false; program.probes],
            program,
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
            memory: BTreeMap::new(),
            now_us: 0,
            scans: 0,
            steps: 0,
        }
    }

    pub fn program(&self) -> &Program {
        &self.program
    }

    /// Cold start: variables back to their initial values, the I/O image,
    /// the clock and every block cleared.
    pub fn reset(&mut self) {
        self.cells = fresh_cells(&self.program);
        self.probes.iter_mut().for_each(|p| *p = false);
        self.inputs.clear();
        self.outputs.clear();
        self.memory.clear();
        self.now_us = 0;
        self.scans = 0;
    }

    /// Microseconds the program has been running.
    pub fn time_us(&self) -> i64 {
        self.now_us
    }

    pub fn scans(&self) -> u64 {
        self.scans
    }

    // ---- the I/O image ------------------------------------------------

    /// Sets what an input channel reads; takes effect at the next scan.
    pub fn set_input(&mut self, address: Address, value: Value) {
        self.inputs.insert(address, image_value(address, value));
    }

    pub fn set_input_bool(&mut self, address: Address, on: bool) {
        self.set_input(address, Value::Bool(on));
    }

    pub fn input(&self, address: Address) -> Value {
        self.inputs
            .get(&address)
            .copied()
            .unwrap_or_else(|| zero_of(address))
    }

    pub fn output(&self, address: Address) -> Value {
        self.outputs
            .get(&address)
            .copied()
            .unwrap_or_else(|| zero_of(address))
    }

    pub fn output_bool(&self, address: Address) -> bool {
        self.output(address).as_bool()
    }

    /// Every output the program has written, in address order.
    pub fn outputs(&self) -> impl Iterator<Item = (Address, Value)> + '_ {
        self.outputs.iter().map(|(a, v)| (*a, *v))
    }

    // ---- variables ------------------------------------------------------

    /// The value of a variable, by name; a block instance has none (read
    /// `block_value`).
    pub fn get(&self, name: &str) -> Option<Value> {
        match self.cells.get(self.program.slot(name)?)? {
            Cell::Val(_, v) => Some(*v),
            Cell::Block(_) => None,
        }
    }

    pub fn get_bool(&self, name: &str) -> bool {
        self.get(name).is_some_and(Value::as_bool)
    }

    /// Writes a variable from outside (a forced value, a test); `false`
    /// when there is no such variable or the value does not fit.
    pub fn set(&mut self, name: &str, value: Value) -> bool {
        let Some(slot) = self.program.slot(name) else {
            return false;
        };
        match &mut self.cells[slot] {
            Cell::Val(t, v) => match t.coerce(value) {
                Some(c) => {
                    *v = c;
                    true
                }
                None => false,
            },
            Cell::Block(_) => false,
        }
    }

    /// Every variable the author declared, for a watch table: its name, its
    /// type and its value as text (a block shows all its parameters).
    pub fn watch(&self) -> Vec<Watch> {
        self.program
            .vars
            .iter()
            .zip(&self.cells)
            .filter(|(d, _)| !d.hidden)
            .map(|(d, cell)| {
                let (ty, value) = match cell {
                    Cell::Val(t, v) => (t.name().to_owned(), v.to_string()),
                    Cell::Block(b) => (
                        b.kind.name().to_owned(),
                        b.kind
                            .inputs()
                            .iter()
                            .map(|(n, _)| *n)
                            .zip(&b.inputs)
                            .chain(b.kind.outputs().iter().map(|(n, _)| *n).zip(&b.outputs))
                            .map(|(n, v)| format!("{n}={v}"))
                            .collect::<Vec<_>>()
                            .join(" "),
                    ),
                };
                Watch {
                    name: d.name.clone(),
                    ty,
                    value,
                    address: d.address,
                }
            })
            .collect()
    }

    /// A parameter of a block instance: `block_value("t1", "ET")`.
    pub fn block_value(&self, name: &str, field: &str) -> Option<Value> {
        let Cell::Block(b) = self.cells.get(self.program.slot(name)?)? else {
            return None;
        };
        match b.kind.field(field)? {
            Field::In(i) => Some(b.inputs[i]),
            Field::Out(i) => Some(b.outputs[i]),
        }
    }

    /// Whether power flows out of ladder element `probe` as of the last scan.
    pub fn power(&self, probe: usize) -> bool {
        self.probes.get(probe).copied().unwrap_or(false)
    }

    // ---- the scan -------------------------------------------------------

    /// One scan lasting `dt_us` microseconds of controller time. On a fault
    /// the outputs keep what they had before the scan.
    pub fn scan(&mut self, dt_us: i64) -> Result<(), RuntimeError> {
        self.now_us += dt_us.max(0);
        self.steps = 0;
        let program = Arc::clone(&self.program);
        // Bound variables take the image values first.
        for (slot, v) in program.vars.iter().enumerate() {
            let Some(a) = v.address else { continue };
            let source = match a.space {
                Space::Input => Some(self.input(a)),
                Space::Memory => self.memory.get(&a).copied(),
                Space::Output => None,
            };
            if let (Some(value), Cell::Val(t, cell)) = (source, &mut self.cells[slot])
                && let Some(c) = t.coerce(value)
            {
                *cell = c;
            }
        }
        let saved_outputs = self.outputs.clone();
        let saved_memory = self.memory.clone();
        let result = self.run_block(&program.body);
        match result {
            Ok(_) => {
                for (slot, v) in program.vars.iter().enumerate() {
                    let Some(a) = v.address else { continue };
                    if let Cell::Val(_, value) = self.cells[slot] {
                        match a.space {
                            Space::Output => {
                                self.outputs.insert(a, image_value(a, value));
                            }
                            Space::Memory => {
                                self.memory.insert(a, image_value(a, value));
                            }
                            Space::Input => {}
                        }
                    }
                }
                self.scans += 1;
                Ok(())
            }
            Err(e) => {
                self.outputs = saved_outputs;
                self.memory = saved_memory;
                Err(e)
            }
        }
    }

    fn fault<T>(&self, line: u32, message: impl Into<String>) -> Result<T, RuntimeError> {
        Err(RuntimeError {
            line,
            message: message.into(),
        })
    }

    fn run_block(&mut self, stmts: &[Stmt]) -> Result<Flow, RuntimeError> {
        for s in stmts {
            match self.run(s)? {
                Flow::Next => {}
                other => return Ok(other),
            }
        }
        Ok(Flow::Next)
    }

    fn run(&mut self, s: &Stmt) -> Result<Flow, RuntimeError> {
        self.steps += 1;
        if self.steps > SCAN_BUDGET {
            return self.fault(s.line, "the scan took too long (watchdog)");
        }
        let line = s.line;
        match &s.kind {
            StmtKind::Assign(target, e) => {
                let v = self.eval(e, line)?;
                self.store(target, v, line)?;
            }
            StmtKind::If(arms, other) => {
                for (cond, body) in arms {
                    if self.eval(cond, line)?.as_bool() {
                        return self.run_block(body);
                    }
                }
                return self.run_block(other);
            }
            StmtKind::Case(sel, arms, other) => {
                let k = self.eval(sel, line)?.as_int();
                for (labels, body) in arms {
                    let hit = labels.iter().any(|l| match l {
                        CaseLabel::Value(v) => *v == k,
                        CaseLabel::Range(lo, hi) => (*lo..=*hi).contains(&k),
                    });
                    if hit {
                        return self.run_block(body);
                    }
                }
                return self.run_block(other);
            }
            StmtKind::For {
                var,
                from,
                to,
                by,
                body,
            } => {
                let mut i = self.eval(from, line)?.as_int();
                let end = self.eval(to, line)?.as_int();
                let step = match by {
                    Some(b) => self.eval(b, line)?.as_int(),
                    None => 1,
                };
                if step == 0 {
                    return self.fault(line, "FOR step is zero");
                }
                let mut rounds = 0u32;
                while (step > 0 && i <= end) || (step < 0 && i >= end) {
                    rounds += 1;
                    if rounds > LOOP_LIMIT {
                        return self.fault(line, "loop ran too long (watchdog)");
                    }
                    self.store(&Target::Var(*var), Value::Int(i), line)?;
                    match self.run_block(body)? {
                        Flow::Exit => break,
                        Flow::Return => return Ok(Flow::Return),
                        _ => {}
                    }
                    i += step;
                }
            }
            StmtKind::While(cond, body) => {
                let mut rounds = 0u32;
                while self.eval(cond, line)?.as_bool() {
                    rounds += 1;
                    if rounds > LOOP_LIMIT {
                        return self.fault(line, "loop ran too long (watchdog)");
                    }
                    match self.run_block(body)? {
                        Flow::Exit => break,
                        Flow::Return => return Ok(Flow::Return),
                        _ => {}
                    }
                }
            }
            StmtKind::Repeat(body, cond) => {
                let mut rounds = 0u32;
                loop {
                    rounds += 1;
                    if rounds > LOOP_LIMIT {
                        return self.fault(line, "loop ran too long (watchdog)");
                    }
                    match self.run_block(body)? {
                        Flow::Exit => break,
                        Flow::Return => return Ok(Flow::Return),
                        _ => {}
                    }
                    if self.eval(cond, line)?.as_bool() {
                        break;
                    }
                }
            }
            StmtKind::Exit => return Ok(Flow::Exit),
            StmtKind::Continue => return Ok(Flow::Continue),
            StmtKind::Return => return Ok(Flow::Return),
            StmtKind::CallFb { slot, args, outs } => {
                for (i, e) in args {
                    let v = self.eval(e, line)?;
                    let Cell::Block(b) = &mut self.cells[*slot] else {
                        return self.fault(line, "not a function block");
                    };
                    let ty = b.kind.inputs()[*i].1;
                    match ty.coerce(v) {
                        Some(c) => b.inputs[*i] = c,
                        None => return self.fault(line, format!("cannot pass {v} as {ty}")),
                    }
                }
                let now = self.now_us;
                let Cell::Block(b) = &mut self.cells[*slot] else {
                    return self.fault(line, "not a function block");
                };
                b.run(now);
                for (i, target) in outs {
                    let Cell::Block(b) = &self.cells[*slot] else {
                        continue;
                    };
                    let v = b.outputs[*i];
                    self.store(target, v, line)?;
                }
            }
            StmtKind::Probe(id, e) => {
                let on = self.eval(e, line)?.as_bool();
                self.probes[*id] = on;
            }
        }
        Ok(Flow::Next)
    }

    fn store(&mut self, target: &Target, v: Value, line: u32) -> Result<(), RuntimeError> {
        match target {
            Target::Var(i) => {
                let Cell::Val(t, cell) = &mut self.cells[*i] else {
                    return self.fault(line, "not a variable");
                };
                match ops::store(*t, v) {
                    Ok(c) => *cell = c,
                    Err(m) => return self.fault(line, m),
                }
            }
            Target::Direct(a) => {
                let v = image_value(*a, v);
                match a.space {
                    Space::Output => self.outputs.insert(*a, v),
                    Space::Memory => self.memory.insert(*a, v),
                    Space::Input => return self.fault(line, format!("{a} is an input")),
                };
            }
            Target::Member(slot, i) => {
                let Cell::Block(b) = &mut self.cells[*slot] else {
                    return self.fault(line, "not a function block");
                };
                let ty = b.kind.inputs()[*i].1;
                match ty.coerce(v) {
                    Some(c) => b.inputs[*i] = c,
                    None => return self.fault(line, format!("cannot store {v} in a {ty}")),
                }
            }
        }
        Ok(())
    }

    fn eval(&self, e: &Expr, line: u32) -> Result<Value, RuntimeError> {
        let wrap = |r: ops::OpResult| r.map_err(|message| RuntimeError { line, message });
        Ok(match e {
            Expr::Lit(v) => *v,
            Expr::Var(i) => match &self.cells[*i] {
                Cell::Val(_, v) => *v,
                Cell::Block(_) => return self.fault(line, "a function block has no value"),
            },
            Expr::Direct(a) => match a.space {
                Space::Input => self.input(*a),
                Space::Output => self.output(*a),
                Space::Memory => self.memory.get(a).copied().unwrap_or_else(|| zero_of(*a)),
            },
            Expr::Member(slot, field) => {
                let Cell::Block(b) = &self.cells[*slot] else {
                    return self.fault(line, "not a function block");
                };
                match field {
                    Field::In(i) => b.inputs[*i],
                    Field::Out(i) => b.outputs[*i],
                }
            }
            Expr::Unary(op, a) => wrap(ops::unary(*op, self.eval(a, line)?))?,
            Expr::Binary(op, a, b) => {
                let a = self.eval(a, line)?;
                let b = self.eval(b, line)?;
                wrap(ops::binary(*op, a, b))?
            }
            Expr::Call(f, args) => {
                let vals = args
                    .iter()
                    .map(|a| self.eval(a, line))
                    .collect::<Result<Vec<_>, _>>()?;
                wrap(ops::call(f, &vals))?
            }
        })
    }
}
