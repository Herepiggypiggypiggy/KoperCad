//! A small, safe expression language for variants and parametric macros.
//!
//! It answers questions like "is this page wanted when the machine has a
//! conveyor?" (`opt.conveyor && power >= 5.5`) and computes values like a
//! breaker rating from a motor power (`next_size(power * 1000 / (400 * 1.73 * 0.85) * 1.25,
//! 6, 10, 16, 20, 25, 32, 40, 50, 63)`).
//!
//! - Values are numbers, booleans and texts ([`Val`]). Texts use `'single'` or
//!   `"double"` quotes.
//! - Operators, tightest first: `^` (right to left), unary `-` and `!`,
//!   `* / %`, `+ -` (`+` also joins texts), `< <= > >=`, `== !=`, `&&`, `||`.
//! - Variables are looked up by name (letters, digits, `_` and `.`; so
//!   `opt.conveyor` is one name). A name that is not known is an error: nothing silently defaults.
//! - Functions: `min max abs ceil floor round(x[, digits]) sqrt pow if(c, a, b)
//!   next_size(x, a, b, ...)` (the smallest listed value that is at least `x`),
//!   `one_of(x, a, b, ...)`, `choose(i, a, b, ...)` (1-based), `text(x)`,
//!   `len(text)`, `concat(...)`.
//!
//! No loops, no assignment, no access to anything but the variables handed
//! in, so a macro file from someone else cannot do anything but compute.

use std::collections::BTreeMap;
use std::fmt;

use novo_core::Value;

/// What an expression evaluates to.
#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Num(f64),
    Bool(bool),
    Text(String),
}

impl Val {
    pub fn as_bool(&self) -> Result<bool, ExprError> {
        match self {
            Val::Bool(b) => Ok(*b),
            other => Err(ExprError::Type(format!("{other} is not a yes/no value"))),
        }
    }

    pub fn as_num(&self) -> Result<f64, ExprError> {
        match self {
            Val::Num(n) => Ok(*n),
            other => Err(ExprError::Type(format!("{other} is not a number"))),
        }
    }

    /// The model value this evaluates to: whole numbers become `Int`.
    pub fn to_value(&self) -> Value {
        match self {
            Val::Bool(b) => Value::Bool(*b),
            Val::Text(t) => Value::Text(t.clone()),
            Val::Num(n) if n.fract() == 0.0 && n.abs() < 9.0e15 => Value::Int(*n as i64),
            Val::Num(n) => Value::Float(*n),
        }
    }
}

impl From<&Value> for Val {
    fn from(v: &Value) -> Val {
        match v {
            Value::Bool(b) => Val::Bool(*b),
            Value::Int(i) => Val::Num(*i as f64),
            Value::Float(f) => Val::Num(*f),
            Value::Text(t) => Val::Text(t.clone()),
        }
    }
}

