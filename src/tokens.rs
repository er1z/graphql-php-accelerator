//! Build a doubly-linked chain of `GraphQL\Language\Token` instances for a
//! parsed source. Used by `Parser::parse` to populate
//! `Location::$startToken` / `Location::$endToken` on the document root.
//!
//! Comments are included in the chain (matching the on-disk Lexer's
//! linked-list); whitespace and commas are skipped.

use apollo_parser::{Lexer, TokenKind};
use ext_php_rs::rc::PhpRc;
use ext_php_rs::types::{ZendObject, Zval};
use ext_php_rs::zend::ClassEntry;

pub struct ChainEnds {
    pub sof: Zval,
    pub eof: Zval,
}

pub fn build_chain(body: &str, line_starts: &[usize]) -> Option<ChainEnds> {
    let token_ce = ClassEntry::try_find("GraphQL\\Language\\Token")?;
    let (raw_tokens, _errors) = Lexer::new(body).lex();

    let line_col = |offset: usize| -> (i64, i64) {
        let line = match line_starts.binary_search(&offset) {
            Ok(i) => i + 1,
            Err(i) => i,
        };
        let line_start = line_starts.get(line.saturating_sub(1)).copied().unwrap_or(0);
        let col = body[line_start..offset.min(body.len())].chars().count() as i64 + 1;
        (line as i64, col)
    };

    let (sof_line, sof_col) = line_col(0);
    let sof = make_token(token_ce, "<SOF>", 0, 0, sof_line, sof_col, None, None);
    let sof_handle = pointer_zval(sof);

    let mut prev_obj: *mut ZendObject = sof;
    let mut last_eof_offset = body.len();

    for tok in &raw_tokens {
        let kind = tok.kind();
        let data = tok.data();
        let start = tok.index();
        let end = start + data.len();
        let (line, col) = line_col(start);

        let (kind_str, value): (&str, Option<String>) = match kind {
            TokenKind::Whitespace | TokenKind::Comma => continue,
            TokenKind::Comment => {
                let trimmed = data.strip_prefix('#').unwrap_or(data);
                let trimmed = trimmed.strip_prefix(' ').unwrap_or(trimmed);
                ("Comment", Some(trimmed.to_string()))
            }
            TokenKind::Bang => ("!", None),
            TokenKind::Dollar => ("$", None),
            TokenKind::Amp => ("&", None),
            TokenKind::Spread => ("...", None),
            TokenKind::Colon => (":", None),
            TokenKind::Eq => ("=", None),
            TokenKind::At => ("@", None),
            TokenKind::LParen => ("(", None),
            TokenKind::RParen => (")", None),
            TokenKind::LBracket => ("[", None),
            TokenKind::RBracket => ("]", None),
            TokenKind::LCurly => ("{", None),
            TokenKind::RCurly => ("}", None),
            TokenKind::Pipe => ("|", None),
            TokenKind::Eof => {
                last_eof_offset = start;
                break;
            }
            TokenKind::Name => ("Name", Some(data.to_string())),
            TokenKind::StringValue => {
                if data.starts_with("\"\"\"") {
                    ("BlockString", Some(strip_block_string(data)))
                } else {
                    ("String", Some(strip_simple_string(data)))
                }
            }
            TokenKind::Int => ("Int", Some(data.to_string())),
            TokenKind::Float => ("Float", Some(data.to_string())),
        };

        let new_obj = make_token(
            token_ce,
            kind_str,
            start as i64,
            end as i64,
            line,
            col,
            Some(pointer_zval(prev_obj)),
            value,
        );
        unsafe {
            let _ = (*prev_obj).set_property("next", pointer_zval(new_obj));
        }
        prev_obj = new_obj;
    }

    let (eof_line, eof_col) = line_col(last_eof_offset);
    let eof = make_token(
        token_ce,
        "<EOF>",
        last_eof_offset as i64,
        last_eof_offset as i64,
        eof_line,
        eof_col,
        Some(pointer_zval(prev_obj)),
        None,
    );
    unsafe {
        let _ = (*prev_obj).set_property("next", pointer_zval(eof));
    }

    Some(ChainEnds {
        sof: sof_handle,
        eof: pointer_zval(eof),
    })
}

