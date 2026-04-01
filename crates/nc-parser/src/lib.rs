//! # nc-parser — S-expression Parser
//!
//! Minimal, zero-dependency recursive-descent parser for S-expressions.
//! Produces a `SexpNode` tree that downstream crates validate against typed IR.
//!
//! ## Design Rationale
//!
//! S-expressions are the ideal interface between LLMs and deterministic systems:
//! - **Minimal syntax**: only atoms and parenthesized lists
//! - **AST = Data**: no parsing ambiguity, no operator precedence
//! - **LLM-proof**: the only rule is bracket matching
//!
//! This parser is deliberately simple (~150 lines). The real rigor comes from
//! the typed IR layer (`nc-ir`) that validates parsed trees against whitelisted enums.

use std::fmt;

/// S-expression node — either an atom (symbol/string) or a list of child nodes.
#[derive(Debug, Clone, PartialEq)]
pub enum SexpNode {
    Atom(String),
    List(Vec<SexpNode>),
}

// ---------------------------------------------------------------------------
// Parser internals
// ---------------------------------------------------------------------------

struct Parser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn skip_ws(&mut self) {
        while self.pos < self.input.len() {
            let ch = self.input[self.pos];
            if ch == b';' {
                while self.pos < self.input.len() && self.input[self.pos] != b'\n' {
                    self.pos += 1;
                }
            } else if ch.is_ascii_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn parse_string(&mut self) -> Result<String, ParseError> {
        let mut s = String::new();
        loop {
            match self.peek() {
                None => return Err(ParseError("unterminated string literal".into())),
                Some(b'\\') => {
                    self.advance();
                    match self.peek() {
                        Some(b'"') => { s.push('"'); self.advance(); }
                        Some(b'\\') => { s.push('\\'); self.advance(); }
                        Some(b'n') => { s.push('\n'); self.advance(); }
                        Some(b't') => { s.push('\t'); self.advance(); }
                        Some(ch) => { s.push('\\'); s.push(ch as char); self.advance(); }
                        None => return Err(ParseError("unterminated escape".into())),
                    }
                }
                Some(b'"') => { self.advance(); return Ok(s); }
                Some(ch) if ch >= 0x80 => {
                    let remaining = &self.input[self.pos..];
                    let text = std::str::from_utf8(remaining)
                        .map_err(|e| ParseError(format!("invalid UTF-8: {e}")))?;
                    if let Some(c) = text.chars().next() {
                        s.push(c);
                        self.pos += c.len_utf8();
                    }
                }
                Some(ch) => { s.push(ch as char); self.advance(); }
            }
        }
    }

    fn parse_atom(&mut self) -> String {
        let start = self.pos;
        while self.pos < self.input.len() {
            let ch = self.input[self.pos];
            if ch.is_ascii_whitespace() || ch == b'(' || ch == b')' || ch == b'"' || ch == b';' {
                break;
            }
            self.pos += 1;
        }
        String::from_utf8_lossy(&self.input[start..self.pos]).into_owned()
    }

    fn parse_node(&mut self) -> Result<SexpNode, ParseError> {
        self.skip_ws();
        match self.peek() {
            Some(b'(') => {
                self.advance();
                let mut children = Vec::new();
                loop {
                    self.skip_ws();
                    match self.peek() {
                        Some(b')') => { self.advance(); return Ok(SexpNode::List(children)); }
                        None => return Err(ParseError("unmatched opening '('".into())),
                        _ => children.push(self.parse_node()?),
                    }
                }
            }
            Some(b'"') => { self.advance(); Ok(SexpNode::Atom(self.parse_string()?)) }
            Some(b')') => Err(ParseError("unexpected ')'".into())),
            Some(_) => Ok(SexpNode::Atom(self.parse_atom())),
            None => Err(ParseError("unexpected end of input".into())),
        }
    }

    fn parse_all(&mut self) -> Result<Vec<SexpNode>, ParseError> {
        let mut nodes = Vec::new();
        loop {
            self.skip_ws();
            if self.pos >= self.input.len() { break; }
            nodes.push(self.parse_node()?);
        }
        Ok(nodes)
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Parse error with a human-readable message.
#[derive(Debug, Clone)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "S-expr parse error: {}", self.0)
    }
}

impl std::error::Error for ParseError {}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

impl SexpNode {
    /// Parse S-expression text into a list of top-level nodes.
    pub fn parse(input: &str) -> Result<Vec<SexpNode>, ParseError> {
        Parser::new(input).parse_all()
    }

