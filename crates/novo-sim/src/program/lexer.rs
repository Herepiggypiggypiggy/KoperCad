//! Tokens of Structured Text and of the ladder notation (they share one
//! lexer: ladder contacts hold ST expressions).

use std::fmt;

use super::address::Address;

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Ident(String),
    Int(i64),
    Real(f64),
    /// A time literal in microseconds.
    Time(i64),
    Addr(Address),
    Assign,    // :=
    Arrow,     // =>
    LParen,    // (
    RParen,    // )
    LBracket,  // [
    RBracket,  // ]
    LBrace,    // {
    RBrace,    // }
    Comma,
    Semi,
    Colon,
    Dot,
    DotDot,
    Plus,
    Minus,
    Star,
    StarStar,
    Slash,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    Amp,
    Bar,
    /// A quoted label, for ladder rung titles.
    Text(String),
    Eof,
}

impl fmt::Display for Tok {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tok::Ident(s) => write!(f, "`{s}`"),
            Tok::Int(i) => write!(f, "`{i}`"),
            Tok::Real(r) => write!(f, "`{r}`"),
            Tok::Time(_) => f.write_str("a time"),
            Tok::Addr(a) => write!(f, "`{a}`"),
            Tok::Text(s) => write!(f, "\"{s}\""),
            Tok::Eof => f.write_str("the end of the program"),
            other => {
                let s = match other {
                    Tok::Assign => ":=",
                    Tok::Arrow => "=>",
                    Tok::LParen => "(",
                    Tok::RParen => ")",
                    Tok::LBracket => "[",
                    Tok::RBracket => "]",
                    Tok::LBrace => "{",
                    Tok::RBrace => "}",
                    Tok::Comma => ",",
                    Tok::Semi => ";",
                    Tok::Colon => ":",
                    Tok::Dot => ".",
                    Tok::DotDot => "..",
                    Tok::Plus => "+",
                    Tok::Minus => "-",
                    Tok::Star => "*",
                    Tok::StarStar => "**",
                    Tok::Slash => "/",
                    Tok::Eq => "=",
                    Tok::Ne => "<>",
                    Tok::Lt => "<",
                    Tok::Gt => ">",
                    Tok::Le => "<=",
                    Tok::Ge => ">=",
                    Tok::Amp => "&",
                    Tok::Bar => "|",
                    _ => "?",
                };
                write!(f, "`{s}`")
            }
        }
    }
}

/// A token and where it starts (1-based line and column).
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub line: u32,
    pub col: u32,
    /// Byte range in the source.
    pub start: usize,
    pub end: usize,
}

/// A syntax or semantic error with its place in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub line: u32,
    pub col: u32,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}, column {}: {}", self.line, self.col, self.message)
    }
}

impl std::error::Error for ParseError {}

impl ParseError {
    pub fn new(line: u32, col: u32, message: impl Into<String>) -> Self {
        ParseError {
            line,
            col,
            message: message.into(),
        }
    }
}

struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    line: u32,
    col: u32,
}

