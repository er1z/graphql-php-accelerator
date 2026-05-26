//! `GraphQL\Language\Parser` — the entry point. Wired through apollo-parser.

use ext_php_rs::builders::{ClassBuilder, FunctionBuilder};
use ext_php_rs::error::Result;
use ext_php_rs::exception::PhpException;
use ext_php_rs::flags::{ClassFlags, DataType, MethodFlags};
use ext_php_rs::types::Zval;
use ext_php_rs::zend::{ce, ClassEntry, ExecuteData};

use super::slots;
use crate::errors;
use crate::lower::{lower_document, lower_type, lower_value, LowerCtx};
use crate::options::{ParserOptions, DEFAULT_RECURSION_LIMIT};
use crate::source::coerce_source;

const RECURSION_LIMIT: i64 = DEFAULT_RECURSION_LIMIT as i64;

pub fn register() -> Result<()> {
    ClassBuilder::new(slots::parser::PHP_NAME)
        .flags(ClassFlags::Final)
        .constant("DEFAULT_RECURSION_LIMIT", RECURSION_LIMIT, &[])?
        .method(
            FunctionBuilder::new("parse", parse)
                .returns(DataType::Object(None), false, false),
            MethodFlags::Public | MethodFlags::Static,
        )
        .method(
            FunctionBuilder::new("parseValue", parse_value)
                .returns(DataType::Mixed, false, true),
            MethodFlags::Public | MethodFlags::Static,
        )
        .method(
            FunctionBuilder::new("parseType", parse_type)
                .returns(DataType::Mixed, false, true),
            MethodFlags::Public | MethodFlags::Static,
        )
        .method(
            FunctionBuilder::new("__callStatic", call_static)
                .arg(ext_php_rs::args::Arg::new("name", DataType::String))
                .arg(ext_php_rs::args::Arg::new("arguments", DataType::Array)),
            MethodFlags::Public | MethodFlags::Static,
        )
        .registration(slots::parser::set)
        .register()
}

extern "C" fn parse(ex: &mut ExecuteData, retval: &mut Zval) {
    let (source_zv, options_zv) = match read_args(ex) {
        Some(v) => v,
        None => return,
    };
    let opts = match ParserOptions::from_zval(options_zv) {
        Ok(o) => o,
        Err(_) => {
            throw_runtime("invalid options array");
            return;
        }
    };
    let mut src = match coerce_source(source_zv) {
        Ok(s) => s,
        Err(msg) => {
            throw_runtime(msg);
            return;
        }
    };
    if opts.allow_legacy_sdl_empty_fields {
        src.body = preprocess_empty_sdl_braces(&src.body);
    }
    if opts.allow_legacy_sdl_implements_interfaces {
        src.body = preprocess_legacy_implements(&src.body);
    }
    // Capture fragment variable definitions when the user explicitly opts
    // in via `experimentalFragmentVariables`.
    let fragment_vars = if opts.experimental_fragment_variables {
        let (cleaned, vars) = extract_fragment_variables(&src.body);
        src.body = cleaned;
        vars
    } else {
        Vec::new()
    };
    let tree = apollo_parser::Parser::new(&src.body)
        .recursion_limit(opts.apollo_recursion_limit())
        .parse();
    if let Some(err) = tree.errors().next() {
        let offset = errors::override_offset(err, &src.body)
            .unwrap_or_else(|| errors::first_error_offset(err));
        errors::throw_syntax_error(
            &src.source_zv,
            offset as i64,
            &errors::first_error_description(err, opts.apollo_recursion_limit(), &src.body),
        );
        return;
    }
    let ctx = LowerCtx::new(src.body.clone(), src.source_zv, opts);
    let mut zv = lower_document(&tree, &ctx);
    // Attach captured fragment variable definitions to matching
    // FragmentDefinitionNode entries.
    if !fragment_vars.is_empty() {
        attach_fragment_variables(&mut zv, &fragment_vars, &ctx);
    }
    // Build the Token chain and attach <SOF>/<EOF> to the document's loc.
    if !opts.no_location {
        if let Some(ends) = crate::tokens::build_chain(&ctx.body, &ctx.line_starts) {
            attach_chain_to_document(&mut zv, ends);
        }
    }
    *retval = zv;
}