    /// Parse exactly one top-level S-expression.
    pub fn parse_one(input: &str) -> Result<SexpNode, ParseError> {
        let nodes = Self::parse(input)?;
        if nodes.len() != 1 {
            return Err(ParseError(format!("expected 1 top-level node, got {}", nodes.len())));
        }
        Ok(nodes.into_iter().next().unwrap())
    }

    /// If this is a List, return the first Atom (the tag).
    /// `(api :method POST)` → `Some("api")`
    pub fn tag(&self) -> Option<&str> {
        match self {
            SexpNode::List(children) => match children.first() {
                Some(SexpNode::Atom(s)) => Some(s),
                _ => None,
            },
            _ => None,
        }
    }

    /// Find the first child List whose tag matches.
    pub fn find(&self, tag: &str) -> Option<&SexpNode> {
        match self {
            SexpNode::List(children) => children.iter().find(|c| c.tag() == Some(tag)),
            _ => None,
        }
    }

    /// Find all child Lists whose tag matches.
    pub fn find_all(&self, tag: &str) -> Vec<&SexpNode> {
        match self {
            SexpNode::List(children) => children.iter().filter(|c| c.tag() == Some(tag)).collect(),
            _ => vec![],
        }
    }

    /// Get the value atom after a keyword tag.
    /// `(:method POST)` with keyword `:method` → `Some("POST")`
    pub fn keyword(&self, key: &str) -> Option<&str> {
        match self {
            SexpNode::List(children) => {
                for pair in children.windows(2) {
                    if let SexpNode::Atom(k) = &pair[0] {
                        if k == key {
                            if let SexpNode::Atom(v) = &pair[1] {
                                return Some(v);
                            }
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Get a child List node following a keyword.
    /// `(:input (file :max-size "5MB"))` with keyword `:input` → `Some((file ...))`
    pub fn keyword_node(&self, key: &str) -> Option<&SexpNode> {
        match self {
            SexpNode::List(children) => {
                for pair in children.windows(2) {
                    if let SexpNode::Atom(k) = &pair[0] {
                        if k == key {
                            if let node @ SexpNode::List(_) = &pair[1] {
                                return Some(node);
                            }
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }
}

impl fmt::Display for SexpNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SexpNode::Atom(s) => {
                if s.contains(' ') || s.contains('"') || s.is_empty() {
                    write!(f, "\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
                } else {
                    write!(f, "{s}")
                }
            }
            SexpNode::List(items) => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 { write!(f, " ")?; }
                    write!(f, "{item}")?;
                }
                write!(f, ")")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_atom() {
        let nodes = SexpNode::parse("hello").unwrap();
        assert_eq!(nodes, vec![SexpNode::Atom("hello".into())]);
    }

    #[test]
    fn parse_list() {
        let node = SexpNode::parse_one("(api :method POST)").unwrap();
        assert_eq!(node.tag(), Some("api"));
        assert_eq!(node.keyword(":method"), Some("POST"));
    }

    #[test]
    fn parse_nested() {
        let node = SexpNode::parse_one(
            r#"(api :method POST :path "/users" :input (json :schema User))"#
        ).unwrap();
        assert_eq!(node.keyword(":path"), Some("/users"));
        let input = node.keyword_node(":input").unwrap();
        assert_eq!(input.tag(), Some("json"));
    }

    #[test]
    fn parse_comments() {
        let nodes = SexpNode::parse("; this is a comment\n(hello)").unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].tag(), Some("hello"));
    }

    #[test]
    fn error_unmatched() {
        assert!(SexpNode::parse("(hello").is_err());
    }

    #[test]
    fn error_unexpected_close() {
        assert!(SexpNode::parse(")").is_err());
    }

    #[test]
    fn display_roundtrip() {
        let input = r#"(api :method POST :path "/users/me" :auth required)"#;
        let node = SexpNode::parse_one(input).unwrap();
        let output = node.to_string();
        let reparsed = SexpNode::parse_one(&output).unwrap();
        assert_eq!(node, reparsed);
    }
}
