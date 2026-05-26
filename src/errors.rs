//! Translate apollo-parser errors into `GraphQL\Error\SyntaxError`.

use apollo_parser::Error as AppError;
use ext_php_rs::exception::PhpException;
use ext_php_rs::types::Zval;
use ext_php_rs::zend::{ce, ClassEntry};

/// Throw a `SyntaxError` with PHP-compatible wording. The PHP class lives in
/// `GraphQL\Error\SyntaxError` and is autoloaded by Composer.
///
/// Implementation note: we delegate to `PhpException::new` so PHP's runtime
/// invokes the SyntaxError constructor via `zend_throw_exception_ex`, the
/// same path used everywhere else. SyntaxError's ctor expects `(Source, int,
/// string)`, but `zend_throw_exception_ex` only takes a message; we
/// therefore throw a plain `\GraphQL\Error\Error` (parent class) and pre-set
/// its `$message`. Source/position metadata is attached **after** the
/// `throw`, on `EG(exception)`.
pub fn throw_syntax_error(source_zv: &Zval, position: i64, description: &str) {
    let ce: &'static ClassEntry = ClassEntry::try_find("GraphQL\\Error\\SyntaxError")
        .unwrap_or_else(|| ce::exception());
    let message = format!("Syntax Error: {description}");
    let _ = PhpException::new(message, 0, ce).throw();

    // Initialize the Error parent's typed properties on the just-thrown
    // exception, scoped to the Error class so private/protected fields
    // accept the write. `zend_throw_exception_ex` bypasses the constructor,
    // leaving these uninitialized — `FormattedError::printSourceLocation`
    // would otherwise hit `Typed property ... must not be accessed before
    // initialization`.
    unsafe {
        let eg = ext_php_rs::zend::ExecutorGlobals::get();
        let exc_ptr = eg.exception;
        if exc_ptr.is_null() {
            return;
        }
        let exc = &mut *exc_ptr;
        let error_ce: *mut ClassEntry = ClassEntry::try_find("GraphQL\\Error\\Error")
            .map(|c| c as *const _ as *mut ClassEntry)
            .unwrap_or(std::ptr::null_mut());

        write_object_property(exc, error_ce, "nodes", PropValue::Null);
        write_object_property(exc, error_ce, "source", PropValue::Zval(source_zv.shallow_clone()));
        let mut pos_ht = ext_php_rs::types::ZendHashTable::new();
        let _ = pos_ht.push(position);
        let mut pos_zv = Zval::new();
        pos_zv.set_hashtable(pos_ht);
        write_object_property(exc, error_ce, "positions", PropValue::Zval(pos_zv));
        write_object_property(exc, error_ce, "path", PropValue::Null);
        write_object_property(exc, error_ce, "unaliasedPath", PropValue::Null);
        write_object_property(exc, error_ce, "extensions", PropValue::Null);
        // SyntaxError without a previous exception is client-safe (the parser
        // is reading user input — its messages never leak server internals).
        // Matches `Error::__construct`'s `$previous === null` branch.
        write_object_property(exc, error_ce, "isClientSafe", PropValue::Bool(true));
    }
}

enum PropValue {
    Null,
    Bool(bool),
    Zval(Zval),
}

unsafe fn write_object_property(
    obj: &mut ext_php_rs::types::ZendObject,
    scope: *mut ClassEntry,
    name: &str,
    value: PropValue,
) {
    let name_cstr = match std::ffi::CString::new(name) {
        Ok(c) => c,
        Err(_) => return,
    };
    let mut zv = match value {
        PropValue::Null => Zval::new(),
        PropValue::Bool(b) => {
            let mut z = Zval::new();
            z.set_bool(b);
            z
        }
        PropValue::Zval(z) => z,
    };
    extern "C" {
        fn zend_update_property(
            scope: *mut ClassEntry,
            object: *mut ext_php_rs::types::ZendObject,
            name: *const std::ffi::c_char,
            name_length: usize,
            value: *mut Zval,
        );
    }
    zend_update_property(
        scope,
        obj as *mut _,
        name_cstr.as_ptr(),
        name.len(),
        &mut zv as *mut Zval,
    );
}