fn attach_chain_to_document(doc_zv: &mut Zval, ends: crate::tokens::ChainEnds) {
    let Some(doc_obj) = doc_zv.object_mut() else { return };
    let loc: Zval = match doc_obj.get_property::<&Zval>("loc") {
        Ok(v) => v.shallow_clone(),
        Err(_) => return,
    };
    if loc.is_null() {
        return;
    }
    let mut loc_clone = loc;
    let Some(loc_obj) = loc_clone.object_mut() else { return };
    let _ = loc_obj.set_property("startToken", ends.sof);
    let _ = loc_obj.set_property("endToken", ends.eof);
}

/// Scan the source for `fragment <name>(<vars>)` patterns. For each match,
/// capture the var-defs text and rewrite the source to omit the parens.
/// Returns the cleaned source and a list of (name, vars-source) tuples.
fn extract_fragment_variables(src: &str) -> (String, Vec<(String, String)>) {
    let mut out = String::with_capacity(src.len());
    let mut vars: Vec<(String, String)> = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Look for the keyword `fragment` at word-start positions.
        if matches_word(bytes, i, b"fragment") {
            out.push_str("fragment");
            let mut j = i + b"fragment".len();
            // skip whitespace
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            // capture the fragment name
            let name_start = j;
            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                j += 1;
            }
            let name = String::from_utf8_lossy(&bytes[name_start..j]).to_string();
            // copy whitespace + name to out
            out.push_str(&src[i + b"fragment".len()..j]);
            // skip whitespace before `(`
            let mut k = j;
            while k < bytes.len() && bytes[k].is_ascii_whitespace() {
                k += 1;
            }
            if k < bytes.len() && bytes[k] == b'(' {
                // Find matching `)`. Naive: ignore nested parens (vars don't
                // contain `(` except inside string literals).
                let mut depth = 0;
                let mut l = k;
                while l < bytes.len() {
                    match bytes[l] {
                        b'(' => depth += 1,
                        b')' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    l += 1;
                }
                if l < bytes.len() && bytes[l] == b')' {
                    // Capture between k+1 and l
                    let vars_text = String::from_utf8_lossy(&bytes[k + 1..l]).to_string();
                    vars.push((name, vars_text));
                    // Replace `(...)` with a single space in out.
                    out.push(' ');
                    i = l + 1;
                    continue;
                }
            }
            i = j;
            continue;
        }
        // Otherwise pass the byte through.
        out.push(bytes[i] as char);
        i += 1;
    }
    (out, vars)
}

fn matches_word(bytes: &[u8], i: usize, word: &[u8]) -> bool {
    if i + word.len() > bytes.len() {
        return false;
    }
    if &bytes[i..i + word.len()] != word {
        return false;
    }
    // Word boundary check: prev char (if any) must not be alnum/_; next char must not be alnum/_
    if i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_') {
        return false;
    }
    let after = i + word.len();
    if after < bytes.len()
        && (bytes[after].is_ascii_alphanumeric() || bytes[after] == b'_')
    {
        return false;
    }
    true
}

