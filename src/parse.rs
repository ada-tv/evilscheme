#![allow(dead_code)]

use std::{fmt::Display, iter::Peekable, num::ParseFloatError, str::Chars};

#[derive(Debug, PartialEq, Clone)]
pub enum Atom {
    Nil,
    List(Vec<Atom>),
    Symbol(String),
    String(String),
    Number(f64),
    Bool(bool),
    Quote(Box<Atom>),

    // not directly parsed, only exists during evaluation
    Function(Vec<String>, Box<Atom>),
    HostFunction(usize),
}

impl std::fmt::Display for Atom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nil => f.write_str("()"),
            Self::Number(x) => write!(f, "{}", x),
            Self::String(x) => f.write_str(x),
            Self::Symbol(x) => f.write_str(x),
            Self::Bool(x) => {
                if *x {
                    f.write_str("#t")
                } else {
                    f.write_str("#f")
                }
            }

            Self::List(list) => {
                f.write_str("(")?;

                if !list.is_empty() {
                    for value in &list[..list.len() - 1] {
                        value.fmt(f)?;
                        f.write_str(" ")?;
                    }

                    list[list.len() - 1].fmt(f)?;
                }

                f.write_str(")")
            }

            Self::Quote(x) => write!(f, "'{}", x),

            Self::Function(bindings, body) => {
                f.write_str("(lambda (")?;

                if !bindings.is_empty() {
                    for value in &bindings[..bindings.len() - 1] {
                        value.fmt(f)?;
                        f.write_str(" ")?;
                    }

                    bindings[bindings.len() - 1].fmt(f)?;
                }

                f.write_str(") ")?;

                body.fmt(f)?;

                f.write_str(")")
            }

            Self::HostFunction(x) => write!(f, "[native #{x}]"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Location {
    line: usize,
    column: usize,
}

impl Default for Location {
    fn default() -> Self {
        Self { line: 1, column: 1 }
    }
}

impl Location {
    pub fn next_line(&mut self) {
        self.line += 1;
        self.column = 1;
    }

    pub fn next_column(&mut self) {
        self.column += 1;
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum AtomParseError {
    EarlyEOF(Location),
    UnexpectedChar(char, Location),
    ExpectedChar(char, Location),
    InvalidEscape(char, Location),
    InvalidNumber(ParseFloatError, Location),
}

impl Display for AtomParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EarlyEOF(Location { line, column }) => write!(
                f,
                "Unexpected end-of-file (line {}, column {})",
                line, column
            ),
            Self::UnexpectedChar(c, Location { line, column }) => write!(
                f,
                "Unexpected character '{:?}' (line {}, column {})",
                c, line, column
            ),
            Self::ExpectedChar(c, Location { line, column }) => write!(
                f,
                "Expected character '{:?}' (line {}, column {})",
                c, line, column
            ),
            Self::InvalidEscape(c, Location { line, column }) => write!(
                f,
                "Invalid string escape '\\{:?}' (line {}, column {})",
                c, line, column
            ),
            Self::InvalidNumber(e, Location { line, column }) => {
                write!(f, "{} (line {}, column {})", e, line, column)
            }
        }
    }
}

impl std::error::Error for AtomParseError {}

struct ParserState<'a> {
    location: Location,
    src: Peekable<Chars<'a>>,
}

impl<'a> ParserState<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            location: Location::default(),
            src: src.chars().peekable(),
        }
    }

    pub fn peek(&mut self) -> Option<char> {
        self.src.peek().copied()
    }

    pub fn skip_whitespace(&mut self) {
        let mut in_comment = false;

        while let Some(c) = self.peek() {
            if in_comment && c == '\n' {
                in_comment = false;
            }

            if c == ';' {
                in_comment = true;
            }

            if !in_comment && !c.is_whitespace() {
                return;
            }

            self.next();
        }
    }
}