/// Convert the first apollo-parser error into a description string that
/// matches the graphql-php Parser's wording where feasible. The full input
/// body is passed in so we can produce graphql-php-style "Unexpected X" /
/// "Expected X, found Y" messages by looking at the token at the error site.
pub fn first_error_description(err: &AppError, recursion_limit: usize, body: &str) -> String {
    let raw = err.message();
    let lower = raw.to_ascii_lowercase();
    if lower.contains("recursion") {
        return format!("Recursion depth limit of {recursion_limit} exceeded");
    }

    let offset = err.index();
    let at_site = peek_token(body, offset);
    // Strip apollo's single-quote wrappers from the message so our pattern
    // matches below see "expected on" instead of "expected 'on'".
    let stripped: String = lower.chars().filter(|c| *c != '\'' && *c != '"').collect();
    let lower_stripped = stripped.as_str();

    // Specific apollo wordings → graphql-php wordings.
    // "Expected [an? ] Implements Interface[s]?, Directives,? or a Fields Definition"
    // covers the various "extend type X" with no body diagnostics across
    // apollo-parser versions.
    if lower_stripped.contains("implements interface")
        && lower_stripped.contains("fields definition")
        && lower_stripped.starts_with("expected")
    {
        return format!(
            "Unexpected {}",
            at_site.clone().unwrap_or_else(|| "<EOF>".to_string())
        );
    }
    if lower.contains("expected a selection set") {
        // `query`, `mutation`, etc. without a body — graphql-php expects `{`.
        return format!(
            "Expected {{, found {}",
            at_site.clone().unwrap_or_else(|| "<EOF>".to_string())
        );
    }
    if lower.contains("at least one selection in selection set")
        || lower.contains("expected union member type")
        || lower.contains("expected field definition")
        || lower.contains("expected directive location")
        || lower.contains("expected a value")
        || lower.contains("expected name in type condition")
        || lower.contains("expected a name in type condition")
    {
        return format!(
            "Expected Name, found {}",
            at_site.clone().unwrap_or_else(|| "<EOF>".to_string())
        );
    }
    if lower_stripped.contains("fragment name cannot be on") {
        return "Unexpected Name \"on\"".to_string();
    }
    if lower_stripped.contains("invalid type system extension") {
        // Two flavours:
        //   1. `"Description" extend …` → "Unexpected Name \"extend\""
        //   2. `extend "Description" …` → "Unexpected String \"Description\""
        if let Some(ext_pos) = body.find("extend") {
            let mut i = ext_pos + "extend".len();
            while i < body.len() && body.as_bytes()[i] == b' ' {
                i += 1;
            }
            if i < body.len() && body.as_bytes()[i] == b'"' {
                // Find the closing quote.
                let rest = &body[i + 1..];
                if let Some(end) = rest.find('"') {
                    let desc = &rest[..end];
                    return format!("Unexpected String \"{desc}\"");
                }
            }
        }
        return "Unexpected Name \"extend\"".to_string();
    }
    if lower_stripped.contains("unexpected variable value in a const context") {
        return "Unexpected $".to_string();
    }
    // `{f1` (open brace + name + EOF) — apollo says "expected R_CURLY, got EOF",
    // graphql-php says "Expected Name, found <EOF>" because in the on-disk
    // parser, after consuming a field name the next token must be `}` OR
    // another Name (start of next selection). At EOF inside an unclosed
    // selection set, PHP picks the "another Name" branch.
    if lower_stripped == "expected r_curly, got eof" {
        return "Expected Name, found <EOF>".to_string();
    }
    if lower_stripped.contains("valid directive location") {
        return format!(
            "Unexpected {}",
            at_site.clone().unwrap_or_else(|| "<EOF>".to_string())
        );
    }
    // Input-object args case: `input X { f(arg: Int) }` — graphql-php
    // detects this as "Expected :, found (" because it had just parsed a
    // Name and was looking for `:` next.
    if lower_stripped.starts_with("expected definition")
        && at_site.as_deref() == Some("(")
    {
        return "Expected :, found (".to_string();
    }
    if lower.contains("a stringvalue, name or operationdefinition")
        || lower.contains("a stringvalue, name, or operationdefinition")
        || lower.contains("expected an enum value definition")
    {
        return format!(
            "Unexpected {}",
            at_site.clone().unwrap_or_else(|| "<EOF>".to_string())
        );
    }
    if lower.contains("expected an implements interfaces, directives, or a fields definition")
        || lower.contains("expected implements interface, directives or a fields definition")
        || lower.contains("exptected an implements interfaces, directives, or a fields definition")
        || lower.contains("expected directives, or a fields definition")
    {
        return format!(
            "Unexpected {}",
            at_site.clone().unwrap_or_else(|| "<EOF>".to_string())
        );
    }
    if lower.starts_with("expected definition") {
        return format!(
            "Unexpected {}",
            at_site.clone().unwrap_or_else(|| "<EOF>".to_string())
        );
    }
    // `input X { f(arg: Int) }` — apollo says "expected a Name" at the `(`;
    // graphql-php expects "Expected :, found (" (it had a Name and was
    // looking for `:`).
    if (lower_stripped.starts_with("expected a name") || lower_stripped.starts_with("expected name"))
        && at_site.as_deref() == Some("(")
    {
        return "Expected :, found (".to_string();
    }

    if let Some(after) = lower.strip_prefix("expected ") {
        let (expected_part, got_part) = match after.find(", got ") {
            Some(i) => (&after[..i], Some(after[i + 6..].trim_matches('\''))),
            None => (after, None),
        };
        let expected = format_expected(expected_part);
        let found = got_part
            .map(|s| format_found_apollo(s))
            .unwrap_or_else(|| at_site.clone().unwrap_or_else(|| "<EOF>".to_string()));
        return format!("Expected {expected}, found {found}");
    }
    if lower.starts_with("unexpected") {
        let mut s = raw.to_string();
        if let Some(c) = s.chars().next() {
            if c.is_lowercase() {
                s = c.to_uppercase().chain(s.chars().skip(1)).collect();
            }
        }
        return s;
    }

    let mut s = String::with_capacity(raw.len() + 16);
    let mut chars = raw.chars();
    if let Some(first) = chars.next() {
        for c in first.to_uppercase() {
            s.push(c);
        }
    }
    for c in chars {
        if c == '\'' {
            continue;
        }
        s.push(c);
    }
    s.replace("got ", "found ")
}