/// Walk the lowered DocumentNode and attach captured variable definitions to
/// each matching FragmentDefinitionNode.
fn attach_fragment_variables(
    doc_zv: &mut Zval,
    vars: &[(String, String)],
    _ctx: &LowerCtx,
) {
    let Some(doc) = doc_zv.object() else { return };
    let definitions: Zval = match doc.get_property::<&Zval>("definitions") {
        Ok(v) => v.shallow_clone(),
        Err(_) => return,
    };
    let Some(defs_obj) = definitions.object() else { return };
    // Iterate the NodeList by integer index.
    let count = defs_obj
        .try_call_method("count", vec![])
        .ok()
        .and_then(|z| z.long())
        .unwrap_or(0);
    for i in 0..count {
        let mut idx_zv = Zval::new();
        idx_zv.set_long(i);
        let Ok(item) = defs_obj.try_call_method("offsetGet", vec![&idx_zv]) else {
            continue;
        };
        let Some(item_obj) = item.object() else { continue };
        let kind: String = match item_obj.get_property::<String>("kind") {
            Ok(k) => k,
            Err(_) => continue,
        };
        if kind != "FragmentDefinition" {
            continue;
        }
        let name_zv = match item_obj.get_property::<&Zval>("name") {
            Ok(v) => v.shallow_clone(),
            Err(_) => continue,
        };
        let Some(name_obj) = name_zv.object() else { continue };
        let name_val: String = match name_obj.get_property::<String>("value") {
            Ok(v) => v,
            Err(_) => continue,
        };
        // Find the var-defs for this fragment.
        let Some((_, vars_text)) = vars.iter().find(|(n, _)| n == &name_val) else {
            continue;
        };
        let parsed = parse_variable_definitions(vars_text);
        if let Some(vd_zv) = parsed {
            // We can't mutate item_obj through the borrow chain easily —
            // wrap a fresh shallow_clone, set property on the cloned ref.
            let mut item_clone = item.shallow_clone();
            if let Some(item_mut) = item_clone.object_mut() {
                let _ = item_mut.set_property("variableDefinitions", vd_zv);
            }
        }
    }
}

/// Parse a piece of source like `$v: Boolean = false` as a NodeList of
/// VariableDefinitionNode. We wrap it as `query (<vars>) { __t }` so apollo
/// recognises the form, then pull `variable_definitions` off the operation.
fn parse_variable_definitions(vars_text: &str) -> Option<Zval> {
    let wrapped = format!("query ({}) {{ __t }}", vars_text);
    let prefix_offset = "query (".len();
    let tree = apollo_parser::Parser::new(&wrapped)
        .recursion_limit(crate::options::DEFAULT_RECURSION_LIMIT)
        .parse();
    if tree.errors().next().is_some() {
        return None;
    }
    let doc = tree.document();
    let op = doc.definitions().find_map(|d| match d {
        apollo_parser::cst::Definition::OperationDefinition(op) => Some(op),
        _ => None,
    })?;
    let vds = op.variable_definitions()?;
    // Build a dummy LowerCtx without a real source — Locations get the
    // synthetic prefix subtracted so they point into the original source.
    let ctx = LowerCtx::new(
        vars_text.to_string(),
        ext_php_rs::types::Zval::new(),
        crate::options::ParserOptions::default(),
    )
    .with_prefix_offset(prefix_offset as u32);
    let mut items: Vec<Zval> = Vec::new();
    for v in vds.variable_definitions() {
        items.push(crate::lower::lower_variable_definition_for_partial(&v, &ctx));
    }
    Some(crate::lower::node_list_from_vec(items))
}