impl<'a> Iterator for ParserState<'a> {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        match self.src.next() {
            Some('\n') => {
                self.location.next_line();
                Some('\n')
            }
            Some(c) => {
                self.location.next_column();
                Some(c)
            }
            None => None,
        }
    }
}

impl Atom {
    fn parse_special(state: &mut ParserState) -> Result<Atom, AtomParseError> {
        if state.next() != Some('#') {
            return Err(AtomParseError::ExpectedChar('#', state.location));
        }

        match state.next() {
            Some('t') => Ok(Atom::Bool(true)),
            Some('f') => Ok(Atom::Bool(false)),
            Some(c) => Err(AtomParseError::UnexpectedChar(c, state.location)),
            None => Err(AtomParseError::EarlyEOF(state.location)),
        }
    }

    // '(' Atom* ')'
    fn parse_list(state: &mut ParserState) -> Result<Atom, AtomParseError> {
        let start_loc = state.location;
        let mut list = Vec::new();

        if state.next() != Some('(') {
            return Err(AtomParseError::ExpectedChar('(', state.location));
        }

        loop {
            state.skip_whitespace();

            if state.peek() == Some(')') {
                state.next();
                break;
            } else if state.peek().is_none() {
                return Err(AtomParseError::EarlyEOF(start_loc));
            } else {
                list.push(Self::parse_atom(state)?);
            }
        }

        Ok(Atom::List(list))
    }

    // [a-zA-Z_][^\s]*
    fn parse_symbol(state: &mut ParserState) -> Result<Atom, AtomParseError> {
        let mut buf = String::new();

        while let Some(c) = state.peek() {
            if c.is_whitespace() || c == ')' || c == '(' {
                break;
            }

            buf.push(c);
            state.next();
        }

        Ok(Atom::Symbol(buf))
    }

    // '"' ([^"]|'\"')* '"'
    fn parse_string(state: &mut ParserState) -> Result<Atom, AtomParseError> {
        let start_loc = state.location;
        let mut buf = String::new();

        if state.next() != Some('"') {
            return Err(AtomParseError::ExpectedChar('"', start_loc));
        }

        loop {
            let Some(c) = state.next() else {
                return Err(AtomParseError::EarlyEOF(start_loc));
            };

            if c == '\\' {
                let Some(c) = state.next() else {
                    return Err(AtomParseError::EarlyEOF(start_loc));
                };

                match c {
                    'r' => {
                        buf.push('\r');
                        continue;
                    }
                    'n' => {
                        buf.push('\n');
                        continue;
                    }
                    't' => {
                        buf.push('\t');
                        continue;
                    }
                    '\\' => {
                        buf.push('\\');
                        continue;
                    }
                    '"' => {
                        buf.push('"');
                        continue;
                    }
                    _ => return Err(AtomParseError::InvalidEscape(c, state.location)),
                }
            }

            if c == '"' {
                break;
            }

            buf.push(c);
        }

        Ok(Atom::String(buf))
    }

    // '-'? [0-9]+('.' [0-9]+)?([eE] [0-9]+)?
    fn parse_number(state: &mut ParserState) -> Result<Atom, AtomParseError> {
        let start_loc = state.location;

        let mut buf = String::new();

        while let Some(c) = state.peek() {
            if !(c.is_ascii_digit() || c == '.' || c == 'e' || c == 'E' || c == '-' || c == '+') {
                break;
            }

            buf.push(c);
            state.next();
        }

        match buf.parse::<f64>() {
            Ok(n) => Ok(Atom::Number(n)),
            Err(e) => Err(AtomParseError::InvalidNumber(e, start_loc)),
        }
    }