/// Inspect the byte at the error position and synthesize a one-token preview
/// that mimics `Token::getDescription()` in the PHP Lexer.
fn peek_token(body: &str, offset: usize) -> Option<String> {
    let bytes = body.as_bytes();
    if offset >= bytes.len() {
        return Some("<EOF>".to_string());
    }
    let b = bytes[offset];
    let one = match b {
        b'{' => Some("{"),
        b'}' => Some("}"),
        b'(' => Some("("),
        b')' => Some(")"),
        b'[' => Some("["),
        b']' => Some("]"),
        b':' => Some(":"),
        b'=' => Some("="),
        b'@' => Some("@"),
        b'$' => Some("$"),
        b'!' => Some("!"),
        b'&' => Some("&"),
        b'|' => Some("|"),
        b'.' => Some("..."),
        _ => None,
    };
    if let Some(s) = one {
        return Some(s.to_string());
    }
    // Try Name / number / string
    if b.is_ascii_alphabetic() || b == b'_' {
        let mut end = offset + 1;
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
            end += 1;
        }
        let name = std::str::from_utf8(&bytes[offset..end]).ok()?;
        return Some(format!("Name \"{name}\""));
    }
    if b.is_ascii_digit() || b == b'-' {
        // Naïve number capture.
        let mut end = offset + 1;
        while end < bytes.len()
            && (bytes[end].is_ascii_digit() || bytes[end] == b'.' || bytes[end] == b'-'
                || bytes[end] == b'+' || bytes[end] == b'e' || bytes[end] == b'E')
        {
            end += 1;
        }
        let num = std::str::from_utf8(&bytes[offset..end]).ok()?;
        return Some(format!("Int \"{num}\""));
    }
    if b == b'"' {
        // Capture up to closing quote (no escape handling — Phase 2 best-effort).
        let mut end = offset + 1;
        while end < bytes.len() && bytes[end] != b'"' {
            if bytes[end] == b'\\' && end + 1 < bytes.len() {
                end += 2;
            } else {
                end += 1;
            }
        }
        let lit = std::str::from_utf8(&bytes[offset..end.min(bytes.len())]).ok()?;
        return Some(format!("String {lit}\""));
    }
    None
}

fn format_expected(raw: &str) -> String {
    // apollo: "R_CURLY" / "NAME" / "'on'" / "a name"
    let r = raw.trim().trim_matches('\'');
    // "a name" / "a Name" / "a operation"
    let stripped = r.strip_prefix("a ").unwrap_or(r);
    let stripped = stripped.strip_prefix("an ").unwrap_or(stripped);
    match stripped.to_ascii_lowercase().as_str() {
        "name" => "Name".to_string(),
        "r_curly" => "}".to_string(),
        "l_curly" => "{".to_string(),
        "r_paren" => ")".to_string(),
        "l_paren" => "(".to_string(),
        "r_brack" => "]".to_string(),
        "l_brack" => "[".to_string(),
        "colon" => ":".to_string(),
        _ => format!("\"{stripped}\""),
    }
}

fn format_found_apollo(s: &str) -> String {
    match s.trim().to_ascii_uppercase().as_str() {
        "EOF" => "<EOF>".to_string(),
        _ => s.trim().to_string(),
    }
}

/// Locate the byte offset of the first error.
pub fn first_error_offset(err: &AppError) -> usize {
    err.index()
}

/// Override the reported byte offset for specific apollo errors. Used when
/// the graphql-php location convention differs from apollo-parser's
/// (e.g. description-followed-by-extend, where apollo points at the
/// description and graphql-php points at `extend`).
pub fn override_offset(err: &AppError, body: &str) -> Option<usize> {
    let lower = err.message().to_ascii_lowercase();
    if lower.contains("invalid type system extension") {
        // Case 1: `extend ...` preceded by a description → point at `extend`.
        // Case 2: `extend "..."` → point at the description string.
        if let Some(ext_pos) = body.find("extend") {
            // Look at what comes after `extend ` (whitespace then ...).
            let mut i = ext_pos + "extend".len();
            while i < body.len() && body.as_bytes()[i] == b' ' {
                i += 1;
            }
            if i < body.len() && body.as_bytes()[i] == b'"' {
                return Some(i);
            }
            return Some(ext_pos);
        }
    }
    None
}