/// Source preprocessor for `allowLegacySDLEmptyFields`: remove empty `{ }`
/// (and `{}`) bodies from type definitions so apollo-parser doesn't error
/// on "expected a field definition". The empty fields list is then
/// supplied by the lowerer as an empty `NodeList`.
/// Preprocessor for `allowLegacySDLImplementsInterfaces`: rewrites
/// `implements A B C` into `implements A & B & C` so apollo-parser accepts
/// the legacy spec syntax. Only kicks in when the option is set.
fn preprocess_legacy_implements(body: &str) -> String {
    let mut out = String::with_capacity(body.len() + 16);
    let bytes = body.as_bytes();
    let kw = b"implements";
    let mut i = 0;
    while i < bytes.len() {
        if matches_word(bytes, i, kw) {
            out.push_str("implements");
            let mut j = i + kw.len();
            // Walk a run of `<ws> NAME <ws> NAME <ws> …` and insert `&` between names.
            // First copy whitespace.
            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                out.push(bytes[j] as char);
                j += 1;
            }
            // Optional leading `&` is acceptable; skip if present.
            if j < bytes.len() && bytes[j] == b'&' {
                out.push('&');
                j += 1;
                while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                    out.push(bytes[j] as char);
                    j += 1;
                }
            }
            // Consume names separated by whitespace; replace the LAST space
            // of each separator with `&` so the source length doesn't shift.
            let mut prev_name_end: Option<usize> = None;
            loop {
                let name_start = j;
                while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                    j += 1;
                }
                if name_start == j {
                    break;
                }
                // If we already have a name and the gap was pure whitespace,
                // overwrite the last space with `&`.
                if let Some(pe) = prev_name_end {
                    let gap_bytes = &body[pe..name_start];
                    if !gap_bytes.is_empty()
                        && gap_bytes.bytes().all(|c| c == b' ' || c == b'\t')
                    {
                        // out currently contains gap_bytes (from a prior pass).
                        // Replace its final char in `out` with `&`.
                        debug_assert!(out.ends_with(' ') || out.ends_with('\t'));
                        out.pop();
                        out.push('&');
                    }
                }
                out.push_str(&body[name_start..j]);
                prev_name_end = Some(j);
                // Look ahead: optional whitespace, then either another name or stop.
                let mut k = j;
                while k < bytes.len() && (bytes[k] == b' ' || bytes[k] == b'\t') {
                    k += 1;
                }
                if k < bytes.len() && (bytes[k].is_ascii_alphabetic() || bytes[k] == b'_') {
                    out.push_str(&body[j..k]);
                    j = k;
                    continue;
                }
                if k < bytes.len() && bytes[k] == b'&' {
                    out.push_str(&body[j..k]);
                    out.push('&');
                    j = k + 1;
                    continue;
                }
                out.push_str(&body[j..k]);
                j = k;
                break;
            }
            i = j;
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn preprocess_empty_sdl_braces(body: &str) -> String {
    // Skip empty `{ ... whitespace ... }` pairs from the source. Walk by
    // chars to stay UTF-8-safe.
    let mut out = String::with_capacity(body.len());
    let mut chars: Vec<(usize, char)> = body.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let (_, c) = chars[i];
        if c == '{' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].1.is_whitespace() {
                j += 1;
            }
            if j < chars.len() && chars[j].1 == '}' {
                // Skip the `{ }` pair entirely.
                i = j + 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    let _ = chars; // owned vec lives till end-of-fn
    out
}

extern "C" fn parse_value(ex: &mut ExecuteData, retval: &mut Zval) {
    let (source_zv, options_zv) = match read_args(ex) {
        Some(v) => v,
        None => return,
    };
    let opts = match ParserOptions::from_zval(options_zv) {
        Ok(o) => o,
        Err(_) => {
            throw_runtime("invalid options array");
            return;
        }
    };
    let src = match coerce_source(source_zv) {
        Ok(s) => s,
        Err(msg) => {
            throw_runtime(msg);
            return;
        }
    };
    // Wrap into a full operation so any value literal (including bare "null"
    // and "[]") parses cleanly: `query{__wrap(__a:<SRC>)}`. The user's offsets
    // are recovered by subtracting the prefix length.
    let prefix = "query{__wrap(__a:";
    let suffix = ")}";
    let wrapped = format!("{prefix}{}{suffix}", src.body);
    let tree = apollo_parser::Parser::new(&wrapped)
        .recursion_limit(opts.apollo_recursion_limit())
        .parse();
    if let Some(err) = tree.errors().next() {
        errors::throw_syntax_error(
            &src.source_zv,
            errors::first_error_offset(err).saturating_sub(prefix.len()) as i64,
            &errors::first_error_description(err, opts.apollo_recursion_limit(), &src.body),
        );
        return;
    }
    let ctx = LowerCtx::new(src.body.clone(), src.source_zv, opts)
        .with_prefix_offset(prefix.len() as u32);
    let doc = tree.document();
    let value = doc
        .definitions()
        .find_map(|d| match d {
            apollo_parser::cst::Definition::OperationDefinition(op) => Some(op),
            _ => None,
        })
        .and_then(|op| op.selection_set())
        .and_then(|ss| ss.selections().next())
        .and_then(|s| match s {
            apollo_parser::cst::Selection::Field(f) => f
                .arguments()
                .and_then(|args| args.arguments().next())
                .and_then(|a| a.value()),
            _ => None,
        });
    let Some(value) = value else {
        throw_runtime("internal: parseValue could not extract Value from wrapper");
        return;
    };
    *retval = lower_value(&value, &ctx);
}

