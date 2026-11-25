//! Lexer for MLIR generic form, built on `logos`.

use logos::Logos;

/// Token kinds. Names are already unescaped and unquoted, so the parser never
/// has to look at raw text.
#[derive(Logos, Debug, Clone, PartialEq, Eq)]
#[logos(skip r"[ \t\r\n]+")]
#[logos(skip(r"//[^\n]*", allow_greedy = true))]
pub enum Tok {
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token("=")]
    Eq,
    #[token("->")]
    Arrow,

    /// `-?[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?`
    #[regex(r"-?[0-9]+(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?", |lex| lex.slice().to_owned())]
    Number(String),

    /// `%0`, `%arg0`, or a quoted `%"odd name"`.
    #[regex(r"%[a-zA-Z0-9_.$]+(?:-[a-zA-Z0-9_.$]+)*", |lex| lex.slice()[1..].to_owned())]
    #[regex(r#"%"(?:[^"\\]|\\.)*""#, |lex| sigil_name(lex.slice()))]
    Value(String),
    /// `@main`, or a quoted `@"odd sym"`.
    #[regex(r"@[a-zA-Z0-9_.$]+(?:-[a-zA-Z0-9_.$]+)*", |lex| lex.slice()[1..].to_owned())]
    #[regex(r#"@"(?:[^"\\]|\\.)*""#, |lex| sigil_name(lex.slice()))]
    Symbol(String),
    /// `^bb0`, or a quoted `^"odd block"`.
    #[regex(r"\^[a-zA-Z0-9_.$]+(?:-[a-zA-Z0-9_.$]+)*", |lex| lex.slice()[1..].to_owned())]
    #[regex(r#"\^"(?:[^"\\]|\\.)*""#, |lex| sigil_name(lex.slice()))]
    Block(String),
    /// `#opaque`, or a quoted `#"odd attr"`.
    #[regex(r"#[a-zA-Z0-9_.$]+(?:-[a-zA-Z0-9_.$]+)*", |lex| lex.slice()[1..].to_owned())]
    #[regex(r##"#"(?:[^"\\]|\\.)*""##, |lex| sigil_name(lex.slice()))]
    Attr(String),
    /// `!my.type`, or a quoted `!"odd type"`.
    #[regex(r"![a-zA-Z0-9_.$]+(?:-[a-zA-Z0-9_.$]+)*", |lex| lex.slice()[1..].to_owned())]
    #[regex(r#"!"(?:[^"\\]|\\.)*""#, |lex| sigil_name(lex.slice()))]
    TypeName(String),

    /// A bare `"string"`.
    #[regex(r#""(?:[^"\\]|\\.)*""#, |lex| unquote(&lex.slice()[1..lex.slice().len() - 1]))]
    Str(String),

    /// Bare identifier: `mir.add`, `i32`, `index`, `true`.
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_.$]*(?:-[a-zA-Z0-9_.$]+)*", |lex| lex.slice().to_owned())]
    Ident(String),

    /// Never produced by the lexer; the parser uses it as an end sentinel.
    Eof,
}

/// A token together with its byte span in the source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spanned {
    pub tok: Tok,
    pub start: usize,
    pub end: usize,
}

impl Spanned {
    pub fn eof(at: usize) -> Self {
        Self {
            tok: Tok::Eof,
            start: at,
            end: at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub message: String,
    pub start: usize,
    pub end: usize,
}

pub fn lex(text: &str) -> Result<Vec<Spanned>, LexError> {
    let mut lexer = Tok::lexer(text);
    let mut out = Vec::new();
    while let Some(item) = lexer.next() {
        let span = lexer.span();
        match item {
            Ok(tok) => out.push(Spanned {
                tok,
                start: span.start,
                end: span.end,
            }),
            Err(()) => {
                return Err(LexError {
                    message: format!("unexpected token {:?}", &text[span.start..span.end]),
                    start: span.start,
                    end: span.end,
                });
            }
        }
    }
    Ok(out)
}

/// Strip the sigil and the quotes from `%"name"` and unescape the name.
fn sigil_name(slice: &str) -> String {
    let sigil_len = slice
        .chars()
        .next()
        .expect("sigil token is non-empty")
        .len_utf8();
    unquote(&slice[sigil_len + 1..slice.len() - 1])
}

/// Unescape the body of a quoted name (the surrounding quotes are stripped).
fn unquote(raw: &str) -> String {
    let chars: Vec<char> = raw.chars().collect();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        i += 1;
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let Some(&escaped) = chars.get(i) else { break };
        i += 1;
        match escaped {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            '0' => out.push('\0'),
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            'u' if chars.get(i) == Some(&'{') => {
                // `\u{1f600}`; anything malformed falls back to the raw text.
                let hex: String = chars[i + 1..].iter().take_while(|ch| **ch != '}').collect();
                let closed = chars.get(i + 1 + hex.chars().count()) == Some(&'}');
                match u32::from_str_radix(&hex, 16)
                    .ok()
                    .and_then(char::from_u32)
                    .filter(|_| closed && !hex.is_empty())
                {
                    Some(value) => {
                        out.push(value);
                        i += hex.chars().count() + 2;
                    }
                    None => out.push_str("\\u"),
                }
            }
            other => out.push(other),
        }
    }
    out
}
