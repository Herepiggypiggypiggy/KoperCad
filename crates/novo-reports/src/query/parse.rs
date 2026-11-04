//! The query language: a small, read-only subset of SQL.
//!
//! ```text
//! SELECT [DISTINCT] item {, item} FROM table
//!   [WHERE expr] [GROUP BY expr {, expr}] [HAVING expr]
//!   [ORDER BY expr [ASC|DESC] {, ...}] [LIMIT n]
//! ```
//!
//! No `INSERT`, `UPDATE`, `DELETE` or anything else: a query can only read.

use std::fmt;

use super::value::QValue;

/// What is wrong with a query, and where (a byte offset into its text).
#[derive(Debug, Clone, PartialEq)]
pub struct QueryError {
    pub message: String,
    /// Byte offset in the query text the problem starts at.
    pub at: usize,
}

impl QueryError {
    pub(crate) fn new(message: impl Into<String>, at: usize) -> Self {
        QueryError {
            message: message.into(),
            at,
        }
    }

    /// The message with the line and column, for a window or the terminal.
    pub fn describe(&self, sql: &str) -> String {
        let before = &sql[..self.at.min(sql.len())];
        let line = before.matches('\n').count() + 1;
        let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
        format!("{} (line {line}, column {column})", self.message)
    }
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for QueryError {}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    /// A word: a column, table, keyword or function name.
    Word(String),
    /// A word in double quotes: never a keyword.
    Quoted(String),
    Str(String),
    Int(i64),
    Num(f64),
    /// An operator or punctuation: `= != <> < <= > >= + - * / || ( ) , .`
    Sym(&'static str),
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BinOp {
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Concat,
}

/// An expression.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Expr {
    Lit(QValue),
    /// A column; `index` is filled in by binding to a table.
    Col {
        name: String,
        at: usize,
        index: Option<usize>,
    },
    Not(Box<Expr>),
    Neg(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    IsNull(Box<Expr>, bool),
    Like(Box<Expr>, Box<Expr>, bool),
    In(Box<Expr>, Vec<Expr>, bool),
    Between(Box<Expr>, Box<Expr>, Box<Expr>, bool),
    Func {
        name: String,
        args: Vec<Expr>,
        distinct: bool,
        /// `count(*)`.
        star: bool,
        at: usize,
    },
}

impl Expr {
    /// Visits this expression and everything inside it.
    pub(crate) fn walk(&self, f: &mut dyn FnMut(&Expr)) {
        f(self);
        match self {
            Expr::Lit(_) | Expr::Col { .. } => {}
            Expr::Not(e) | Expr::Neg(e) | Expr::IsNull(e, _) => e.walk(f),
            Expr::Bin(_, a, b) | Expr::Like(a, b, _) => {
                a.walk(f);
                b.walk(f);
            }
            Expr::In(e, list, _) => {
                e.walk(f);
                list.iter().for_each(|x| x.walk(f));
            }
            Expr::Between(a, b, c, _) => {
                a.walk(f);
                b.walk(f);
                c.walk(f);
            }
            Expr::Func { args, .. } => args.iter().for_each(|x| x.walk(f)),
        }
    }

    pub(crate) fn has_aggregate(&self) -> bool {
        let mut found = false;
        self.walk(&mut |e| {
            if let Expr::Func { name, .. } = e
                && is_aggregate(name)
            {
                found = true;
            }
        });
        found
    }
}

pub(crate) fn is_aggregate(name: &str) -> bool {
    matches!(
        name,
        "count" | "sum" | "avg" | "min" | "max" | "group_concat"
    )
}

/// One column of the answer.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Item {
    /// `*`: every column of the table.
    Star,
    Expr {
        expr: Expr,
        alias: Option<String>,
        /// The text the person wrote, used as the header.
        text: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OrderKey {
    pub expr: Expr,
    pub descending: bool,
    pub at: usize,
}

/// A parsed query.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub(crate) distinct: bool,
    pub(crate) items: Vec<Item>,
    pub(crate) table: String,
    pub(crate) table_at: usize,
    pub(crate) filter: Option<Expr>,
    pub(crate) group_by: Vec<Expr>,
    pub(crate) having: Option<Expr>,
    pub(crate) order_by: Vec<OrderKey>,
    pub(crate) limit: Option<usize>,
}

impl Query {
    /// The table the query reads.
    pub fn table(&self) -> &str {
        &self.table
    }
}

/// Words that cannot be a column without quotes.
const RESERVED: &[&str] = &[
    "select", "from", "where", "group", "by", "having", "order", "limit", "as", "and", "or", "not",
    "like", "in", "is", "null", "between", "asc", "desc", "distinct", "true", "false",
];

fn lex(sql: &str) -> Result<Vec<(Tok, usize)>, QueryError> {
    let bytes = sql.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        let start = i;
        if c.is_ascii_whitespace() {
            i += 1;
        } else if c == b'-' && bytes.get(i + 1) == Some(&b'-') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if c == b'\'' || c == b'"' {
            let mut text = String::new();
            i += 1;
            loop {
                match bytes.get(i) {
                    None => {
                        return Err(QueryError::new(
                            if c == b'\'' {
                                "text in quotes is not closed"
                            } else {
                                "name in double quotes is not closed"
                            },
                            start,
                        ));
                    }
                    Some(&q) if q == c => {
                        if bytes.get(i + 1) == Some(&c) {
                            text.push(c as char);
                            i += 2;
                        } else {
                            i += 1;
                            break;
                        }
                    }
                    Some(_) => {
                        let ch = sql[i..].chars().next().unwrap_or(' ');
                        text.push(ch);
                        i += ch.len_utf8();
                    }
                }
            }
            out.push((
                if c == b'\'' {
                    Tok::Str(text)
                } else {
                    Tok::Quoted(text)
                },
                start,
            ));
        } else if c.is_ascii_digit() {
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            let text = &sql[start..i];
            let tok = match text.parse::<i64>() {
                Ok(n) => Tok::Int(n),
                Err(_) => Tok::Num(
                    text.parse()
                        .map_err(|_| QueryError::new(format!("`{text}` is not a number"), start))?,
                ),
            };
            out.push((tok, start));
        } else if c.is_ascii_alphabetic() || c == b'_' || c >= 0x80 {
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] >= 0x80)
            {
                i += 1;
            }
            out.push((Tok::Word(sql[start..i].to_owned()), start));
        } else {
            let two = sql.get(i..i + 2).unwrap_or("");
            let sym = match two {
                "<=" => Some("<="),
                ">=" => Some(">="),
                "!=" => Some("!="),
                "<>" => Some("!="),
                "||" => Some("||"),
                "==" => Some("="),
                _ => None,
            };
            if let Some(sym) = sym {
                out.push((Tok::Sym(sym), start));
                i += 2;
            } else {
                let sym = match c {
                    b'=' => "=",
                    b'<' => "<",
                    b'>' => ">",
                    b'+' => "+",
                    b'-' => "-",
                    b'*' => "*",
                    b'/' => "/",
                    b'(' => "(",
                    b')' => ")",
                    b',' => ",",
                    b'.' => ".",
                    b';' => ";",
                    _ => {
                        let ch = sql[i..].chars().next().unwrap_or('?');
                        return Err(QueryError::new(
                            format!("unexpected character `{ch}`"),
                            start,
                        ));
                    }
                };
                out.push((Tok::Sym(sym), start));
                i += 1;
            }
        }
    }
    out.push((Tok::End, sql.len()));
    Ok(out)
}

