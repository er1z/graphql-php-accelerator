//! Lower GraphQL value literals.

use apollo_parser::cst as cst;
use apollo_parser::cst::CstNode;
use ext_php_rs::types::Zval;

use crate::classes::slots;
use super::helpers::*;
use super::LowerCtx;

pub fn lower_value(v: &cst::Value, ctx: &LowerCtx) -> Zval {
    use cst::Value::*;
    let range = v.syntax().text_range();
    match v {
        Variable(var) => {
            let name = variable_name_node(var, ctx);
            new_node(slots::variable_node::get(), range, ctx)
                .prop("name", name)
                .finish()
        }
        IntValue(n) => {
            let txt = token_text(n.syntax());
            new_node(slots::int_value_node::get(), range, ctx)
                .prop_str("value", &txt)
                .finish()
        }
        FloatValue(n) => {
            let txt = token_text(n.syntax());
            new_node(slots::float_value_node::get(), range, ctx)
                .prop_str("value", &txt)
                .finish()
        }
        StringValue(s) => {
            let (value, is_block) = decode_string_value(s);
            new_node(slots::string_value_node::get(), range, ctx)
                .prop_str("value", &value)
                .prop_bool("block", is_block)
                .finish()
        }
        BooleanValue(b) => {
            let txt = token_text(b.syntax());
            let val = txt.trim() == "true";
            let mut bool_zv = Zval::new();
            bool_zv.set_bool(val);
            new_node(slots::boolean_value_node::get(), range, ctx)
                .prop("value", bool_zv)
                .finish()
        }
        NullValue(_) => {
            new_node(slots::null_value_node::get(), range, ctx).finish()
        }
        EnumValue(e) => {
            let txt = e
                .name()
                .map(|n| n.text().to_string())
                .unwrap_or_default();
            new_node(slots::enum_value_node::get(), range, ctx)
                .prop_str("value", &txt)
                .finish()
        }
        ListValue(lv) => {
            let mut items: Vec<Zval> = Vec::new();
            for it in lv.values() {
                items.push(lower_value(&it, ctx));
            }
            let values = node_list_from_vec(items);
            new_node(slots::list_value_node::get(), range, ctx)
                .prop("values", values)
                .finish()
        }
        ObjectValue(ov) => {
            let mut items: Vec<Zval> = Vec::new();
            for f in ov.object_fields() {
                items.push(lower_object_field(&f, ctx));
            }
            let fields = node_list_from_vec(items);
            new_node(slots::object_value_node::get(), range, ctx)
                .prop("fields", fields)
                .finish()
        }
    }
}

fn lower_object_field(f: &cst::ObjectField, ctx: &LowerCtx) -> Zval {
    let name = f.name().map(|n| name_node(&n, ctx)).unwrap_or(Zval::new());
    let value = f
        .value()
        .map(|v| lower_value(&v, ctx))
        .unwrap_or(Zval::new());
    new_node(slots::object_field_node::get(), f.syntax().text_range(), ctx)
        .prop("name", name)
        .prop("value", value)
        .finish()
}

fn token_text(node: &apollo_parser::SyntaxNode) -> String {
    node.text().to_string()
}

/// Extract the actual string contents of a StringValue token, stripping the
/// surrounding quotes and applying GraphQL escape-sequence rules.
fn decode_string_value(s: &cst::StringValue) -> (String, bool) {
    let raw = s.syntax().text().to_string();
    if raw.starts_with("\"\"\"") && raw.ends_with("\"\"\"") && raw.len() >= 6 {
        // Block string: strip outer `"""`, apply the lone escape `\"""` →
        // `"""`, then dedent per the spec.
        let body = &raw[3..raw.len() - 3];
        let unescaped = body.replace("\\\"\"\"", "\"\"\"");
        let dedented = dedent_block(&unescaped);
        (dedented, true)
    } else if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
        let inner = &raw[1..raw.len() - 1];
        (decode_escapes(inner), false)
    } else {
        (raw, false)
    }
}

fn decode_escapes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut iter = s.chars();
    while let Some(c) = iter.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match iter.next() {
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('/') => out.push('/'),
            Some('b') => out.push('\u{0008}'),
            Some('f') => out.push('\u{000C}'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('u') => {
                let hex: String = iter.by_ref().take(4).collect();
                if let Ok(code) = u32::from_str_radix(&hex, 16) {
                    if let Some(c) = char::from_u32(code) {
                        out.push(c);
                    }
                }
            }
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Minimal BlockString dedent — matches graphql-js semantics enough for the
/// majority of tests. Phase 4 polish will tighten the edge cases.
fn dedent_block(s: &str) -> String {
    let lines: Vec<&str> = s.split('\n').collect();
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
