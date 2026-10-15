//! Structured Text and ladder notation to a [`Program`].
//!
//! Ladder is written as text, one rung per `RUNG`:
//!
//! ```text
//! RUNG "Seal-in"
//!   { [start] | [run] } -- [/stop] -- [/overload] -- (run);
//! ```
//!
//! `[x]` is a normally open contact, `[/x]` a normally closed one, `[P x]`
//! and `[N x]` rising and falling edge contacts, and `[count >= 5]` any
//! BOOL expression. `(x)` is an output coil, `(/x)` a negated one, `(S x)`
//! sets, `(R x)` resets, `(P x)` pulses for one scan, and `(x := expr)`
//! computes while power flows. `<t1: TON, PT := T#5s>` is a function block
//! box; power flow enters its first input and leaves by its first output.
//! `{ a | b }` is a parallel branch. Rungs compile to the same statements
//! Structured Text does, and can sit between ST statements.

use std::collections::{BTreeSet, HashMap};

use super::address::{Address, Size, Space};
use super::ast::*;
use super::blocks::{FbKind, Field};
use super::lexer::{ParseError, Tok, Token, lex};
use super::model::{Element, ElementKind, Program, Rung};
use super::ops;
use super::value::{DataType, Value};

type Res<T> = Result<T, ParseError>;

pub fn parse(source: &str) -> Res<Program> {
    let toks = lex(source)?;
    let mut p = Parser {
        src: source,
        toks,
        pos: 0,
        vars: Vec::new(),
        names: HashMap::new(),
        probes: 0,
        rungs: Vec::new(),
        no_gt: false,
        flow: None,
        scratch: Vec::new(),
        hidden_count: 0,
        addresses: BTreeSet::new(),
    };
    p.program()
}

struct Parser<'a> {
    src: &'a str,
    toks: Vec<Token>,
    pos: usize,
    vars: Vec<VarDecl>,
    names: HashMap<String, usize>,
    probes: usize,
    rungs: Vec<Rung>,
    /// Inside a ladder box `<...>` a bare `>` closes it.
    no_gt: bool,
    flow: Option<usize>,
    scratch: Vec<(usize, usize)>,
    hidden_count: usize,
    addresses: BTreeSet<Address>,
}

const BLOCK_END: &[&str] = &[
    "END_PROGRAM",
    "END_IF",
    "ELSIF",
    "ELSE",
    "END_CASE",
    "END_FOR",
    "END_WHILE",
    "END_REPEAT",
    "UNTIL",
];