/// Build a `Token` ZendObject. Returns a raw `*mut` whose refcount is **0**
/// after this call — the caller is responsible for bumping it (via
/// `pointer_zval`, which `set_object`s the pointer into a Zval and
/// increments to 1). This mirrors what `<ZBox<ZendObject> as IntoZval>` does
/// and keeps refcount accounting balanced so the cyclic prev/next chain
/// drops cleanly when the document goes away.
fn make_token(
    ce: &'static ClassEntry,
    kind: &str,
    start: i64,
    end: i64,
    line: i64,
    column: i64,
    prev_zv: Option<Zval>,
    value: Option<String>,
) -> *mut ZendObject {
    let mut obj = ZendObject::new(ce);
    let _ = obj.set_property("kind", kind);
    let _ = obj.set_property("start", start);
    let _ = obj.set_property("end", end);
    let _ = obj.set_property("line", line);
    let _ = obj.set_property("column", column);
    let _ = obj.set_property("prev", prev_zv.unwrap_or_else(Zval::new));
    let _ = obj.set_property("next", Zval::new());
    match value {
        Some(v) => {
            let _ = obj.set_property("value", v);
        }
        None => {
            let _ = obj.set_property("value", Zval::new());
        }
    }
    // ZendObject::new lands at refcount=1 (the "owner" being the ZBox we're
    // about to leak). Drop that initial ref so subsequent `pointer_zval` +
    // `set_property` calls bring the count up to exactly what the chain
    // needs — without that, every Token leaks 1 ref per parse and the
    // doubly-linked-list cycle never collects.
    let raw = obj.into_raw();
    raw.dec_count();
    raw as *mut _
}

/// Wrap a `*mut ZendObject` into a Zval, bumping the object's refcount so the
/// Zval owns one reference.
fn pointer_zval(obj: *mut ZendObject) -> Zval {
    let mut zv = Zval::new();
    unsafe {
        zv.set_object(&mut *obj);
    }
    zv
}

fn strip_simple_string(raw: &str) -> String {
    if !raw.starts_with('"') || !raw.ends_with('"') || raw.len() < 2 {
        return raw.to_string();
    }
    let inner = &raw[1..raw.len() - 1];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('/') => out.push('/'),
            Some('b') => out.push('\u{0008}'),
            Some('f') => out.push('\u{000C}'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                if let Ok(code) = u32::from_str_radix(&hex, 16) {
                    if let Some(c) = char::from_u32(code) {
                        out.push(c);
                    }
                }
            }
            Some(o) => {
                out.push('\\');
                out.push(o);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn strip_block_string(raw: &str) -> String {
    if raw.len() < 6 || !raw.starts_with("\"\"\"") || !raw.ends_with("\"\"\"") {
        return raw.to_string();
    }
    let body = &raw[3..raw.len() - 3];
    let unescaped = body.replace("\\\"\"\"", "\"\"\"");
    let lines: Vec<&str> = unescaped.split('\n').collect();
    let mut common: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            continue;
        }
        let stripped = line.trim_start();
        if stripped.is_empty() {
            continue;
        }
        let indent = line.len() - stripped.len();
        common = Some(common.map_or(indent, |c| c.min(indent)));
    }
    let dedent = common.unwrap_or(0);
    let mut result: Vec<String> = lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            if i == 0 || dedent == 0 || l.len() < dedent {
                (*l).to_string()
            } else {
                l[dedent..].to_string()
            }
        })
        .collect();
    while result.first().map(|l| l.trim().is_empty()).unwrap_or(false) {
        result.remove(0);
    }
    while result.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        result.pop();
    }
    result.join("\n")
}