impl Lexer<'_> {
    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn peek_at(&self, n: usize) -> Option<u8> {
        self.src.get(self.pos + n).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let c = self.peek()?;
        self.pos += 1;
        if c == b'\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn err<T>(&self, message: impl Into<String>) -> Result<T, ParseError> {
        Err(ParseError::new(self.line, self.col, message))
    }

    /// Skips white space and comments; an unterminated comment is an error.
    fn skip(&mut self) -> Result<(), ParseError> {
        loop {
            match self.peek() {
                Some(c) if c.is_ascii_whitespace() => {
                    self.bump();
                }
                Some(b'/') if self.peek_at(1) == Some(b'/') => {
                    while self.peek().is_some_and(|c| c != b'\n') {
                        self.bump();
                    }
                }
                Some(b'(') if self.peek_at(1) == Some(b'*') => {
                    let (line, col) = (self.line, self.col);
                    self.bump();
                    self.bump();
                    let mut depth = 1;
                    while depth > 0 {
                        match (self.peek(), self.peek_at(1)) {
                            (Some(b'*'), Some(b')')) => {
                                self.bump();
                                self.bump();
                                depth -= 1;
                            }
                            (Some(b'('), Some(b'*')) => {
                                self.bump();
                                self.bump();
                                depth += 1;
                            }
                            (Some(_), _) => {
                                self.bump();
                            }
                            (None, _) => {
                                return Err(ParseError::new(line, col, "comment is never closed"));
                            }
                        }
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn word(&mut self) -> String {
        let start = self.pos;
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            self.bump();
        }
        String::from_utf8_lossy(&self.src[start..self.pos]).into_owned()
    }

    fn digits(&mut self, radix: u32) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c == b'_' {
                self.bump();
            } else if (c as char).is_digit(radix) {
                s.push(c as char);
                self.bump();
            } else {
                break;
            }
        }
        s
    }

    fn number(&mut self) -> Result<Tok, ParseError> {
        let int = self.digits(10);
        // 16#FF based literal
        if self.peek() == Some(b'#') && matches!(int.as_str(), "2" | "8" | "16") {
            self.bump();
            let radix = int.parse::<u32>().unwrap_or(10);
            let body = self.digits(radix);
            return match i64::from_str_radix(&body, radix) {
                Ok(v) => Ok(Tok::Int(v)),
                Err(_) => self.err(format!("`{int}#{body}` is not a base {radix} number")),
            };
        }
        let mut real = false;
        let mut text = int;
        if self.peek() == Some(b'.') && self.peek_at(1).is_some_and(|c| c.is_ascii_digit()) {
            real = true;
            self.bump();
            text.push('.');
            text.push_str(&self.digits(10));
        }
        if matches!(self.peek(), Some(b'e' | b'E'))
            && (self.peek_at(1).is_some_and(|c| c.is_ascii_digit())
                || (matches!(self.peek_at(1), Some(b'+' | b'-'))
                    && self.peek_at(2).is_some_and(|c| c.is_ascii_digit())))
        {
            real = true;
            self.bump();
            text.push('e');
            if let Some(sign @ (b'+' | b'-')) = self.peek() {
                text.push(sign as char);
                self.bump();
            }
            text.push_str(&self.digits(10));
        }
        if real {
            text.parse().map(Tok::Real).or_else(|_| self.err("bad number"))
        } else {
            text.parse()
                .map(Tok::Int)
                .or_else(|_| self.err(format!("`{text}` is too large")))
        }
    }

    /// `T#1h2m3s4ms`, `T#1.5s`, `T#-2s`, after the `T#` / `TIME#` prefix.
    fn time(&mut self) -> Result<Tok, ParseError> {
        let negative = self.peek() == Some(b'-');
        if negative {
            self.bump();
        }
        let mut total = 0f64;
        let mut any = false;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            let mut num = self.digits(10);
            if self.peek() == Some(b'.') && self.peek_at(1).is_some_and(|c| c.is_ascii_digit()) {
                self.bump();
                num.push('.');
                num.push_str(&self.digits(10));
            }
            let value: f64 = num.parse().unwrap_or(0.0);
            let unit_start = self.pos;
            let mut unit = String::new();
            while let Some(c) = self.peek().filter(u8::is_ascii_alphabetic) {
                unit.push(c.to_ascii_lowercase() as char);
                self.bump();
            }
            let scale = match unit.as_str() {
                "d" => 86_400_000_000.0,
                "h" => 3_600_000_000.0,
                "m" => 60_000_000.0,
                "s" => 1_000_000.0,
                "ms" => 1_000.0,
                "us" => 1.0,
                _ => {
                    self.pos = unit_start;
                    return self.err("a time needs a unit: d, h, m, s, ms or us (T#5s)");
                }
            };
            total += value * scale;
            any = true;
            if self.peek() == Some(b'_') {
                self.bump();
            }
        }
        if !any {
            return self.err("a time needs a value (T#5s)");
        }
        let us = total.round() as i64;
        Ok(Tok::Time(if negative { -us } else { us }))
    }

    fn address(&mut self) -> Result<Tok, ParseError> {
        let start = self.pos;
        self.bump(); // %
        let mut text = String::from("%");
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == b'.' {
                // a trailing `.` that starts `..` is not part of the address
                if c == b'.' && !self.peek_at(1).is_some_and(|n| n.is_ascii_digit()) {
                    break;
                }
                text.push(c as char);
                self.bump();
            } else {
                break;
            }
        }
        match Address::parse(&text) {
            Some(a) => Ok(Tok::Addr(a)),
            None => {
                self.pos = start;
                self.err(format!("`{text}` is not an address (use %IX0.0, %QW2, %MX1.0)"))
            }
        }
    }
}