struct Parser<'a> {
    sql: &'a str,
    toks: Vec<(Tok, usize)>,
    pos: usize,
}

type PResult<T> = Result<T, QueryError>;

impl Parser<'_> {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos].0
    }

    fn at(&self) -> usize {
        self.toks[self.pos].1
    }

    fn bump(&mut self) -> Tok {
        let tok = self.toks[self.pos].0.clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        tok
    }

    fn is_word(&self, word: &str) -> bool {
        matches!(self.peek(), Tok::Word(w) if w.eq_ignore_ascii_case(word))
    }

    fn eat_word(&mut self, word: &str) -> bool {
        let hit = self.is_word(word);
        if hit {
            self.bump();
        }
        hit
    }

    fn eat_sym(&mut self, sym: &str) -> bool {
        let hit = matches!(self.peek(), Tok::Sym(s) if *s == sym);
        if hit {
            self.bump();
        }
        hit
    }

    fn expect_word(&mut self, word: &str) -> PResult<()> {
        if self.eat_word(word) {
            Ok(())
        } else {
            Err(self.unexpected(&format!("`{}`", word.to_uppercase())))
        }
    }

    fn expect_sym(&mut self, sym: &str) -> PResult<()> {
        if self.eat_sym(sym) {
            Ok(())
        } else {
            Err(self.unexpected(&format!("`{sym}`")))
        }
    }

    fn unexpected(&self, wanted: &str) -> QueryError {
        let found = match self.peek() {
            Tok::End => "the end of the query".to_owned(),
            Tok::Word(w) | Tok::Quoted(w) => format!("`{w}`"),
            Tok::Str(s) => format!("'{s}'"),
            Tok::Int(n) => n.to_string(),
            Tok::Num(n) => n.to_string(),
            Tok::Sym(s) => format!("`{s}`"),
        };
        QueryError::new(format!("expected {wanted}, found {found}"), self.at())
    }

    fn ident(&mut self, what: &str) -> PResult<String> {
        match self.peek().clone() {
            Tok::Quoted(w) => {
                self.bump();
                Ok(w)
            }
            Tok::Word(w) if !RESERVED.contains(&w.to_lowercase().as_str()) => {
                self.bump();
                Ok(w)
            }
            _ => Err(self.unexpected(what)),
        }
    }

    fn query(&mut self) -> PResult<Query> {
        if let Tok::Word(w) = self.peek()
            && !w.eq_ignore_ascii_case("select")
        {
            let w = w.to_lowercase();
            if matches!(
                w.as_str(),
                "insert" | "update" | "delete" | "drop" | "create" | "alter" | "replace" | "pragma"
            ) {
                return Err(QueryError::new(
                    "queries only read the project; start with SELECT",
                    self.at(),
                ));
            }
        }
        self.expect_word("select")?;
        let distinct = self.eat_word("distinct");
        let mut items = Vec::new();
        loop {
            items.push(self.item()?);
            if !self.eat_sym(",") {
                break;
            }
        }
        self.expect_word("from")?;
        let table_at = self.at();
        let table = self.ident("a table name")?;
        let mut q = Query {
            distinct,
            items,
            table: table.to_lowercase(),
            table_at,
            filter: None,
            group_by: Vec::new(),
            having: None,
            order_by: Vec::new(),
            limit: None,
        };
        if self.eat_word("where") {
            q.filter = Some(self.expr()?);
        }
        if self.eat_word("group") {
            self.expect_word("by")?;
            loop {
                q.group_by.push(self.expr()?);
                if !self.eat_sym(",") {
                    break;
                }
            }
        }
        if self.eat_word("having") {
            q.having = Some(self.expr()?);
        }
        if self.eat_word("order") {
            self.expect_word("by")?;
            loop {
                let at = self.at();
                let expr = self.expr()?;
                let descending = if self.eat_word("desc") {
                    true
                } else {
                    self.eat_word("asc");
                    false
                };
                q.order_by.push(OrderKey {
                    expr,
                    descending,
                    at,
                });
                if !self.eat_sym(",") {
                    break;
                }
            }
        }
        if self.eat_word("limit") {
            match self.bump() {
                Tok::Int(n) if n >= 0 => q.limit = Some(n as usize),
                _ => {
                    self.pos = self.pos.saturating_sub(1);
                    return Err(self.unexpected("a row count after LIMIT"));
                }
            }
        }
        while self.eat_sym(";") {}
        if *self.peek() != Tok::End {
            return Err(self.unexpected("the end of the query"));
        }
        Ok(q)
    }

    fn item(&mut self) -> PResult<Item> {
        if self.eat_sym("*") {
            return Ok(Item::Star);
        }
        let start = self.at();
        let expr = self.expr()?;
        let end = self.at();
        let text = self.sql[start..end].trim().to_owned();
        let alias = if self.eat_word("as") {
            Some(self.ident("a name after AS")?)
        } else {
            None
        };
        Ok(Item::Expr { expr, alias, text })
    }

    fn expr(&mut self) -> PResult<Expr> {
        let mut left = self.and()?;
        while self.eat_word("or") {
            let right = self.and()?;
            left = Expr::Bin(BinOp::Or, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn and(&mut self) -> PResult<Expr> {
        let mut left = self.not()?;
        while self.eat_word("and") {
            let right = self.not()?;
            left = Expr::Bin(BinOp::And, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn not(&mut self) -> PResult<Expr> {
        if self.eat_word("not") {
            Ok(Expr::Not(Box::new(self.not()?)))
        } else {
            self.comparison()
        }
    }

    fn comparison(&mut self) -> PResult<Expr> {
        let left = self.additive()?;
        let op = match self.peek() {
            Tok::Sym("=") => Some(BinOp::Eq),
            Tok::Sym("!=") => Some(BinOp::Ne),
            Tok::Sym("<") => Some(BinOp::Lt),
            Tok::Sym("<=") => Some(BinOp::Le),
            Tok::Sym(">") => Some(BinOp::Gt),
            Tok::Sym(">=") => Some(BinOp::Ge),
            _ => None,
        };
        if let Some(op) = op {
            self.bump();
            let right = self.additive()?;
            return Ok(Expr::Bin(op, Box::new(left), Box::new(right)));
        }
        if self.eat_word("is") {
            let negated = self.eat_word("not");
            self.expect_word("null")?;
            return Ok(Expr::IsNull(Box::new(left), negated));
        }
        let negated = self.is_word("not")
            && matches!(
                self.toks.get(self.pos + 1),
                Some((Tok::Word(w), _))
                    if ["like", "in", "between"].contains(&w.to_lowercase().as_str())
            );
        if negated {
            self.bump();
        }
        if self.eat_word("like") {
            let pattern = self.additive()?;
            return Ok(Expr::Like(Box::new(left), Box::new(pattern), negated));
        }
        if self.eat_word("in") {
            self.expect_sym("(")?;
            let mut list = Vec::new();
            loop {
                list.push(self.additive()?);
                if !self.eat_sym(",") {
                    break;
                }
            }
            self.expect_sym(")")?;
            return Ok(Expr::In(Box::new(left), list, negated));
        }
        if self.eat_word("between") {
            let low = self.additive()?;
            self.expect_word("and")?;
            let high = self.additive()?;
            return Ok(Expr::Between(
                Box::new(left),
                Box::new(low),
                Box::new(high),
                negated,
            ));
        }
        Ok(left)
    }

    fn additive(&mut self) -> PResult<Expr> {
        let mut left = self.multiplicative()?;
        loop {
            let op = match self.peek() {
                Tok::Sym("+") => BinOp::Add,
                Tok::Sym("-") => BinOp::Sub,
                Tok::Sym("||") => BinOp::Concat,
                _ => return Ok(left),
            };
            self.bump();
            let right = self.multiplicative()?;
            left = Expr::Bin(op, Box::new(left), Box::new(right));
        }
    }

    fn multiplicative(&mut self) -> PResult<Expr> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek() {
                Tok::Sym("*") => BinOp::Mul,
                Tok::Sym("/") => BinOp::Div,
                _ => return Ok(left),
            };
            self.bump();
            let right = self.unary()?;
            left = Expr::Bin(op, Box::new(left), Box::new(right));
        }
    }

    fn unary(&mut self) -> PResult<Expr> {
        if self.eat_sym("-") {
            Ok(Expr::Neg(Box::new(self.unary()?)))
        } else {
            self.primary()
        }
    }

    fn primary(&mut self) -> PResult<Expr> {
        let at = self.at();
        match self.peek().clone() {
            Tok::Int(n) => {
                self.bump();
                Ok(Expr::Lit(QValue::Int(n)))
            }
            Tok::Num(n) => {
                self.bump();
                Ok(Expr::Lit(QValue::Num(n)))
            }
            Tok::Str(s) => {
                self.bump();
                Ok(Expr::Lit(QValue::Text(s)))
            }
            Tok::Sym("(") => {
                self.bump();
                let e = self.expr()?;
                self.expect_sym(")")?;
                Ok(e)
            }
            Tok::Quoted(name) => {
                self.bump();
                Ok(self.column(name, at))
            }
            Tok::Word(w) => {
                let lower = w.to_lowercase();
                match lower.as_str() {
                    "null" => {
                        self.bump();
                        return Ok(Expr::Lit(QValue::Null));
                    }
                    "true" | "false" => {
                        self.bump();
                        return Ok(Expr::Lit(QValue::Bool(lower == "true")));
                    }
                    _ => {}
                }
                if RESERVED.contains(&lower.as_str()) {
                    return Err(self.unexpected("a column, a value or a function"));
                }
                self.bump();
                if self.eat_sym("(") {
                    return self.call(lower, at);
                }
                Ok(self.column(w, at))
            }
            _ => Err(self.unexpected("a column, a value or a function")),
        }
    }

    /// A column, with an optional `table.` in front of it (the table name
    /// is checked when the query is bound).
    fn column(&mut self, first: String, at: usize) -> Expr {
        let mut name = first;
        if self.eat_sym(".") {
            // `devices.tag`: keep the column, the table part is checked later.
            if let Tok::Word(w) | Tok::Quoted(w) = self.peek().clone() {
                self.bump();
                name = format!("{name}.{w}");
            }
        }
        Expr::Col {
            name,
            at,
            index: None,
        }
    }

    fn call(&mut self, name: String, at: usize) -> PResult<Expr> {
        let mut args = Vec::new();
        let mut star = false;
        let distinct = self.eat_word("distinct");
        if self.eat_sym("*") {
            star = true;
        } else if !matches!(self.peek(), Tok::Sym(")")) {
            loop {
                args.push(self.expr()?);
                if !self.eat_sym(",") {
                    break;
                }
            }
        }
        self.expect_sym(")")?;
        Ok(Expr::Func {
            name,
            args,
            distinct,
            star,
            at,
        })
    }
}

/// Reads a query. Fails with the place of the first problem.
pub fn parse(sql: &str) -> Result<Query, QueryError> {
    let toks = lex(sql)?;
    Parser { sql, toks, pos: 0 }.query()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_full_query() {
        let q = parse(
            "SELECT tag, count(*) AS n FROM devices WHERE kind = 'PLC' AND NOT description LIKE '%x%' \
             GROUP BY tag ORDER BY n DESC, tag LIMIT 5;",
        )
        .unwrap();
        assert_eq!(q.table, "devices");
        assert_eq!(q.items.len(), 2);
        assert_eq!(q.order_by.len(), 2);
        assert!(q.order_by[0].descending);
        assert_eq!(q.limit, Some(5));
    }

    #[test]
    fn only_select_is_allowed() {
        let e = parse("DELETE FROM devices").unwrap_err();
        assert!(e.message.contains("only read"), "{e}");
        assert!(parse("select 1; drop table x").is_err());
    }

    #[test]
    fn errors_point_at_the_problem() {
        let sql = "SELECT tag FROM devices WHERE kind =";
        let e = parse(sql).unwrap_err();
        assert_eq!(e.at, sql.len());
        assert!(e.describe(sql).contains("line 1"));
        let e = parse("SELECT FROM devices").unwrap_err();
        assert!(e.message.contains("expected"), "{e}");
        assert!(parse("SELECT 'open FROM devices").is_err());
    }

    #[test]
    fn handles_in_between_and_negations() {
        parse("SELECT * FROM wires WHERE color IN ('BK','BU') AND cross_section_mm2 NOT BETWEEN 1 AND 2")
            .unwrap();
        parse("SELECT * FROM wires WHERE number IS NOT NULL AND number NOT LIKE '1%'").unwrap();
        parse("SELECT \"order number\" FROM parts -- note").unwrap();
    }
}
