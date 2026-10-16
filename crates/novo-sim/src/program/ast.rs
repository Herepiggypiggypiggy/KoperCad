//! The compiled form of a program: names are already resolved to slots, so
//! a scan never looks anything up by text.

use super::address::Address;
use super::blocks::{FbKind, Field};
use super::value::{DataType, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Or,
    Xor,
    And,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
}

/// Built-in functions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Func {
    Abs,
    Sqrt,
    Ln,
    Log,
    Exp,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Min,
    Max,
    Limit,
    Sel,
    Mux,
    Expt,
    Trunc,
    Shl,
    Shr,
    Rol,
    Ror,
    /// `INT_TO_REAL` and friends: convert to the type.
    To(DataType),
}

impl Func {
    /// The function called `name`, with its allowed argument count range.
    pub fn parse(name: &str) -> Option<(Func, usize, usize)> {
        let upper = name.to_ascii_uppercase();
        Some(match upper.as_str() {
            "ABS" => (Func::Abs, 1, 1),
            "SQRT" => (Func::Sqrt, 1, 1),
            "LN" => (Func::Ln, 1, 1),
            "LOG" => (Func::Log, 1, 1),
            "EXP" => (Func::Exp, 1, 1),
            "SIN" => (Func::Sin, 1, 1),
            "COS" => (Func::Cos, 1, 1),
            "TAN" => (Func::Tan, 1, 1),
            "ASIN" => (Func::Asin, 1, 1),
            "ACOS" => (Func::Acos, 1, 1),
            "ATAN" => (Func::Atan, 1, 1),
            "MIN" => (Func::Min, 2, 16),
            "MAX" => (Func::Max, 2, 16),
            "LIMIT" => (Func::Limit, 3, 3),
            "SEL" => (Func::Sel, 3, 3),
            "MUX" => (Func::Mux, 3, 17),
            "EXPT" => (Func::Expt, 2, 2),
            "TRUNC" => (Func::Trunc, 1, 1),
            "SHL" => (Func::Shl, 2, 2),
            "SHR" => (Func::Shr, 2, 2),
            "ROL" => (Func::Rol, 2, 2),
            "ROR" => (Func::Ror, 2, 2),
            _ => {
                // X_TO_Y, or TO_Y
                let target = upper
                    .rsplit_once("_TO_")
                    .map(|(_, y)| y)
                    .or_else(|| upper.strip_prefix("TO_"))?;
                (Func::To(DataType::parse(target)?), 1, 1)
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Lit(Value),
    Var(usize),
    Direct(Address),
    /// A parameter of a block instance: `t1.Q`.
    Member(usize, Field),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Call(Func, Vec<Expr>),
}

/// What an assignment writes.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    Var(usize),
    Direct(Address),
    Member(usize, usize),
}

#[derive(Debug, Clone, PartialEq)]
pub enum CaseLabel {
    Value(i64),
    Range(i64, i64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub line: u32,
    pub kind: StmtKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    Assign(Target, Expr),
    If(Vec<(Expr, Vec<Stmt>)>, Vec<Stmt>),
    Case(Expr, Vec<(Vec<CaseLabel>, Vec<Stmt>)>, Vec<Stmt>),
    For {
        var: usize,
        from: Expr,
        to: Expr,
        by: Option<Expr>,
        body: Vec<Stmt>,
    },
    While(Expr, Vec<Stmt>),
    Repeat(Vec<Stmt>, Expr),
    Exit,
    Continue,
    Return,
    /// Calls block instance `slot` with `(input index, value)` arguments;
    /// each `=> target` output connection is an extra assignment after.
    CallFb {
        slot: usize,
        args: Vec<(usize, Expr)>,
        outs: Vec<(usize, Target)>,
    },
    /// Records the power flow at ladder element `id` (for the monitor).
    Probe(usize, Expr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarClass {
    Input,
    Output,
    Local,
    Global,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VarType {
    Elem(DataType),
    Block(FbKind),
}

#[derive(Debug, Clone, PartialEq)]
pub struct VarDecl {
    pub name: String,
    pub ty: VarType,
    pub class: VarClass,
    pub address: Option<Address>,
    pub init: Value,
    /// Initial values of block inputs: `t1 : TON := (PT := T#5s)`.
    pub block_init: Vec<(usize, Value)>,
    pub constant: bool,
    /// Declared by the compiler (ladder edge detectors), not by the user.
    pub hidden: bool,
    pub line: u32,
}