/// Splits source text into tokens, ending with [`Tok::Eof`].
pub fn lex(source: &str) -> Result<Vec<Token>, ParseError> {
    let mut lx = Lexer {
        src: source.as_bytes(),
        pos: 0,
        line: 1,
        col: 1,
    };
    let mut out = Vec::new();
    loop {
        lx.skip()?;
        let (line, col) = (lx.line, lx.col);
        let start = lx.pos;
        let Some(c) = lx.peek() else {
            out.push(Token {
                tok: Tok::Eof,
                line,
                col,
                start,
                end: start,
            });
            return Ok(out);
        };
        let two = lx.peek_at(1);
        let tok = match c {
            b'0'..=b'9' => lx.number()?,
            b'%' => lx.address()?,
            b'"' => {
                lx.bump();
                let mut s = String::new();
                loop {
                    match lx.bump() {
                        Some(b'"') => break,
                        Some(b) => s.push(b as char),
                        None => return lx.err("a quoted label is never closed"),
                    }
                }
                Tok::Text(s)
            }
            c if c.is_ascii_alphabetic() || c == b'_' => {
                let w = lx.word();
                let upper = w.to_ascii_uppercase();
                if (upper == "T" || upper == "TIME") && lx.peek() == Some(b'#') {
                    lx.bump();
                    lx.time()?
                } else {
                    Tok::Ident(w)
                }
            }
            b':' if two == Some(b'=') => {
                lx.bump();
                lx.bump();
                Tok::Assign
            }
            b'=' if two == Some(b'>') => {
                lx.bump();
                lx.bump();
                Tok::Arrow
            }
            b'<' if two == Some(b'>') => {
                lx.bump();
                lx.bump();
                Tok::Ne
            }
            b'<' if two == Some(b'=') => {
                lx.bump();
                lx.bump();
                Tok::Le
            }
            b'>' if two == Some(b'=') => {
                lx.bump();
                lx.bump();
                Tok::Ge
            }
            b'*' if two == Some(b'*') => {
                lx.bump();
                lx.bump();
                Tok::StarStar
            }
            b'.' if two == Some(b'.') => {
                lx.bump();
                lx.bump();
                Tok::DotDot
            }
            _ => {
                lx.bump();
                match c {
                    b'(' => Tok::LParen,
                    b')' => Tok::RParen,
                    b'[' => Tok::LBracket,
                    b']' => Tok::RBracket,
                    b'{' => Tok::LBrace,
                    b'}' => Tok::RBrace,
                    b',' => Tok::Comma,
                    b';' => Tok::Semi,
                    b':' => Tok::Colon,
                    b'.' => Tok::Dot,
                    b'+' => Tok::Plus,
                    b'-' => Tok::Minus,
                    b'*' => Tok::Star,
                    b'/' => Tok::Slash,
                    b'=' => Tok::Eq,
                    b'<' => Tok::Lt,
                    b'>' => Tok::Gt,
                    b'&' => Tok::Amp,
                    b'|' => Tok::Bar,
                    other => {
                        return Err(ParseError::new(
                            line,
                            col,
                            format!("unexpected character `{}`", other as char),
                        ));
                    }
                }
            }
        };
        out.push(Token {
            tok,
            line,
            col,
            start,
            end: lx.pos,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(src: &str) -> Vec<Tok> {
        lex(src).unwrap().into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn comments_and_times() {
        let t = toks("(* a (* nested *) *) x := T#1m30s; // tail");
        assert_eq!(
            t,
            vec![
                Tok::Ident("x".into()),
                Tok::Assign,
                Tok::Time(90_000_000),
                Tok::Semi,
                Tok::Eof
            ]
        );
    }

    #[test]
    fn numbers_ranges_and_addresses() {
        let t = toks("16#FF 1_000 2.5e1 1..3 %IX0.1");
        assert_eq!(t[0], Tok::Int(255));
        assert_eq!(t[1], Tok::Int(1000));
        assert_eq!(t[2], Tok::Real(25.0));
        assert_eq!(&t[3..6], &[Tok::Int(1), Tok::DotDot, Tok::Int(3)]);
        assert!(matches!(t[6], Tok::Addr(_)));
    }

    #[test]
    fn errors_name_their_place() {
        let e = lex("x := 1;\n  y := $;").unwrap_err();
        assert_eq!((e.line, e.col), (2, 8));
        assert!(lex("(* open").is_err());
        assert!(lex("T#5").is_err());
    }
}