extern "C" fn parse_type(ex: &mut ExecuteData, retval: &mut Zval) {
    let (source_zv, options_zv) = match read_args(ex) {
        Some(v) => v,
        None => return,
    };
    let opts = match ParserOptions::from_zval(options_zv) {
        Ok(o) => o,
        Err(_) => {
            throw_runtime("invalid options array");
            return;
        }
    };
    let src = match coerce_source(source_zv) {
        Ok(s) => s,
        Err(msg) => {
            throw_runtime(msg);
            return;
        }
    };
    let tree = apollo_parser::Parser::new(&src.body)
        .recursion_limit(opts.apollo_recursion_limit())
        .parse_type();
    if let Some(err) = tree.errors().next() {
        errors::throw_syntax_error(
            &src.source_zv,
            errors::first_error_offset(err) as i64,
            &errors::first_error_description(err, opts.apollo_recursion_limit(), &src.body),
        );
        return;
    }
    let ctx = LowerCtx::new(src.body.clone(), src.source_zv, opts);
    let t = tree.ty();
    *retval = lower_type(&t, &ctx);
}

extern "C" fn call_static(ex: &mut ExecuteData, retval: &mut Zval) {
    let n_args = unsafe { ex.This.u2.num_args } as usize;
    if n_args < 1 {
        throw_runtime("Parser::__callStatic expects (name, arguments)");
        return;
    }
    let name_zv = match unsafe { ex.zend_call_arg(0) } {
        Some(z) => z,
        None => return,
    };
    let args_zv = if n_args > 1 { unsafe { ex.zend_call_arg(1) } } else { None };
    let name = name_zv.str().unwrap_or("").to_string();
    let args_ht = args_zv.and_then(|z| z.array());
    // first positional argument inside $arguments is the source
    let source_zv = args_ht.and_then(|ht| ht.get_index(0));
    let options_zv = args_ht.and_then(|ht| ht.get_index(1));

    let Some(source_zv) = source_zv else {
        throw_runtime(&format!("Parser::{name}: missing source"));
        return;
    };

    let opts = match crate::options::ParserOptions::from_zval(options_zv) {
        Ok(o) => o,
        Err(_) => {
            throw_runtime("invalid options array");
            return;
        }
    };
    let src = match crate::source::coerce_source(source_zv) {
        Ok(s) => s,
        Err(msg) => {
            throw_runtime(msg);
            return;
        }
    };

    // const* helpers must reject Variable values.
    if name.starts_with("const") && src.body.contains('$') {
        errors::throw_syntax_error(
            &src.source_zv,
            src.body.find('$').unwrap_or(0) as i64,
            "Unexpected $",
        );
        return;
    }

    // Dispatch to the right partial-parser wrapping.
    let (wrapped, prefix_len) = match name.as_str() {
        "valueLiteral" | "value" | "constValueLiteral" => {
            (format!("query{{__wrap(__a:{})}}", src.body), "query{__wrap(__a:".len())
        }
        "typeReference" | "type" => {
            // parse_type entry point handles bare types.
            return parse_partial_type(retval, &src, opts);
        }
        "name" => {
            // Names appear as the alias of a wrapper field.
            (format!("{{ __wrap: {} }}", src.body), "{ __wrap: ".len())
        }
        "argument" | "constArgument" => {
            (format!("query{{__wrap({})}}", src.body), "query{__wrap(".len())
        }
        "arguments" | "constArguments" => {
            (format!("query{{__wrap{}}}", src.body), "query{__wrap".len())
        }
        "directive" | "constDirective" => {
            (format!("query {} {{__typename}}", src.body), "query ".len())
        }
        "directives" | "constDirectives" => {
            (format!("query {} {{__typename}}", src.body), "query ".len())
        }
        "selectionSet" => {
            (format!("query {}", src.body), "query ".len())
        }
        "argumentsDefinition" => {
            // `(arg1: Type, arg2: Type)` — wrap as directive arg list.
            (format!("directive @x{} on FIELD", src.body), "directive @x".len())
        }
        "fieldsDefinition" => {
            // `{ field1: T, field2: T }` — wrap as object type definition.
            (format!("type X {}", src.body), "type X ".len())
        }
        "directiveLocations" => {
            // `| INPUT_OBJECT | OBJECT` — wrap as directive definition.
            (format!("directive @x on {}", src.body), "directive @x on ".len())
        }
        "implementsInterfaces" => {
            (format!("type X {} {{ f: T }}", src.body), "type X ".len())
        }
        "unionMemberTypes" => {
            (format!("union X {}", src.body), "union X ".len())
        }
        "operationDefinition" | "fragmentDefinition" | "typeSystemDefinition"
        | "schemaDefinition" | "operationTypeDefinition" | "scalarTypeDefinition"
        | "objectTypeDefinition" | "fieldDefinition" | "inputValueDefinition"
        | "interfaceTypeDefinition" | "unionTypeDefinition" | "enumTypeDefinition"
        | "enumValueDefinition" | "inputObjectTypeDefinition" | "directiveDefinition"
        | "typeSystemExtension" | "schemaExtension" | "scalarTypeExtension"
        | "objectTypeExtension" | "interfaceTypeExtension" | "unionTypeExtension"
        | "enumTypeExtension" | "inputObjectTypeExtension" => {
            // Parse as a full document, then return the first definition.
            return parse_partial_definition(retval, &src, opts);
        }
        _ => {
            throw_runtime(&format!("Parser::{name}() is not supported"));
            return;
        }
    };

    let tree = apollo_parser::Parser::new(&wrapped)
        .recursion_limit(opts.apollo_recursion_limit())
        .parse();
    if let Some(err) = tree.errors().next() {
        errors::throw_syntax_error(
            &src.source_zv,
            errors::first_error_offset(err).saturating_sub(prefix_len) as i64,
            &errors::first_error_description(err, opts.apollo_recursion_limit(), &src.body),
        );
        return;
    }
    let ctx = LowerCtx::new(src.body.clone(), src.source_zv, opts)
        .with_prefix_offset(prefix_len as u32);
    let doc = tree.document();

    use apollo_parser::cst as cst;
    let zv = match name.as_str() {
        "valueLiteral" | "value" | "constValueLiteral" => {
            doc.definitions()
                .find_map(|d| match d {
                    cst::Definition::OperationDefinition(op) => Some(op),
                    _ => None,
                })
                .and_then(|op| op.selection_set())
                .and_then(|ss| ss.selections().next())
                .and_then(|s| match s {
                    cst::Selection::Field(f) => f
                        .arguments()
                        .and_then(|args| args.arguments().next())
                        .and_then(|a| a.value()),
                    _ => None,
                })
                .map(|v| lower_value(&v, &ctx))
        }
        "name" => {
            // The synthesized "{ __wrap: <NAME> }" has the user's name as
            // the field's `name` (it's just a Name token in field position).
            doc.definitions()
                .find_map(|d| match d {
                    cst::Definition::OperationDefinition(op) => Some(op),
                    _ => None,
                })
                .and_then(|op| op.selection_set())
                .and_then(|ss| ss.selections().next())
                .and_then(|s| match s {
                    cst::Selection::Field(f) => f.name(),
                    _ => None,
                })
                .map(|n| crate::lower::name_node(&n, &ctx))
        }
        "argument" | "constArgument" => {
            doc.definitions()
                .find_map(|d| match d {
                    cst::Definition::OperationDefinition(op) => Some(op),
                    _ => None,
                })
                .and_then(|op| op.selection_set())
                .and_then(|ss| ss.selections().next())
                .and_then(|s| match s {
                    cst::Selection::Field(f) => f.arguments(),
                    _ => None,
                })
                .and_then(|args| args.arguments().next())
                .map(|a| crate::lower::lower_argument(&a, &ctx))
        }
        "argumentsDefinition" => {
            // We wrapped as `directive @x(...) on FIELD`. Pull the
            // arguments_definition off the directive definition and emit a
            // NodeList<InputValueDefinitionNode>.
            doc.definitions()
                .find_map(|d| match d {
                    cst::Definition::DirectiveDefinition(d) => d.arguments_definition(),
                    _ => None,
                })
                .map(|ad| {
                    let mut items: Vec<Zval> = Vec::new();
                    for ivd in ad.input_value_definitions() {
                        // Reuse the sdl lowerer via a tiny shim.
                        let zv = crate::lower::lower_input_value_definition_for_partial(&ivd, &ctx);
                        items.push(zv);
                    }
                    crate::lower::node_list_from_vec(items)
                })
        }
        "fieldsDefinition" => {
            doc.definitions()
                .find_map(|d| match d {
                    cst::Definition::ObjectTypeDefinition(o) => o.fields_definition(),
                    _ => None,
                })
                .map(|fd| {
                    let mut items: Vec<Zval> = Vec::new();
                    for f in fd.field_definitions() {
                        let zv = crate::lower::lower_field_definition_for_partial(&f, &ctx);
                        items.push(zv);
                    }
                    crate::lower::node_list_from_vec(items)
                })
        }
        "directive" | "constDirective" => {
            // We wrapped as `query <SRC> {__typename}`. The directive is on
            // the OperationDefinition.
            doc.definitions()
                .find_map(|d| match d {
                    cst::Definition::OperationDefinition(op) => Some(op),
                    _ => None,
                })
                .and_then(|op| op.directives())
                .and_then(|d| d.directives().next())
                .map(|d| crate::lower::lower_directive(&d, &ctx))
        }
        "directives" | "constDirectives" => {
            doc.definitions()
                .find_map(|d| match d {
                    cst::Definition::OperationDefinition(op) => Some(op),
                    _ => None,
                })
                .and_then(|op| op.directives())
                .map(|d| {
                    let mut items: Vec<Zval> = Vec::new();
                    for di in d.directives() {
                        items.push(crate::lower::lower_directive(&di, &ctx));
                    }
                    crate::lower::node_list_from_vec(items)
                })
        }
        "selectionSet" => {
            doc.definitions()
                .find_map(|d| match d {
                    cst::Definition::OperationDefinition(op) => Some(op),
                    _ => None,
                })
                .and_then(|op| op.selection_set())
                .map(|ss| crate::lower::lower_selection_set_for_partial(&ss, &ctx))
        }
        "directiveLocations" => {
            doc.definitions()
                .find_map(|d| match d {
                    cst::Definition::DirectiveDefinition(d) => d.directive_locations(),
                    _ => None,
                })
                .map(|dls| {
                    let mut items: Vec<Zval> = Vec::new();
                    for loc in dls.directive_locations() {
                        let txt = {
                            use apollo_parser::cst::CstNode;
                            loc.syntax().text().to_string().trim().to_string()
                        };
                        let r = {
                            use apollo_parser::cst::CstNode;
                            loc.syntax().text_range()
                        };
                        items.push(
                            crate::lower::name_node_with_value(&txt, r, &ctx),
                        );
                    }
                    crate::lower::node_list_from_vec(items)
                })
        }
        "implementsInterfaces" => {
            doc.definitions()
                .find_map(|d| match d {
                    cst::Definition::ObjectTypeDefinition(o) => o.implements_interfaces(),
                    _ => None,
                })
                .map(|ii| {
                    let mut items: Vec<Zval> = Vec::new();
                    for nt in ii.named_types() {
                        let name = nt
                            .name()
                            .map(|n| crate::lower::name_node(&n, &ctx))
                            .unwrap_or_else(Zval::new);
                        let zv = {
                            use apollo_parser::cst::CstNode;
                            crate::lower::new_node(
                                crate::classes::slots::named_type_node::get(),
                                nt.syntax().text_range(),
                                &ctx,
                            )
                            .prop("name", name)
                            .finish()
                        };
                        items.push(zv);
                    }
                    crate::lower::node_list_from_vec(items)
                })
        }
        "unionMemberTypes" => {
            doc.definitions()
                .find_map(|d| match d {
                    cst::Definition::UnionTypeDefinition(u) => u.union_member_types(),
                    _ => None,
                })
                .map(|um| {
                    let mut items: Vec<Zval> = Vec::new();
                    for nt in um.named_types() {
                        let name = nt
                            .name()
                            .map(|n| crate::lower::name_node(&n, &ctx))
                            .unwrap_or_else(Zval::new);
                        let zv = {
                            use apollo_parser::cst::CstNode;
                            crate::lower::new_node(
                                crate::classes::slots::named_type_node::get(),
                                nt.syntax().text_range(),
                                &ctx,
                            )
                            .prop("name", name)
                            .finish()
                        };
                        items.push(zv);
                    }
                    crate::lower::node_list_from_vec(items)
                })
        }
        _ => None,
    };

    match zv {
        Some(v) => *retval = v,
        None => throw_runtime(&format!("Parser::{name}() failed to extract result")),
    }
}

