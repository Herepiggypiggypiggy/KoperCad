//! A compiled program, and the ladder rungs kept for display.

use super::ast::{Stmt, VarDecl};

/// One piece of a rung, as the ladder monitor draws it.
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    /// Index into the probe list: [`Runtime::power`](super::Runtime::power)
    /// tells whether power flows out of this element.
    pub probe: usize,
    pub kind: ElementKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ElementKind {
    /// `[x]`, `[/x]`, `[P x]`, `[N x]`, `[count >= 5]`.
    Contact { text: String },
    /// `(x)`, `(/x)`, `(S x)`, `(R x)`, `(x := expr)`.
    Coil { text: String },
    /// `<t1: TON, PT := T#5s>`.
    Box { text: String },
    /// `{ a | b }`: each branch is a series of elements.
    Branch(Vec<Vec<Element>>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rung {
    pub title: String,
    pub line: u32,
    pub elements: Vec<Element>,
}

/// A parsed program: declarations, statements in source order (ladder rungs
/// compiled into statements among them), and the rungs for display.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub name: String,
    pub vars: Vec<VarDecl>,
    pub body: Vec<Stmt>,
    pub rungs: Vec<Rung>,
    pub probes: usize,
    /// Every direct address the program names, declared or used inline.
    pub addresses: std::collections::BTreeSet<super::address::Address>,
}

impl Program {
    /// Reads a program written in Structured Text and/or the ladder notation.
    pub fn parse(source: &str) -> Result<Program, super::ParseError> {
        super::parser::parse(source)
    }

    /// The slot of a variable by name (case-insensitive).
    pub fn slot(&self, name: &str) -> Option<usize> {
        self.vars
            .iter()
            .position(|v| v.name.eq_ignore_ascii_case(name))
    }
}