impl fmt::Display for Val {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Val::Bool(b) => write!(f, "{b}"),
            Val::Text(t) => f.write_str(t),
            Val::Num(n) if n.fract() == 0.0 && n.abs() < 9.0e15 => write!(f, "{}", *n as i64),
            Val::Num(n) => write!(f, "{n}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ExprError {
    #[error("{0}")]
    Syntax(String),
    #[error("unknown name '{0}'")]
    Unknown(String),
    #[error("{0}")]
    Type(String),
    #[error("{0}")]
    Math(String),
}

/// The names an expression can read.
pub type Vars = BTreeMap<String, Val>;

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Name(String),
    Op(&'static str),
    LParen,
    RParen,
    Comma,
}

fn lex(src: &str) -> Result<Vec<Tok>, ExprError> {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit()
            || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit()))
        {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            let n = text
                .parse::<f64>()
                .map_err(|_| ExprError::Syntax(format!("'{text}' is not a number")))?;
            out.push(Tok::Num(n));
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len()
                && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.')
            {
                i += 1;
            }
            out.push(Tok::Name(chars[start..i].iter().collect()));
        } else if c == '\'' || c == '"' {
            let quote = c;
            i += 1;
            let start = i;
            while i < chars.len() && chars[i] != quote {
                i += 1;
            }
            if i >= chars.len() {
                return Err(ExprError::Syntax("a text is not closed".into()));
            }
            out.push(Tok::Str(chars[start..i].iter().collect()));
            i += 1;
        } else {
            let two: String = chars[i..(i + 2).min(chars.len())].iter().collect();
            let op = match two.as_str() {
                "&&" => Some("&&"),
                "||" => Some("||"),
                "==" => Some("=="),
                "!=" => Some("!="),
                "<=" => Some("<="),
                ">=" => Some(">="),
                _ => None,
            };
            if let Some(op) = op {
                out.push(Tok::Op(op));
                i += 2;
                continue;
            }
            match c {
                '(' => out.push(Tok::LParen),
                ')' => out.push(Tok::RParen),
                ',' => out.push(Tok::Comma),
                '+' => out.push(Tok::Op("+")),
                '-' => out.push(Tok::Op("-")),
                '*' => out.push(Tok::Op("*")),
                '/' => out.push(Tok::Op("/")),
                '%' => out.push(Tok::Op("%")),
                '^' => out.push(Tok::Op("^")),
                '<' => out.push(Tok::Op("<")),
                '>' => out.push(Tok::Op(">")),
                '!' => out.push(Tok::Op("!")),
                other => return Err(ExprError::Syntax(format!("unexpected '{other}'"))),
            }
            i += 1;
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Lit(Val),
    Var(String),
    Not(Box<Node>),
    Neg(Box<Node>),
    Bin(&'static str, Box<Node>, Box<Node>),
    Call(String, Vec<Node>),
}

struct Parser {
    toks: Vec<Tok>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.at)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.at).cloned();
        self.at += 1;
        t
    }

    fn op(&mut self, ops: &[&'static str]) -> Option<&'static str> {
        if let Some(Tok::Op(o)) = self.peek()
            && let Some(found) = ops.iter().find(|x| *x == o)
        {
            self.at += 1;
            return Some(found);
        }
        None
    }

    fn level(&mut self, depth: usize) -> Result<Node, ExprError> {
        const LEVELS: [&[&str]; 5] = [
            &["||"],
            &["&&"],
            &["==", "!="],
            &["<", "<=", ">", ">="],
            &["+", "-"],
        ];
        if depth == LEVELS.len() {
            return self.term();
        }
        let mut left = self.level(depth + 1)?;
        while let Some(o) = self.op(LEVELS[depth]) {
            let right = self.level(depth + 1)?;
            left = Node::Bin(o, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn term(&mut self) -> Result<Node, ExprError> {
        let mut left = self.unary()?;
        while let Some(o) = self.op(&["*", "/", "%"]) {
            let right = self.unary()?;
            left = Node::Bin(o, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Node, ExprError> {
        if self.op(&["!"]).is_some() {
            return Ok(Node::Not(Box::new(self.unary()?)));
        }
        if self.op(&["-"]).is_some() {
            return Ok(Node::Neg(Box::new(self.unary()?)));
        }
        self.power()
    }

    fn power(&mut self) -> Result<Node, ExprError> {
        let base = self.primary()?;
        if self.op(&["^"]).is_some() {
            let exp = self.unary()?;
            return Ok(Node::Bin("^", Box::new(base), Box::new(exp)));
        }
        Ok(base)
    }

    fn primary(&mut self) -> Result<Node, ExprError> {
        match self.next() {
            Some(Tok::Num(n)) => Ok(Node::Lit(Val::Num(n))),
            Some(Tok::Str(s)) => Ok(Node::Lit(Val::Text(s))),
            Some(Tok::LParen) => {
                let inner = self.level(0)?;
                match self.next() {
                    Some(Tok::RParen) => Ok(inner),
                    _ => Err(ExprError::Syntax("a ')' is missing".into())),
                }
            }
            Some(Tok::Name(name)) => {
                if self.peek() == Some(&Tok::LParen) {
                    self.at += 1;
                    let mut args = Vec::new();
                    if self.peek() == Some(&Tok::RParen) {
                        self.at += 1;
                    } else {
                        loop {
                            args.push(self.level(0)?);
                            match self.next() {
                                Some(Tok::Comma) => {}
                                Some(Tok::RParen) => break,
                                _ => {
                                    return Err(ExprError::Syntax(format!(
                                        "'{name}(' needs ',' or ')'"
                                    )));
                                }
                            }
                        }
                    }
                    Ok(Node::Call(name, args))
                } else {
                    match name.as_str() {
                        "true" => Ok(Node::Lit(Val::Bool(true))),
                        "false" => Ok(Node::Lit(Val::Bool(false))),
                        "pi" => Ok(Node::Lit(Val::Num(std::f64::consts::PI))),
                        _ => Ok(Node::Var(name)),
                    }
                }
            }
            Some(other) => Err(ExprError::Syntax(format!("unexpected {other:?}"))),
            None => Err(ExprError::Syntax("the expression ends too soon".into())),
        }
    }
}

/// A parsed expression.
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    root: Node,
}

impl Expr {
    pub fn parse(src: &str) -> Result<Expr, ExprError> {
        if src.trim().is_empty() {
            return Err(ExprError::Syntax("the expression is empty".into()));
        }
        let mut p = Parser {
            toks: lex(src)?,
            at: 0,
        };
        let root = p.level(0)?;
        if p.at < p.toks.len() {
            return Err(ExprError::Syntax(format!(
                "unexpected {:?} after the expression",
                p.toks[p.at]
            )));
        }
        Ok(Expr { root })
    }

    pub fn eval(&self, vars: &Vars) -> Result<Val, ExprError> {
        eval(&self.root, vars)
    }

    /// The names the expression reads (for finding what depends on what).
    pub fn names(&self) -> Vec<String> {
        fn walk(n: &Node, out: &mut Vec<String>) {
            match n {
                Node::Var(v) => out.push(v.clone()),
                Node::Not(a) | Node::Neg(a) => walk(a, out),
                Node::Bin(_, a, b) => {
                    walk(a, out);
                    walk(b, out);
                }
                Node::Call(_, args) => args.iter().for_each(|a| walk(a, out)),
                Node::Lit(_) => {}
            }
        }
        let mut out = Vec::new();
        walk(&self.root, &mut out);
        out.sort();
        out.dedup();
        out
    }
}

/// Parses and evaluates in one step.
pub fn eval_str(src: &str, vars: &Vars) -> Result<Val, ExprError> {
    Expr::parse(src)?.eval(vars)
}

fn eval(node: &Node, vars: &Vars) -> Result<Val, ExprError> {
    match node {
        Node::Lit(v) => Ok(v.clone()),
        Node::Var(name) => vars
            .get(name)
            .cloned()
            .ok_or_else(|| ExprError::Unknown(name.clone())),
        Node::Not(a) => Ok(Val::Bool(!eval(a, vars)?.as_bool()?)),
        Node::Neg(a) => Ok(Val::Num(-eval(a, vars)?.as_num()?)),
        Node::Bin(op, a, b) => match *op {
            // Short-circuit, so `opt.x && 1 / n > 2` needs no `n` when x is off.
            "&&" => Ok(Val::Bool(
                eval(a, vars)?.as_bool()? && eval(b, vars)?.as_bool()?,
            )),
            "||" => Ok(Val::Bool(
                eval(a, vars)?.as_bool()? || eval(b, vars)?.as_bool()?,
            )),
            _ => binary(op, eval(a, vars)?, eval(b, vars)?),
        },
        Node::Call(name, args) => {
            if name == "if" {
                if args.len() != 3 {
                    return Err(ExprError::Type("if(c, a, b) takes three values".into()));
                }
                // Only the branch taken is evaluated.
                let pick = if eval(&args[0], vars)?.as_bool()? {
                    1
                } else {
                    2
                };
                return eval(&args[pick], vars);
            }
            let values = args
                .iter()
                .map(|a| eval(a, vars))
                .collect::<Result<Vec<_>, _>>()?;
            call(name, &values)
        }
    }
}

fn binary(op: &str, a: Val, b: Val) -> Result<Val, ExprError> {
    use Val::*;
    match op {
        "+" => match (&a, &b) {
            (Num(x), Num(y)) => Ok(Num(x + y)),
            (Text(_), _) | (_, Text(_)) => Ok(Text(format!("{a}{b}"))),
            _ => Err(ExprError::Type(format!("cannot add {a} and {b}"))),
        },
        "-" | "*" | "/" | "%" | "^" => {
            let (x, y) = (a.as_num()?, b.as_num()?);
            match op {
                "-" => Ok(Num(x - y)),
                "*" => Ok(Num(x * y)),
                "/" if y == 0.0 => Err(ExprError::Math("division by zero".into())),
                "/" => Ok(Num(x / y)),
                "%" if y == 0.0 => Err(ExprError::Math("division by zero".into())),
                "%" => Ok(Num(x % y)),
                _ => Ok(Num(x.powf(y))),
            }
        }
        "<" | "<=" | ">" | ">=" => {
            let (x, y) = (a.as_num()?, b.as_num()?);
            Ok(Bool(match op {
                "<" => x < y,
                "<=" => x <= y,
                ">" => x > y,
                _ => x >= y,
            }))
        }
        "==" => Ok(Bool(same(&a, &b))),
        "!=" => Ok(Bool(!same(&a, &b))),
        _ => Err(ExprError::Syntax(format!("unknown operator {op}"))),
    }
}

fn same(a: &Val, b: &Val) -> bool {
    match (a, b) {
        (Val::Num(x), Val::Num(y)) => (x - y).abs() < 1e-9 * x.abs().max(y.abs()).max(1.0),
        (Val::Text(x), Val::Text(y)) => x.eq_ignore_ascii_case(y),
        (Val::Bool(x), Val::Bool(y)) => x == y,
        _ => false,
    }
}

fn call(name: &str, a: &[Val]) -> Result<Val, ExprError> {
    let arity = |n: usize| -> Result<(), ExprError> {
        if a.len() == n {
            Ok(())
        } else {
            Err(ExprError::Type(format!("{name}() takes {n} value(s)")))
        }
    };
    let nums = || {
        a.iter()
            .map(|v| v.as_num())
            .collect::<Result<Vec<f64>, _>>()
    };
    match name {
        "min" | "max" => {
            let n = nums()?;
            if n.is_empty() {
                return Err(ExprError::Type(format!("{name}() needs a value")));
            }
            let f = if name == "min" { f64::min } else { f64::max };
            Ok(Val::Num(n.into_iter().reduce(f).unwrap()))
        }
        "abs" => {
            arity(1)?;
            Ok(Val::Num(a[0].as_num()?.abs()))
        }
        "ceil" => {
            arity(1)?;
            Ok(Val::Num(a[0].as_num()?.ceil()))
        }
        "floor" => {
            arity(1)?;
            Ok(Val::Num(a[0].as_num()?.floor()))
        }
        "sqrt" => {
            arity(1)?;
            let x = a[0].as_num()?;
            if x < 0.0 {
                return Err(ExprError::Math("sqrt of a negative number".into()));
            }
            Ok(Val::Num(x.sqrt()))
        }
        "pow" => {
            arity(2)?;
            Ok(Val::Num(a[0].as_num()?.powf(a[1].as_num()?)))
        }
        "round" => {
            if a.is_empty() || a.len() > 2 {
                return Err(ExprError::Type(
                    "round(x[, digits]) takes one or two values".into(),
                ));
            }
            let x = a[0].as_num()?;
            let digits = a.get(1).map(|d| d.as_num()).transpose()?.unwrap_or(0.0);
            let f = 10f64.powi(digits as i32);
            Ok(Val::Num((x * f).round() / f))
        }
        "next_size" => {
            if a.len() < 2 {
                return Err(ExprError::Type("next_size(x, sizes...) needs sizes".into()));
            }
            let x = a[0].as_num()?;
            let mut sizes: Vec<f64> = a[1..]
                .iter()
                .map(|v| v.as_num())
                .collect::<Result<_, _>>()?;
            sizes.sort_by(f64::total_cmp);
            sizes
                .into_iter()
                .find(|s| *s >= x - 1e-9)
                .map(Val::Num)
                .ok_or_else(|| ExprError::Math(format!("{x} is larger than every listed size")))
        }
        "one_of" => {
            if a.len() < 2 {
                return Err(ExprError::Type("one_of(x, a, b, ...) needs choices".into()));
            }
            Ok(Val::Bool(a[1..].iter().any(|v| same(&a[0], v))))
        }
        "choose" => {
            if a.len() < 2 {
                return Err(ExprError::Type("choose(i, a, b, ...) needs choices".into()));
            }
            let i = a[0].as_num()?;
            let choices = &a[1..];
            if i.fract() != 0.0 || i < 1.0 || i as usize > choices.len() {
                return Err(ExprError::Math(format!(
                    "choose({i}, ...) has only {} choices",
                    choices.len()
                )));
            }
            Ok(choices[i as usize - 1].clone())
        }
        "text" => {
            arity(1)?;
            Ok(Val::Text(a[0].to_string()))
        }
        "len" => {
            arity(1)?;
            match &a[0] {
                Val::Text(t) => Ok(Val::Num(t.chars().count() as f64)),
                other => Err(ExprError::Type(format!("{other} is not a text"))),
            }
        }
        "concat" => Ok(Val::Text(a.iter().map(|v| v.to_string()).collect())),
        other => Err(ExprError::Unknown(format!("{other}()"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, Val)]) -> Vars {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    fn num(src: &str, v: &Vars) -> f64 {
        eval_str(src, v).unwrap().as_num().unwrap()
    }

    #[test]
    fn arithmetic_and_precedence() {
        let v = Vars::new();
        assert_eq!(num("1 + 2 * 3", &v), 7.0);
        assert_eq!(num("(1 + 2) * 3", &v), 9.0);
        assert_eq!(num("2 ^ 3 ^ 2", &v), 512.0);
        assert_eq!(num("-2 ^ 2", &v), -4.0);
        assert_eq!(num("10 % 4 - 1", &v), 1.0);
    }

    #[test]
    fn logic_and_comparison() {
        let v = vars(&[("opt.conveyor", Val::Bool(true)), ("power", Val::Num(7.5))]);
        assert_eq!(
            eval_str("opt.conveyor && power >= 5.5", &v).unwrap(),
            Val::Bool(true)
        );
        assert_eq!(
            eval_str("!opt.conveyor || power < 1", &v).unwrap(),
            Val::Bool(false)
        );
        assert_eq!(eval_str("'EN' == 'en'", &v).unwrap(), Val::Bool(true));
    }

    #[test]
    fn short_circuit_skips_unknown_names() {
        let v = vars(&[("opt.x", Val::Bool(false))]);
        assert_eq!(
            eval_str("opt.x && missing > 1", &v).unwrap(),
            Val::Bool(false)
        );
        assert!(matches!(
            eval_str("missing > 1", &v),
            Err(ExprError::Unknown(_))
        ));
    }

    #[test]
    fn motor_breaker_sizing_formula() {
        // 7.5 kW at 400 V, cos phi 0.85: I = P / (sqrt(3) U cos phi) = 12.74 A;
        // 125 % for the breaker is 15.9 A, so the next standard size is 16 A.
        let v = vars(&[("power", Val::Num(7.5))]);
        let i = num("power * 1000 / (sqrt(3) * 400 * 0.85)", &v);
        assert!((i - 12.737).abs() < 0.01, "{i}");
        let size = num(
            "next_size(power * 1000 / (sqrt(3) * 400 * 0.85) * 1.25, 6, 10, 16, 20, 25, 32)",
            &v,
        );
        assert_eq!(size, 16.0);
        assert!(eval_str("next_size(100, 6, 10)", &v).is_err());
    }

    #[test]
    fn functions() {
        let v = Vars::new();
        assert_eq!(num("max(1, 5, 3)", &v), 5.0);
        assert_eq!(num("round(2.346, 2)", &v), 2.35);
        assert_eq!(num("if(1 < 2, 10, 1 / 0)", &v), 10.0);
        assert_eq!(num("choose(2, 10, 20, 30)", &v), 20.0);
        assert_eq!(
            eval_str("concat('Q', 3, '.', 1)", &v).unwrap(),
            Val::Text("Q3.1".into())
        );
        assert_eq!(
            eval_str("one_of('b', 'a', 'B')", &v).unwrap(),
            Val::Bool(true)
        );
        assert_eq!(eval_str("'M' + 2", &v).unwrap(), Val::Text("M2".into()));
    }

    #[test]
    fn errors_are_plain() {
        let v = Vars::new();
        assert!(matches!(eval_str("1 +", &v), Err(ExprError::Syntax(_))));
        assert!(matches!(eval_str("1 / 0", &v), Err(ExprError::Math(_))));
        assert!(matches!(eval_str("1 && 2", &v), Err(ExprError::Type(_))));
        assert!(matches!(
            eval_str("nope(1)", &v),
            Err(ExprError::Unknown(_))
        ));
        assert!(matches!(eval_str("(1", &v), Err(ExprError::Syntax(_))));
        assert!(Expr::parse("  ").is_err());
    }

    #[test]
    fn names_listed() {
        let e = Expr::parse("opt.a && (power > 2 || opt.b)").unwrap();
        assert_eq!(e.names(), vec!["opt.a", "opt.b", "power"]);
    }

    #[test]
    fn whole_numbers_become_int_values() {
        assert_eq!(Val::Num(16.0).to_value(), Value::Int(16));
        assert_eq!(Val::Num(1.5).to_value(), Value::Float(1.5));
    }
}