fn parse_partial_type(
    retval: &mut Zval,
    src: &crate::source::SourceInput,
    opts: crate::options::ParserOptions,
) {
    let tree = apollo_parser::Parser::new(&src.body)
        .recursion_limit(opts.apollo_recursion_limit())
        .parse_type();
    if let Some(err) = tree.errors().next() {
        errors::throw_syntax_error(
            &src.source_zv,
            errors::first_error_offset(err) as i64,
            &errors::first_error_description(err, opts.apollo_recursion_limit(), &src.body),
        );
        return;
    }
    let ctx = LowerCtx::new(src.body.clone(), src.source_zv.shallow_clone(), opts);
    let t = tree.ty();
    *retval = lower_type(&t, &ctx);
}

fn parse_partial_definition(
    retval: &mut Zval,
    src: &crate::source::SourceInput,
    opts: crate::options::ParserOptions,
) {
    let tree = apollo_parser::Parser::new(&src.body)
        .recursion_limit(opts.apollo_recursion_limit())
        .parse();
    if let Some(err) = tree.errors().next() {
        errors::throw_syntax_error(
            &src.source_zv,
            errors::first_error_offset(err) as i64,
            &errors::first_error_description(err, opts.apollo_recursion_limit(), &src.body),
        );
        return;
    }
    let ctx = LowerCtx::new(src.body.clone(), src.source_zv.shallow_clone(), opts);
    let doc = tree.document();
    if let Some(def) = doc.definitions().next() {
        if let Some(zv) = crate::lower::lower_definition(&def, &ctx) {
            *retval = zv;
            return;
        }
    }
    throw_runtime("Parser: empty document");
}

fn read_args<'a>(ex: &'a mut ExecuteData) -> Option<(&'a Zval, Option<&'a Zval>)> {
    let num = unsafe { ex.This.u2.num_args } as usize;
    if num == 0 {
        throw_runtime("Parser::parse expects at least one argument");
        return None;
    }
    // SAFETY: zend_call_arg returns a reference tied to `ex`; we cap the index
    // by `num` so we never read past the actual argument vector.
    let arg0 = unsafe { ex.zend_call_arg(0) };
    let arg0 = match arg0 {
        Some(a) => &*a,
        None => {
            throw_runtime("Parser::parse: missing source argument");
            return None;
        }
    };
    let arg1: Option<&Zval> = if num > 1 {
        unsafe { ex.zend_call_arg(1) }.map(|z| &*z)
    } else {
        None
    };
    Some((arg0, arg1))
}

fn throw_runtime(msg: &str) {
    let ce: &'static ClassEntry =
        ClassEntry::try_find("RuntimeException").unwrap_or_else(|| ce::exception());
    let _ = PhpException::new(msg.to_string(), 0, ce).throw();
}