impl Parser<'_> {
    // ---- token helpers ----------------------------------------------

    fn tok(&self) -> &Tok {
        &self.toks[self.pos].tok
    }

    fn tok_at(&self, n: usize) -> &Tok {
        let i = (self.pos + n).min(self.toks.len() - 1);
        &self.toks[i].tok
    }

    fn line(&self) -> u32 {
        self.toks[self.pos].line
    }

    fn err<T>(&self, message: impl Into<String>) -> Res<T> {
        let t = &self.toks[self.pos];
        Err(ParseError::new(t.line, t.col, message))
    }

    fn advance(&mut self) {
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
    }

    fn accept(&mut self, tok: &Tok) -> bool {
        if self.tok() == tok {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, tok: &Tok) -> Res<()> {
        if self.accept(tok) {
            Ok(())
        } else {
            self.err(format!("expected {tok}, found {}", self.tok()))
        }
    }

    fn is_kw(&self, kw: &str) -> bool {
        matches!(self.tok(), Tok::Ident(s) if s.eq_ignore_ascii_case(kw))
    }

    fn accept_kw(&mut self, kw: &str) -> bool {
        if self.is_kw(kw) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect_kw(&mut self, kw: &str) -> Res<()> {
        if self.accept_kw(kw) {
            Ok(())
        } else {
            self.err(format!("expected {kw}, found {}", self.tok()))
        }
    }

    fn ident(&mut self, what: &str) -> Res<String> {
        if let Tok::Ident(s) = self.tok() {
            let s = s.clone();
            self.advance();
            Ok(s)
        } else {
            self.err(format!("expected {what}, found {}", self.tok()))
        }
    }

    fn at_block_end(&self) -> bool {
        matches!(self.tok(), Tok::Eof) || BLOCK_END.iter().any(|kw| self.is_kw(kw))
    }

    /// The source text of tokens `from..to` (exclusive of `to`).
    fn text(&self, from: usize, to: usize) -> String {
        if to <= from {
            return String::new();
        }
        self.src[self.toks[from].start..self.toks[to - 1].end]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    // ---- program and declarations -----------------------------------

    fn program(&mut self) -> Res<Program> {
        let mut name = String::from("Program");
        if self.accept_kw("PROGRAM") {
            name = self.ident("a program name")?;
        }
        while self.is_var_block() {
            self.var_block()?;
        }
        let body = self.statements()?;
        if self.is_kw("END_PROGRAM") {
            self.advance();
            self.accept(&Tok::Semi);
        }
        if *self.tok() != Tok::Eof {
            return self.err(format!("unexpected {}", self.tok()));
        }
        Ok(Program {
            name,
            vars: std::mem::take(&mut self.vars),
            body,
            rungs: std::mem::take(&mut self.rungs),
            probes: self.probes,
            addresses: std::mem::take(&mut self.addresses),
        })
    }

    fn is_var_block(&self) -> bool {
        ["VAR", "VAR_INPUT", "VAR_OUTPUT", "VAR_GLOBAL", "VAR_TEMP"]
            .iter()
            .any(|kw| self.is_kw(kw))
    }

    fn var_block(&mut self) -> Res<()> {
        let Tok::Ident(kw) = self.tok().clone() else {
            unreachable!()
        };
        self.advance();
        let class = match kw.to_ascii_uppercase().as_str() {
            "VAR_INPUT" => VarClass::Input,
            "VAR_OUTPUT" => VarClass::Output,
            "VAR_GLOBAL" => VarClass::Global,
            _ => VarClass::Local,
        };
        let mut constant = false;
        while self.is_kw("CONSTANT") || self.is_kw("RETAIN") || self.is_kw("PERSISTENT") {
            constant |= self.is_kw("CONSTANT");
            self.advance();
        }
        while !self.is_kw("END_VAR") {
            if *self.tok() == Tok::Eof {
                return self.err("VAR block is never closed with END_VAR");
            }
            self.declaration(class, constant)?;
        }
        self.advance();
        self.accept(&Tok::Semi);
        Ok(())
    }

    fn declaration(&mut self, class: VarClass, constant: bool) -> Res<()> {
        let line = self.line();
        let mut names = vec![(
            self.line(),
            self.toks[self.pos].col,
            self.ident("a variable name")?,
        )];
        while self.accept(&Tok::Comma) {
            names.push((
                self.line(),
                self.toks[self.pos].col,
                self.ident("a variable name")?,
            ));
        }
        let mut address = None;
        if self.accept_kw("AT") {
            if names.len() > 1 {
                return self.err("AT can only follow a single variable name");
            }
            let Tok::Addr(a) = self.tok().clone() else {
                return self.err(format!(
                    "expected an address after AT, found {}",
                    self.tok()
                ));
            };
            self.advance();
            self.addresses.insert(a);
            address = Some(a);
        }
        self.expect(&Tok::Colon)?;
        let type_name = self.ident("a type")?;
        let ty = if let Some(t) = DataType::parse(&type_name) {
            VarType::Elem(t)
        } else if let Some(k) = FbKind::parse(&type_name) {
            VarType::Block(k)
        } else {
            return self.err(format!(
                "unknown type `{type_name}` (supported: BOOL, SINT..LINT, USINT..ULINT, BYTE, WORD, DWORD, REAL, LREAL, TIME, TON, TOF, TP, CTU, CTD, CTUD, R_TRIG, F_TRIG, RS, SR)"
            ));
        };
        if let (Some(a), VarType::Elem(t)) = (address, &ty) {
            let bit_ok = a.is_bit() == (*t == DataType::Bool);
            if !bit_ok {
                return self.err(format!(
                    "{a} is a {} address, so the variable cannot be {t}",
                    if a.is_bit() { "bit" } else { "word" }
                ));
            }
            if a.size != Size::Bit && !t.is_integer() {
                return self.err(format!("{a} holds an integer, not {t}"));
            }
        } else if address.is_some() {
            return self.err("a function block cannot sit at an address");
        }
        let mut init = match &ty {
            VarType::Elem(t) => t.zero(),
            VarType::Block(_) => Value::Bool(false),
        };
        let mut block_init = Vec::new();
        if self.accept(&Tok::Assign) {
            match &ty {
                VarType::Elem(t) => {
                    let e = self.expr()?;
                    let v = self.fold(&e)?;
                    init = ops::store(*t, v).or_else(|m| self.err(m))?;
                }
                VarType::Block(kind) => {
                    self.expect(&Tok::LParen)?;
                    while *self.tok() != Tok::RParen {
                        let pname = self.ident("a parameter name")?;
                        let Some(Field::In(i)) = kind.field(&pname) else {
                            return self.err(format!(
                                "{kind:?} has no input `{pname}`",
                                kind = kind.name()
                            ));
                        };
                        self.expect(&Tok::Assign)?;
                        let e = self.expr()?;
                        let v = self.fold(&e)?;
                        let t = kind.inputs()[i].1;
                        block_init.push((i, ops::store(t, v).or_else(|m| self.err(m))?));
                        if !self.accept(&Tok::Comma) {
                            break;
                        }
                    }
                    self.expect(&Tok::RParen)?;
                }
            }
        }
        self.expect(&Tok::Semi)?;
        for (l, c, name) in names {
            if self.names.contains_key(&name.to_ascii_lowercase()) {
                return Err(ParseError::new(l, c, format!("`{name}` is declared twice")));
            }
            self.names
                .insert(name.to_ascii_lowercase(), self.vars.len());
            self.vars.push(VarDecl {
                name,
                ty: ty.clone(),
                class,
                address,
                init,
                block_init: block_init.clone(),
                constant,
                hidden: false,
                line,
            });
        }
        Ok(())
    }

    /// Declares a compiler-made variable.
    fn hidden(&mut self, ty: VarType) -> usize {
        self.hidden_count += 1;
        let init = match &ty {
            VarType::Elem(t) => t.zero(),
            VarType::Block(_) => Value::Bool(false),
        };
        self.vars.push(VarDecl {
            name: format!("__{}", self.hidden_count),
            ty,
            class: VarClass::Local,
            address: None,
            init,
            block_init: Vec::new(),
            constant: false,
            hidden: true,
            line: 0,
        });
        self.vars.len() - 1
    }

    /// Evaluates a constant expression (initial values).
    fn fold(&self, e: &Expr) -> Res<Value> {
        fn go(vars: &[VarDecl], e: &Expr) -> Result<Value, String> {
            match e {
                Expr::Lit(v) => Ok(*v),
                Expr::Var(i) if vars[*i].constant => Ok(vars[*i].init),
                Expr::Unary(op, a) => ops::unary(*op, go(vars, a)?),
                Expr::Binary(op, a, b) => ops::binary(*op, go(vars, a)?, go(vars, b)?),
                Expr::Call(f, args) => {
                    let vals = args
                        .iter()
                        .map(|a| go(vars, a))
                        .collect::<Result<Vec<_>, _>>()?;
                    ops::call(f, &vals)
                }
                _ => Err("an initial value must be a constant".into()),
            }
        }
        go(&self.vars, e).or_else(|m| self.err(m))
    }

    // ---- statements ---------------------------------------------------

    fn statements(&mut self) -> Res<Vec<Stmt>> {
        let mut out = Vec::new();
        while !self.at_block_end() {
            self.statement(&mut out)?;
        }
        Ok(out)
    }

    fn stmt(&self, line: u32, kind: StmtKind) -> Stmt {
        Stmt { line, kind }
    }

    fn statement(&mut self, out: &mut Vec<Stmt>) -> Res<()> {
        let line = self.line();
        if self.accept(&Tok::Semi) {
            return Ok(());
        }
        let Tok::Ident(word) = self.tok().clone() else {
            if let Tok::Addr(_) = self.tok() {
                return self.assignment(out);
            }
            return self.err(format!("expected a statement, found {}", self.tok()));
        };
        match word.to_ascii_uppercase().as_str() {
            "IF" => {
                self.advance();
                let mut arms = Vec::new();
                let cond = self.expr()?;
                self.expect_kw("THEN")?;
                arms.push((cond, self.statements()?));
                let mut other = Vec::new();
                loop {
                    if self.accept_kw("ELSIF") {
                        let c = self.expr()?;
                        self.expect_kw("THEN")?;
                        arms.push((c, self.statements()?));
                    } else if self.accept_kw("ELSE") {
                        other = self.statements()?;
                    } else {
                        break;
                    }
                }
                self.expect_kw("END_IF")?;
                self.accept(&Tok::Semi);
                out.push(self.stmt(line, StmtKind::If(arms, other)));
            }
            "CASE" => {
                self.advance();
                let sel = self.expr()?;
                self.expect_kw("OF")?;
                let mut arms = Vec::new();
                let mut other = Vec::new();
                loop {
                    if self.accept_kw("ELSE") {
                        other = self.statements()?;
                        break;
                    }
                    if self.is_kw("END_CASE") {
                        break;
                    }
                    let mut labels = Vec::new();
                    loop {
                        let lo = self.case_int()?;
                        if self.accept(&Tok::DotDot) {
                            let hi = self.case_int()?;
                            labels.push(CaseLabel::Range(lo, hi));
                        } else {
                            labels.push(CaseLabel::Value(lo));
                        }
                        if !self.accept(&Tok::Comma) {
                            break;
                        }
                    }
                    self.expect(&Tok::Colon)?;
                    let mut body = Vec::new();
                    while !matches!(self.tok(), Tok::Int(_) | Tok::Minus) && !self.at_block_end() {
                        self.statement(&mut body)?;
                    }
                    arms.push((labels, body));
                }
                self.expect_kw("END_CASE")?;
                self.accept(&Tok::Semi);
                out.push(self.stmt(line, StmtKind::Case(sel, arms, other)));
            }
            "FOR" => {
                self.advance();
                let name = self.ident("a loop variable")?;
                let var = self.lookup(&name)?;
                if !matches!(&self.vars[var].ty, VarType::Elem(t) if t.is_integer()) {
                    return self.err(format!("loop variable `{name}` must be an integer"));
                }
                self.expect(&Tok::Assign)?;
                let from = self.expr()?;
                self.expect_kw("TO")?;
                let to = self.expr()?;
                let by = if self.accept_kw("BY") {
                    Some(self.expr()?)
                } else {
                    None
                };
                self.expect_kw("DO")?;
                let body = self.statements()?;
                self.expect_kw("END_FOR")?;
                self.accept(&Tok::Semi);
                out.push(self.stmt(
                    line,
                    StmtKind::For {
                        var,
                        from,
                        to,
                        by,
                        body,
                    },
                ));
            }
            "WHILE" => {
                self.advance();
                let cond = self.expr()?;
                self.expect_kw("DO")?;
                let body = self.statements()?;
                self.expect_kw("END_WHILE")?;
                self.accept(&Tok::Semi);
                out.push(self.stmt(line, StmtKind::While(cond, body)));
            }
            "REPEAT" => {
                self.advance();
                let body = self.statements()?;
                self.expect_kw("UNTIL")?;
                let cond = self.expr()?;
                self.expect_kw("END_REPEAT")?;
                self.accept(&Tok::Semi);
                out.push(self.stmt(line, StmtKind::Repeat(body, cond)));
            }
            "EXIT" | "CONTINUE" | "RETURN" => {
                self.advance();
                self.expect(&Tok::Semi)?;
                let kind = match word.to_ascii_uppercase().as_str() {
                    "EXIT" => StmtKind::Exit,
                    "CONTINUE" => StmtKind::Continue,
                    _ => StmtKind::Return,
                };
                out.push(self.stmt(line, kind));
            }
            "RUNG" => return self.rung(out),
            _ => return self.assignment_or_call(out),
        }
        Ok(())
    }

    fn case_int(&mut self) -> Res<i64> {
        let neg = self.accept(&Tok::Minus);
        let Tok::Int(i) = self.tok().clone() else {
            return self.err(format!("expected a whole number, found {}", self.tok()));
        };
        self.advance();
        Ok(if neg { -i } else { i })
    }

    fn lookup(&self, name: &str) -> Res<usize> {
        match self.names.get(&name.to_ascii_lowercase()) {
            Some(&i) => Ok(i),
            None => self.err(format!("`{name}` is not declared")),
        }
    }

    /// `x`, `t1.IN` or `%QX0.0` as something to write to.
    fn target(&mut self) -> Res<Target> {
        match self.tok().clone() {
            Tok::Addr(a) => {
                if a.space == Space::Input {
                    return self.err(format!("{a} is an input and cannot be written"));
                }
                self.advance();
                self.addresses.insert(a);
                Ok(Target::Direct(a))
            }
            Tok::Ident(name) => {
                self.advance();
                let slot = self.lookup(&name)?;
                if self.accept(&Tok::Dot) {
                    let VarType::Block(kind) = self.vars[slot].ty else {
                        return self.err(format!("`{name}` is not a function block"));
                    };
                    let field = self.ident("a parameter name")?;
                    match kind.field(&field) {
                        Some(Field::In(i)) => Ok(Target::Member(slot, i)),
                        _ => self.err(format!(
                            "`{field}` is not an input of {}; only inputs can be written",
                            kind.name()
                        )),
                    }
                } else {
                    if self.vars[slot].constant {
                        return self.err(format!("`{name}` is a constant"));
                    }
                    if matches!(self.vars[slot].ty, VarType::Block(_)) {
                        return self.err(format!(
                            "`{name}` is a function block; call it or write one of its inputs"
                        ));
                    }
                    if matches!(self.vars[slot].address, Some(a) if a.space == Space::Input) {
                        return self.err(format!(
                            "`{name}` is bound to an input and cannot be written"
                        ));
                    }
                    Ok(Target::Var(slot))
                }
            }
            other => self.err(format!("expected a variable, found {other}")),
        }
    }

    fn assignment(&mut self, out: &mut Vec<Stmt>) -> Res<()> {
        let line = self.line();
        let target = self.target()?;
        self.expect(&Tok::Assign)?;
        let e = self.expr()?;
        self.expect(&Tok::Semi)?;
        out.push(self.stmt(line, StmtKind::Assign(target, e)));
        Ok(())
    }

    fn assignment_or_call(&mut self, out: &mut Vec<Stmt>) -> Res<()> {
        // `name (` on a block instance is a call.
        if let Tok::Ident(name) = self.tok().clone()
            && *self.tok_at(1) == Tok::LParen
        {
            let line = self.line();
            let slot = self.lookup(&name)?;
            let VarType::Block(kind) = self.vars[slot].ty else {
                return self.err(format!("`{name}` is not a function block"));
            };
            self.advance();
            self.advance();
            let mut args = Vec::new();
            let mut outs = Vec::new();
            while *self.tok() != Tok::RParen {
                let pname = self.ident("a parameter name")?;
                if self.accept(&Tok::Assign) {
                    let Some(Field::In(i)) = kind.field(&pname) else {
                        return self.err(format!("{} has no input `{pname}`", kind.name()));
                    };
                    args.push((i, self.expr()?));
                } else if self.accept(&Tok::Arrow) {
                    let Some(Field::Out(i)) = kind.field(&pname) else {
                        return self.err(format!("{} has no output `{pname}`", kind.name()));
                    };
                    outs.push((i, self.target()?));
                } else {
                    return self.err(format!("expected `:=` or `=>` after `{pname}`"));
                }
                if !self.accept(&Tok::Comma) {
                    break;
                }
            }
            self.expect(&Tok::RParen)?;
            self.expect(&Tok::Semi)?;
            out.push(self.stmt(line, StmtKind::CallFb { slot, args, outs }));
            return Ok(());
        }
        self.assignment(out)
    }

    // ---- expressions --------------------------------------------------

    fn expr(&mut self) -> Res<Expr> {
        self.binary_level(0)
    }

    /// Binary operator levels from loosest to tightest.
    fn op_at(&self, level: usize) -> Option<BinOp> {
        let t = self.tok();
        let kw = |s: &str| matches!(t, Tok::Ident(w) if w.eq_ignore_ascii_case(s));
        match level {
            0 => kw("OR").then_some(BinOp::Or),
            1 => kw("XOR").then_some(BinOp::Xor),
            2 => (kw("AND") || *t == Tok::Amp).then_some(BinOp::And),
            3 => match t {
                Tok::Eq => Some(BinOp::Eq),
                Tok::Ne => Some(BinOp::Ne),
                _ => None,
            },
            4 => match t {
                Tok::Lt => Some(BinOp::Lt),
                Tok::Gt if !self.no_gt => Some(BinOp::Gt),
                Tok::Le => Some(BinOp::Le),
                Tok::Ge => Some(BinOp::Ge),
                _ => None,
            },
            5 => match t {
                Tok::Plus => Some(BinOp::Add),
                Tok::Minus => Some(BinOp::Sub),
                _ => None,
            },
            6 => match t {
                Tok::Star => Some(BinOp::Mul),
                Tok::Slash => Some(BinOp::Div),
                _ if kw("MOD") => Some(BinOp::Mod),
                _ => None,
            },
            _ => None,
        }
    }

    fn binary_level(&mut self, level: usize) -> Res<Expr> {
        if level > 6 {
            return self.unary();
        }
        let mut left = self.binary_level(level + 1)?;
        while let Some(op) = self.op_at(level) {
            self.advance();
            let right = self.binary_level(level + 1)?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn unary(&mut self) -> Res<Expr> {
        if self.accept(&Tok::Minus) {
            let e = self.unary()?;
            return Ok(match e {
                Expr::Lit(Value::Int(i)) => Expr::Lit(Value::Int(-i)),
                Expr::Lit(Value::Real(r)) => Expr::Lit(Value::Real(-r)),
                Expr::Lit(Value::Time(t)) => Expr::Lit(Value::Time(-t)),
                e => Expr::Unary(UnOp::Neg, Box::new(e)),
            });
        }
        if self.accept(&Tok::Plus) {
            return self.unary();
        }
        if self.accept_kw("NOT") {
            return Ok(Expr::Unary(UnOp::Not, Box::new(self.unary()?)));
        }
        self.power()
    }

    fn power(&mut self) -> Res<Expr> {
        let mut left = self.primary()?;
        while self.accept(&Tok::StarStar) {
            let right = self.primary()?;
            left = Expr::Binary(BinOp::Pow, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn primary(&mut self) -> Res<Expr> {
        match self.tok().clone() {
            Tok::Int(i) => {
                self.advance();
                Ok(Expr::Lit(Value::Int(i)))
            }
            Tok::Real(r) => {
                self.advance();
                Ok(Expr::Lit(Value::Real(r)))
            }
            Tok::Time(t) => {
                self.advance();
                Ok(Expr::Lit(Value::Time(t)))
            }
            Tok::Addr(a) => {
                self.advance();
                self.addresses.insert(a);
                Ok(Expr::Direct(a))
            }
            Tok::LParen => {
                self.advance();
                let saved = std::mem::replace(&mut self.no_gt, false);
                let e = self.expr()?;
                self.no_gt = saved;
                self.expect(&Tok::RParen)?;
                Ok(e)
            }
            Tok::Ident(name) => {
                let upper = name.to_ascii_uppercase();
                if upper == "TRUE" || upper == "FALSE" {
                    self.advance();
                    return Ok(Expr::Lit(Value::Bool(upper == "TRUE")));
                }
                self.advance();
                if let Some(&slot) = self.names.get(&name.to_ascii_lowercase()) {
                    if self.accept(&Tok::Dot) {
                        let VarType::Block(kind) = self.vars[slot].ty else {
                            return self.err(format!("`{name}` is not a function block"));
                        };
                        let field = self.ident("a parameter name")?;
                        return match kind.field(&field) {
                            Some(f) => Ok(Expr::Member(slot, f)),
                            None => self.err(format!("{} has no parameter `{field}`", kind.name())),
                        };
                    }
                    if matches!(self.vars[slot].ty, VarType::Block(_)) {
                        return self.err(format!(
                            "`{name}` is a function block; read one of its outputs (`{name}.Q`)"
                        ));
                    }
                    return Ok(Expr::Var(slot));
                }
                if *self.tok() == Tok::LParen
                    && let Some((func, min, max)) = Func::parse(&name)
                {
                    self.advance();
                    let saved = std::mem::replace(&mut self.no_gt, false);
                    let mut args = Vec::new();
                    while *self.tok() != Tok::RParen {
                        args.push(self.expr()?);
                        if !self.accept(&Tok::Comma) {
                            break;
                        }
                    }
                    self.no_gt = saved;
                    self.expect(&Tok::RParen)?;
                    if args.len() < min || args.len() > max {
                        return self.err(format!(
                            "{upper} takes {} argument(s), found {}",
                            if min == max {
                                min.to_string()
                            } else {
                                format!("{min} to {max}")
                            },
                            args.len()
                        ));
                    }
                    return Ok(Expr::Call(func, args));
                }
                self.pos -= 1;
                self.err(format!("`{name}` is not declared"))
            }
            other => self.err(format!("expected a value, found {other}")),
        }
    }

    // ---- ladder -------------------------------------------------------

    fn flow_var(&mut self) -> usize {
        if let Some(f) = self.flow {
            return f;
        }
        let f = self.hidden(VarType::Elem(DataType::Bool));
        self.flow = Some(f);
        f
    }

    fn scratch(&mut self, depth: usize) -> (usize, usize) {
        while self.scratch.len() <= depth {
            let save = self.hidden(VarType::Elem(DataType::Bool));
            let acc = self.hidden(VarType::Elem(DataType::Bool));
            self.scratch.push((save, acc));
        }
        self.scratch[depth]
    }

    fn probe(&mut self) -> usize {
        self.probes += 1;
        self.probes - 1
    }

    fn rung(&mut self, out: &mut Vec<Stmt>) -> Res<()> {
        let line = self.line();
        self.advance(); // RUNG
        let title = if let Tok::Text(t) = self.tok().clone() {
            self.advance();
            t
        } else {
            format!("Rung {}", self.rungs.len() + 1)
        };
        let flow = self.flow_var();
        out.push(self.stmt(
            line,
            StmtKind::Assign(Target::Var(flow), Expr::Lit(Value::Bool(true))),
        ));
        let elements = self.series(flow, 0, out)?;
        self.expect(&Tok::Semi)?;
        self.rungs.push(Rung {
            title,
            line,
            elements,
        });
        Ok(())
    }

    fn series(&mut self, flow: usize, depth: usize, out: &mut Vec<Stmt>) -> Res<Vec<Element>> {
        let mut elements = Vec::new();
        loop {
            while self.accept(&Tok::Minus) {}
            let line = self.line();
            let start = self.pos;
            let kind = match self.tok() {
                Tok::LBracket => self.contact(flow, out)?,
                Tok::LParen => self.coil(flow, out)?,
                Tok::Lt => self.fb_box(flow, out)?,
                Tok::LBrace => self.branch(flow, depth, out)?,
                _ => break,
            };
            let kind = match kind {
                ElementKind::Contact { .. } => ElementKind::Contact {
                    text: self.text(start + 1, self.pos - 1),
                },
                ElementKind::Coil { .. } => ElementKind::Coil {
                    text: self.text(start + 1, self.pos - 1),
                },
                ElementKind::Box { .. } => ElementKind::Box {
                    text: self.text(start + 1, self.pos - 1),
                },
                other => other,
            };
            let probe = self.probe();
            out.push(self.stmt(line, StmtKind::Probe(probe, Expr::Var(flow))));
            elements.push(Element { probe, kind });
        }
        Ok(elements)
    }

    fn and_flow(&self, flow: usize, e: Expr) -> StmtKind {
        StmtKind::Assign(
            Target::Var(flow),
            Expr::Binary(BinOp::And, Box::new(Expr::Var(flow)), Box::new(e)),
        )
    }

    fn contact(&mut self, flow: usize, out: &mut Vec<Stmt>) -> Res<ElementKind> {
        let line = self.line();
        self.advance(); // [
        let negated = self.accept(&Tok::Slash);
        let edge = match (self.tok(), self.tok_at(1)) {
            (Tok::Ident(w), Tok::Ident(_) | Tok::Addr(_))
                if !negated && (w.eq_ignore_ascii_case("P") || w.eq_ignore_ascii_case("N")) =>
            {
                Some(w.eq_ignore_ascii_case("P"))
            }
            _ => None,
        };
        if edge.is_some() {
            self.advance();
        }
        let saved = std::mem::replace(&mut self.no_gt, false);
        let e = self.expr()?;
        self.no_gt = saved;
        self.expect(&Tok::RBracket)?;
        if let Some(rising) = edge {
            let kind = if rising { FbKind::RTrig } else { FbKind::FTrig };
            let slot = self.hidden(VarType::Block(kind));
            out.push(self.stmt(
                line,
                StmtKind::CallFb {
                    slot,
                    args: vec![(0, e)],
                    outs: Vec::new(),
                },
            ));
            let k = self.and_flow(flow, Expr::Member(slot, Field::Out(0)));
            out.push(self.stmt(line, k));
        } else {
            let e = if negated {
                Expr::Unary(UnOp::Not, Box::new(e))
            } else {
                e
            };
            let k = self.and_flow(flow, e);
            out.push(self.stmt(line, k));
        }
        Ok(ElementKind::Contact {
            text: String::new(),
        })
    }

    fn coil(&mut self, flow: usize, out: &mut Vec<Stmt>) -> Res<ElementKind> {
        let line = self.line();
        self.advance(); // (
        let negated = self.accept(&Tok::Slash);
        let modifier = match (self.tok(), self.tok_at(1)) {
            (Tok::Ident(w), Tok::Ident(_) | Tok::Addr(_)) if !negated => {
                match w.to_ascii_uppercase().as_str() {
                    m @ ("S" | "R" | "P" | "N") => Some(m.chars().next().unwrap_or('S')),
                    _ => None,
                }
            }
            _ => None,
        };
        if modifier.is_some() {
            self.advance();
        }
        let target = self.target()?;
        if self.accept(&Tok::Assign) {
            if negated || modifier.is_some() {
                return self.err("an assignment coil `(x := expr)` takes no modifier");
            }
            let e = self.expr()?;
            self.expect(&Tok::RParen)?;
            out.push(self.stmt(
                line,
                StmtKind::If(
                    vec![(
                        Expr::Var(flow),
                        vec![self.stmt(line, StmtKind::Assign(target, e))],
                    )],
                    Vec::new(),
                ),
            ));
            return Ok(ElementKind::Coil {
                text: String::new(),
            });
        }
        self.expect(&Tok::RParen)?;
        // A coil drives a BOOL.
        let is_bool = match &target {
            Target::Var(i) => matches!(self.vars[*i].ty, VarType::Elem(DataType::Bool)),
            Target::Direct(a) => a.is_bit(),
            Target::Member(slot, i) => {
                let VarType::Block(k) = self.vars[*slot].ty else {
                    return self.err("not a function block");
                };
                k.inputs()[*i].1 == DataType::Bool
            }
        };
        if !is_bool {
            return self.err("a coil needs a BOOL; use `(x := expr)` to compute a number");
        }
        let truth = |b| Expr::Lit(Value::Bool(b));
        let assign = |p: &Self, t: Target, e: Expr| p.stmt(line, StmtKind::Assign(t, e));
        match modifier {
            None => {
                let e = if negated {
                    Expr::Unary(UnOp::Not, Box::new(Expr::Var(flow)))
                } else {
                    Expr::Var(flow)
                };
                out.push(assign(self, target, e));
            }
            Some(m @ ('S' | 'R')) => {
                let value = m == 'S';
                out.push(self.stmt(
                    line,
                    StmtKind::If(
                        vec![(Expr::Var(flow), vec![assign(self, target, truth(value))])],
                        Vec::new(),
                    ),
                ));
            }
            Some(m) => {
                let kind = if m == 'P' {
                    FbKind::RTrig
                } else {
                    FbKind::FTrig
                };
                let slot = self.hidden(VarType::Block(kind));
                out.push(self.stmt(
                    line,
                    StmtKind::CallFb {
                        slot,
                        args: vec![(0, Expr::Var(flow))],
                        outs: Vec::new(),
                    },
                ));
                out.push(assign(self, target, Expr::Member(slot, Field::Out(0))));
            }
        }
        Ok(ElementKind::Coil {
            text: String::new(),
        })
    }

    fn fb_box(&mut self, flow: usize, out: &mut Vec<Stmt>) -> Res<ElementKind> {
        let line = self.line();
        self.advance(); // <
        let name = self.ident("an instance name")?;
        self.expect(&Tok::Colon)?;
        let type_name = self.ident("a function block type")?;
        let Some(kind) = FbKind::parse(&type_name) else {
            return self.err(format!("`{type_name}` is not a function block"));
        };
        let slot = match self.names.get(&name.to_ascii_lowercase()) {
            Some(&s) => {
                if self.vars[s].ty != VarType::Block(kind) {
                    return self.err(format!("`{name}` is declared as another type"));
                }
                s
            }
            None => {
                let s = self.vars.len();
                self.names.insert(name.to_ascii_lowercase(), s);
                self.vars.push(VarDecl {
                    name,
                    ty: VarType::Block(kind),
                    class: VarClass::Local,
                    address: None,
                    init: Value::Bool(false),
                    block_init: Vec::new(),
                    constant: false,
                    hidden: false,
                    line,
                });
                s
            }
        };
        let mut args = vec![(kind.power_input(), Expr::Var(flow))];
        let saved = std::mem::replace(&mut self.no_gt, true);
        while self.accept(&Tok::Comma) {
            let pname = self.ident("a parameter name")?;
            let Some(Field::In(i)) = kind.field(&pname) else {
                return self.err(format!("{} has no input `{pname}`", kind.name()));
            };
            self.expect(&Tok::Assign)?;
            args.push((i, self.expr()?));
        }
        self.no_gt = saved;
        self.expect(&Tok::Gt)?;
        out.push(self.stmt(
            line,
            StmtKind::CallFb {
                slot,
                args,
                outs: Vec::new(),
            },
        ));
        out.push(self.stmt(
            line,
            StmtKind::Assign(
                Target::Var(flow),
                Expr::Member(slot, Field::Out(kind.power_output())),
            ),
        ));
        Ok(ElementKind::Box {
            text: String::new(),
        })
    }

    fn branch(&mut self, flow: usize, depth: usize, out: &mut Vec<Stmt>) -> Res<ElementKind> {
        let line = self.line();
        self.advance(); // {
        let (save, acc) = self.scratch(depth);
        let var = |v| Expr::Var(v);
        out.push(self.stmt(line, StmtKind::Assign(Target::Var(save), var(flow))));
        out.push(self.stmt(
            line,
            StmtKind::Assign(Target::Var(acc), Expr::Lit(Value::Bool(false))),
        ));
        let mut branches = Vec::new();
        loop {
            out.push(self.stmt(line, StmtKind::Assign(Target::Var(flow), var(save))));
            branches.push(self.series(flow, depth + 1, out)?);
            out.push(self.stmt(
                line,
                StmtKind::Assign(
                    Target::Var(acc),
                    Expr::Binary(BinOp::Or, Box::new(var(acc)), Box::new(var(flow))),
                ),
            ));
            if !self.accept(&Tok::Bar) {
                break;
            }
        }
        self.expect(&Tok::RBrace)?;
        out.push(self.stmt(line, StmtKind::Assign(Target::Var(flow), var(acc))));
        Ok(ElementKind::Branch(branches))
    }
}