    fn parse_atom(state: &mut ParserState) -> Result<Atom, AtomParseError> {
        match state.peek() {
            Some('(') => Self::parse_list(state),

            Some('"') => Self::parse_string(state),

            Some('#') => Self::parse_special(state),

            Some('\'') => {
                state.next();
                Ok(Atom::Quote(Box::new(Self::parse_atom(state)?)))
            }

            // could either be a symbol or a negative number
            Some('-') => {
                state.next();

                let Some(c) = state.peek() else {
                    return Err(AtomParseError::EarlyEOF(state.location));
                };

                if c.is_ascii_digit() {
                    let Atom::Number(num) = Self::parse_number(state)? else {
                        unreachable!()
                    };
                    Ok(Atom::Number(-num))
                } else if c.is_whitespace() || c == '(' {
                    // it's literally just '-'
                    Ok(Atom::Symbol("-".into()))
                } else {
                    let Atom::Symbol(sym) = Self::parse_symbol(state)? else {
                        unreachable!()
                    };
                    // glue the - back on
                    Ok(Atom::Symbol(format!("-{}", sym)))
                }
            }

            Some(c) if c.is_ascii_digit() => Self::parse_number(state),

            Some(c) if !c.is_whitespace() => Self::parse_symbol(state),

            Some(c) => Err(AtomParseError::UnexpectedChar(c, state.location)),
            None => Err(AtomParseError::EarlyEOF(state.location)),
        }
    }

    pub fn parse(src: &str) -> Result<Atom, AtomParseError> {
        let mut state = ParserState::new(src);
        state.skip_whitespace();

        let mut list = Vec::new();

        loop {
            state.skip_whitespace();

            if state.peek().is_none() {
                break;
            }

            list.push(Self::parse_atom(&mut state)?);
        }

        if list.len() == 1 {
            Ok(list[0].clone())
        } else {
            Ok(Self::List(list))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_string() {
        assert_eq!(
            Atom::parse("\"hello world\""),
            Ok(Atom::String("hello world".into()))
        );
        assert_eq!(
            Atom::parse("\"\\\"☺\\r\\n\\t\""),
            Ok(Atom::String("\"☺\r\n\t".into()))
        );
    }

    #[test]
    fn parse_symbol() {
        assert_eq!(
            Atom::parse("my-symbol"),
            Ok(Atom::Symbol("my-symbol".into()))
        );
        assert_eq!(
            Atom::parse("_starts-underscore"),
            Ok(Atom::Symbol("_starts-underscore".into()))
        );
    }

    #[test]
    fn parse_number() {
        assert_eq!(Atom::parse("1234"), Ok(Atom::Number(1234.0)));
        assert_eq!(Atom::parse("12.34"), Ok(Atom::Number(12.34)));
        assert_eq!(Atom::parse("-1234"), Ok(Atom::Number(-1234.0)));
        assert_eq!(Atom::parse("1e6"), Ok(Atom::Number(1e6)));
    }

    #[test]
    fn parse_list() {
        assert_eq!(Atom::parse("()"), Ok(Atom::List(vec![])));
        assert_eq!(
            Atom::parse("(symbol 1234 () \"string\")"),
            Ok(Atom::List(vec![
                Atom::Symbol("symbol".into()),
                Atom::Number(1234.0),
                Atom::List(vec![]),
                Atom::String("string".into()),
            ]))
        );
    }

    #[test]
    fn parse_with_comments() {
        const TEST_SOURCE: &str = r#"
;; this is a test script! woohoo!
(print "message")

(if my-var
    ; comment inside a list
    (print "my-var is true")
    (print "my-var is false"))
"#;

        assert_eq!(
            Atom::parse(TEST_SOURCE),
            Ok(Atom::List(vec![
                Atom::List(vec![
                    Atom::Symbol("print".into()),
                    Atom::String("message".into()),
                ]),
                Atom::List(vec![
                    Atom::Symbol("if".into()),
                    Atom::Symbol("my-var".into()),
                    Atom::List(vec![
                        Atom::Symbol("print".into()),
                        Atom::String("my-var is true".into()),
                    ]),
                    Atom::List(vec![
                        Atom::Symbol("print".into()),
                        Atom::String("my-var is false".into()),
                    ]),
                ]),
            ])),
        );
    }
}
