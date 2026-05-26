//! Support classes: SourceLocation, Source, Token, Lexer (stub),
//! DirectiveLocation, NodeKind, Location.

use ext_php_rs::builders::{ClassBuilder, FunctionBuilder};
use ext_php_rs::error::Result;
use ext_php_rs::flags::{DataType, MethodFlags};
use ext_php_rs::types::{ZendHashTable, Zval};
use ext_php_rs::zend::ExecuteData;

use super::slots;
use super::{json_serializable, pub_int, pub_nullable, pub_string, pub_uninit};

pub fn register_pre_node() -> Result<()> {
    source_location()?;
    source()?;
    // `Token` and `Lexer` are intentionally not registered: see PRD §3.2.
    // `NodeKind` is also intentionally not registered: the class includes a
    // 41-entry `CLASS_MAP` array constant which ext-php-rs 0.15 corrupts
    // when stored on the class constant table. Composer autoloads the
    // on-disk `NodeKind.php` with the same constants intact.
    directive_location()?;
    location()?;
    Ok(())
}

fn source_location() -> Result<()> {
    ClassBuilder::new(slots::source_location::PHP_NAME)
        .implements(json_serializable())
        .property(pub_int("line", 0))
        .property(pub_int("column", 0))
        .method(
            FunctionBuilder::new("__construct", source_location_construct)
                .arg(ext_php_rs::args::Arg::new("line", DataType::Long))
                .arg(ext_php_rs::args::Arg::new("col", DataType::Long)),
            MethodFlags::Public,
        )
        .method(
            FunctionBuilder::new("toArray", source_location_to_array)
                .returns(DataType::Array, false, false),
            MethodFlags::Public,
        )
        .method(
            FunctionBuilder::new("toSerializableArray", source_location_to_array)
                .returns(DataType::Array, false, false),
            MethodFlags::Public,
        )
        .method(
            FunctionBuilder::new("jsonSerialize", source_location_json_serialize)
                .returns(DataType::Array, false, false),
            MethodFlags::Public,
        )
        .registration(slots::source_location::set)
        .register()
}

extern "C" fn source_location_construct(ex: &mut ExecuteData, _retval: &mut Zval) {
    let n_args = unsafe { ex.This.u2.num_args } as usize;
    let line = if n_args > 0 {
        unsafe { ex.zend_call_arg(0) }.and_then(|z| z.long()).unwrap_or(0)
    } else { 0 };
    let col = if n_args > 1 {
        unsafe { ex.zend_call_arg(1) }.and_then(|z| z.long()).unwrap_or(0)
    } else { 0 };
    if let Some(this) = ex.get_self() {
        let _ = this.set_property("line", line);
        let _ = this.set_property("column", col);
    }
}

extern "C" fn source_location_to_array(ex: &mut ExecuteData, retval: &mut Zval) {
    source_location_json_serialize(ex, retval);
}

extern "C" fn source_location_json_serialize(ex: &mut ExecuteData, retval: &mut Zval) {
    let mut ht = ZendHashTable::new();
    if let Some(this) = ex.get_self() {
        let line = this.get_property::<i64>("line").unwrap_or(0);
        let column = this.get_property::<i64>("column").unwrap_or(0);
        let _ = ht.insert("line", line);
        let _ = ht.insert("column", column);
    }
    retval.set_hashtable(ht);
}

fn source() -> Result<()> {
    ClassBuilder::new(slots::source::PHP_NAME)
        .property(pub_string("body", ""))
        .property(pub_int("length", 0))
        .property(pub_string("name", "GraphQL request"))
        .property(pub_uninit("locationOffset"))
        .method(
            FunctionBuilder::new("__construct", source_construct)
                .arg(ext_php_rs::args::Arg::new("body", DataType::String))
                .arg(
                    ext_php_rs::args::Arg::new("name", DataType::String)
                        .default(r#"null"#)
                        .allow_null(),
                )
                .arg(
                    ext_php_rs::args::Arg::new(
                        "location",
                        DataType::Object(Some("GraphQL\\Language\\SourceLocation")),
                    )
                    .default(r#"null"#)
                    .allow_null(),
                ),
            MethodFlags::Public,
        )
        .method(
            FunctionBuilder::new("getLocation", source_get_location)
                .arg(ext_php_rs::args::Arg::new("position", DataType::Long))
                .returns(
                    DataType::Object(Some("GraphQL\\Language\\SourceLocation")),
                    false,
                    false,
                ),
            MethodFlags::Public,
        )
        .registration(slots::source::set)
        .register()
}

extern "C" fn source_construct(ex: &mut ExecuteData, _retval: &mut Zval) {
    let n_args = unsafe { ex.This.u2.num_args } as usize;
    let body_arg = if n_args > 0 { unsafe { ex.zend_call_arg(0) } } else { None };
    let name_arg = if n_args > 1 { unsafe { ex.zend_call_arg(1) } } else { None };
    let loc_arg = if n_args > 2 { unsafe { ex.zend_call_arg(2) } } else { None };
    let Some(this) = ex.get_self() else { return };

    let body = body_arg.and_then(|z| z.str()).unwrap_or("").to_string();
    let length = body.chars().count() as i64;
    let _ = this.set_property("body", body.as_str());
    let _ = this.set_property("length", length);

    let name = name_arg
        .and_then(|z| z.str())
        .filter(|s| !s.is_empty())
        .unwrap_or("GraphQL request");
    let _ = this.set_property("name", name);

    if let Some(loc) = loc_arg.filter(|z| !z.is_null()) {
        let _ = this.set_property("locationOffset", loc.shallow_clone());
    } else {
        // Default to new SourceLocation(1, 1)
        let mut sl = ext_php_rs::types::ZendObject::new(slots::source_location::get());
        let _ = sl.set_property("line", 1_i64);
        let _ = sl.set_property("column", 1_i64);
        let mut sl_zv = Zval::new();
        sl_zv = crate::classes::obj_to_zval(sl);
        let _ = this.set_property("locationOffset", sl_zv);
    }
}

extern "C" fn source_get_location(ex: &mut ExecuteData, retval: &mut Zval) {
    let n_args = unsafe { ex.This.u2.num_args } as usize;
    let position = if n_args > 0 {
        unsafe { ex.zend_call_arg(0) }.and_then(|z| z.long()).unwrap_or(0)
    } else { 0 };
    let Some(this) = ex.get_self() else { return };
    let body = this.get_property::<String>("body").unwrap_or_default();

    let (line, column) = compute_line_column(&body, position);

    let mut sl = ext_php_rs::types::ZendObject::new(slots::source_location::get());
    let _ = sl.set_property("line", line);
    let _ = sl.set_property("column", column);
    *retval = crate::classes::obj_to_zval(sl);
}

/// Mirror of `Source::getLocation` from the on-disk PHP file. `position` is a
/// **character** offset (mirroring `mb_substr` semantics). Returns 1-indexed
/// (line, column).
fn compute_line_column(body: &str, position: i64) -> (i64, i64) {
    let position = if position < 0 { 0_usize } else { position as usize };
    let mut line: i64 = 1;
    let mut col_start: usize = 0; // character offset where the current line begins
    let mut idx: usize = 0;
    let mut chars = body.chars().peekable();
    while idx < position {
        let Some(c) = chars.next() else { break };
        idx += 1;
        match c {
            '\r' => {
                line += 1;
                if chars.peek() == Some(&'\n') {
                    chars.next();
                    idx += 1;
                }
                col_start = idx;
            }
            '\n' | '\u{2028}' | '\u{2029}' => {
                line += 1;
                col_start = idx;
            }
            _ => {}
        }
    }
    let column = (position - col_start) as i64 + 1;
    (line, column)
}

fn token() -> Result<()> {
    ClassBuilder::new(slots::token::PHP_NAME)
        .constant("SOF", "<SOF>", &[])?
        .constant("EOF", "<EOF>", &[])?
        .constant("BANG", "!", &[])?
        .constant("DOLLAR", "$", &[])?
        .constant("AMP", "&", &[])?
        .constant("PAREN_L", "(", &[])?
        .constant("PAREN_R", ")", &[])?
        .constant("SPREAD", "...", &[])?
        .constant("COLON", ":", &[])?
        .constant("EQUALS", "=", &[])?
        .constant("AT", "@", &[])?
        .constant("BRACKET_L", "[", &[])?
        .constant("BRACKET_R", "]", &[])?
        .constant("BRACE_L", "{", &[])?
        .constant("PIPE", "|", &[])?
        .constant("BRACE_R", "}", &[])?
        .constant("NAME", "Name", &[])?
        .constant("INT", "Int", &[])?
        .constant("FLOAT", "Float", &[])?
        .constant("STRING", "String", &[])?
        .constant("BLOCK_STRING", "BlockString", &[])?
        .constant("COMMENT", "Comment", &[])?
        .property(pub_uninit("kind"))
        .property(pub_int("start", 0))
        .property(pub_int("end", 0))
        .property(pub_int("line", 0))
        .property(pub_int("column", 0))
        .property(pub_nullable("value"))
        .property(pub_nullable("prev"))
        .property(pub_nullable("next"))
        .registration(slots::token::set)
        .register()
}

fn lexer() -> Result<()> {
    // Stub: PRD §3.2 explicitly defers the full token-chain `Lexer` to a later
    // phase. The class exists so user code that does
    // `class_exists('GraphQL\\Language\\Lexer', false)` doesn't fall through
    // to the autoloader. No methods or properties are declared.
    ClassBuilder::new(slots::lexer::PHP_NAME)
        .registration(slots::lexer::set)
        .register()
}

fn directive_location() -> Result<()> {
    ClassBuilder::new(slots::directive_location::PHP_NAME)
        .constant("QUERY", "QUERY", &[])?
        .constant("MUTATION", "MUTATION", &[])?
        .constant("SUBSCRIPTION", "SUBSCRIPTION", &[])?
        .constant("FIELD", "FIELD", &[])?
        .constant("FRAGMENT_DEFINITION", "FRAGMENT_DEFINITION", &[])?
        .constant("FRAGMENT_SPREAD", "FRAGMENT_SPREAD", &[])?
        .constant("INLINE_FRAGMENT", "INLINE_FRAGMENT", &[])?
        .constant("VARIABLE_DEFINITION", "VARIABLE_DEFINITION", &[])?
        .constant("SCHEMA", "SCHEMA", &[])?
        .constant("SCALAR", "SCALAR", &[])?
        .constant("OBJECT", "OBJECT", &[])?
        .constant("FIELD_DEFINITION", "FIELD_DEFINITION", &[])?
        .constant("ARGUMENT_DEFINITION", "ARGUMENT_DEFINITION", &[])?
        .constant("IFACE", "INTERFACE", &[])?
        .constant("UNION", "UNION", &[])?
        .constant("ENUM", "ENUM", &[])?
        .constant("ENUM_VALUE", "ENUM_VALUE", &[])?
        .constant("INPUT_OBJECT", "INPUT_OBJECT", &[])?
        .constant("INPUT_FIELD_DEFINITION", "INPUT_FIELD_DEFINITION", &[])?
        // Composite array constants (EXECUTABLE_LOCATIONS / TYPE_SYSTEM_LOCATIONS / LOCATIONS)
        // are deferred to Phase 2 — `DirectiveLocation::has()` is a static method,
        // not currently on hot paths, and the PHP fallback is unused once the
        // extension defines the class.
        .registration(slots::directive_location::set)
        .register()
}

fn build_class_map() -> ext_php_rs::boxed::ZBox<ZendHashTable> {
    let mut ht = ZendHashTable::new();
    for (kind, fqcn) in CLASS_MAP_ENTRIES {
        let key: String = (*kind).to_string();
        let val: String = (*fqcn).to_string();
        let _ = ht.insert(key.as_str(), val);
    }
    ht
}

const CLASS_MAP_ENTRIES: &[(&str, &str)] = &[
    ("Name", "GraphQL\\Language\\AST\\NameNode"),
    ("Document", "GraphQL\\Language\\AST\\DocumentNode"),
    ("OperationDefinition", "GraphQL\\Language\\AST\\OperationDefinitionNode"),
    ("VariableDefinition", "GraphQL\\Language\\AST\\VariableDefinitionNode"),
    ("Variable", "GraphQL\\Language\\AST\\VariableNode"),
    ("SelectionSet", "GraphQL\\Language\\AST\\SelectionSetNode"),
    ("Field", "GraphQL\\Language\\AST\\FieldNode"),
    ("Argument", "GraphQL\\Language\\AST\\ArgumentNode"),
    ("FragmentSpread", "GraphQL\\Language\\AST\\FragmentSpreadNode"),
    ("InlineFragment", "GraphQL\\Language\\AST\\InlineFragmentNode"),
    ("FragmentDefinition", "GraphQL\\Language\\AST\\FragmentDefinitionNode"),
    ("IntValue", "GraphQL\\Language\\AST\\IntValueNode"),
    ("FloatValue", "GraphQL\\Language\\AST\\FloatValueNode"),
    ("StringValue", "GraphQL\\Language\\AST\\StringValueNode"),
    ("BooleanValue", "GraphQL\\Language\\AST\\BooleanValueNode"),
    ("EnumValue", "GraphQL\\Language\\AST\\EnumValueNode"),
    ("NullValue", "GraphQL\\Language\\AST\\NullValueNode"),
    ("ListValue", "GraphQL\\Language\\AST\\ListValueNode"),
    ("ObjectValue", "GraphQL\\Language\\AST\\ObjectValueNode"),
    ("ObjectField", "GraphQL\\Language\\AST\\ObjectFieldNode"),
    ("Directive", "GraphQL\\Language\\AST\\DirectiveNode"),
    ("NamedType", "GraphQL\\Language\\AST\\NamedTypeNode"),
    ("ListType", "GraphQL\\Language\\AST\\ListTypeNode"),
    ("NonNullType", "GraphQL\\Language\\AST\\NonNullTypeNode"),
    ("SchemaDefinition", "GraphQL\\Language\\AST\\SchemaDefinitionNode"),
    ("OperationTypeDefinition", "GraphQL\\Language\\AST\\OperationTypeDefinitionNode"),
    ("ScalarTypeDefinition", "GraphQL\\Language\\AST\\ScalarTypeDefinitionNode"),
    ("ObjectTypeDefinition", "GraphQL\\Language\\AST\\ObjectTypeDefinitionNode"),
    ("FieldDefinition", "GraphQL\\Language\\AST\\FieldDefinitionNode"),
    ("InputValueDefinition", "GraphQL\\Language\\AST\\InputValueDefinitionNode"),
    ("InterfaceTypeDefinition", "GraphQL\\Language\\AST\\InterfaceTypeDefinitionNode"),
    ("UnionTypeDefinition", "GraphQL\\Language\\AST\\UnionTypeDefinitionNode"),
    ("EnumTypeDefinition", "GraphQL\\Language\\AST\\EnumTypeDefinitionNode"),
    ("EnumValueDefinition", "GraphQL\\Language\\AST\\EnumValueDefinitionNode"),
    ("InputObjectTypeDefinition", "GraphQL\\Language\\AST\\InputObjectTypeDefinitionNode"),
    ("ScalarTypeExtension", "GraphQL\\Language\\AST\\ScalarTypeExtensionNode"),
    ("ObjectTypeExtension", "GraphQL\\Language\\AST\\ObjectTypeExtensionNode"),
    ("InterfaceTypeExtension", "GraphQL\\Language\\AST\\InterfaceTypeExtensionNode"),
    ("UnionTypeExtension", "GraphQL\\Language\\AST\\UnionTypeExtensionNode"),
    ("EnumTypeExtension", "GraphQL\\Language\\AST\\EnumTypeExtensionNode"),
    ("InputObjectTypeExtension", "GraphQL\\Language\\AST\\InputObjectTypeExtensionNode"),
    ("DirectiveDefinition", "GraphQL\\Language\\AST\\DirectiveDefinitionNode"),
];

fn node_kind() -> Result<()> {
    ClassBuilder::new(slots::node_kind::PHP_NAME)
        .constant("NAME", "Name", &[])?
        .constant("DOCUMENT", "Document", &[])?
        .constant("OPERATION_DEFINITION", "OperationDefinition", &[])?
        .constant("VARIABLE_DEFINITION", "VariableDefinition", &[])?
        .constant("VARIABLE", "Variable", &[])?
        .constant("SELECTION_SET", "SelectionSet", &[])?
        .constant("FIELD", "Field", &[])?
        .constant("ARGUMENT", "Argument", &[])?
        .constant("FRAGMENT_SPREAD", "FragmentSpread", &[])?
        .constant("INLINE_FRAGMENT", "InlineFragment", &[])?
        .constant("FRAGMENT_DEFINITION", "FragmentDefinition", &[])?
        .constant("INT", "IntValue", &[])?
        .constant("FLOAT", "FloatValue", &[])?
        .constant("STRING", "StringValue", &[])?
        .constant("BOOLEAN", "BooleanValue", &[])?
        .constant("ENUM", "EnumValue", &[])?
        .constant("NULL", "NullValue", &[])?
        .constant("LST", "ListValue", &[])?
        .constant("OBJECT", "ObjectValue", &[])?
        .constant("OBJECT_FIELD", "ObjectField", &[])?
        .constant("DIRECTIVE", "Directive", &[])?
        .constant("NAMED_TYPE", "NamedType", &[])?
        .constant("LIST_TYPE", "ListType", &[])?
        .constant("NON_NULL_TYPE", "NonNullType", &[])?
        .constant("SCHEMA_DEFINITION", "SchemaDefinition", &[])?
        .constant("OPERATION_TYPE_DEFINITION", "OperationTypeDefinition", &[])?
        .constant("SCALAR_TYPE_DEFINITION", "ScalarTypeDefinition", &[])?
        .constant("OBJECT_TYPE_DEFINITION", "ObjectTypeDefinition", &[])?
        .constant("FIELD_DEFINITION", "FieldDefinition", &[])?
        .constant("INPUT_VALUE_DEFINITION", "InputValueDefinition", &[])?
        .constant("INTERFACE_TYPE_DEFINITION", "InterfaceTypeDefinition", &[])?
        .constant("UNION_TYPE_DEFINITION", "UnionTypeDefinition", &[])?
        .constant("ENUM_TYPE_DEFINITION", "EnumTypeDefinition", &[])?
        .constant("ENUM_VALUE_DEFINITION", "EnumValueDefinition", &[])?
        .constant("INPUT_OBJECT_TYPE_DEFINITION", "InputObjectTypeDefinition", &[])?
        .constant("SCALAR_TYPE_EXTENSION", "ScalarTypeExtension", &[])?
        .constant("OBJECT_TYPE_EXTENSION", "ObjectTypeExtension", &[])?
        .constant("INTERFACE_TYPE_EXTENSION", "InterfaceTypeExtension", &[])?
        .constant("UNION_TYPE_EXTENSION", "UnionTypeExtension", &[])?
        .constant("ENUM_TYPE_EXTENSION", "EnumTypeExtension", &[])?
        .constant("INPUT_OBJECT_TYPE_EXTENSION", "InputObjectTypeExtension", &[])?
        .constant("DIRECTIVE_DEFINITION", "DirectiveDefinition", &[])?
        .constant("SCHEMA_EXTENSION", "SchemaExtension", &[])?
        // `CLASS_MAP` deferred: storing a large HashTable as a class
        // constant via ext-php-rs 0.15 corrupts the zval. Userland that needs
        // it falls through to Composer's autoloaded NodeKind.php (we leave
        // the constant unset; userland readers that always go through our
        // class entry get a "no such constant" the engine handles cleanly).
        .registration(slots::node_kind::set)
        .register()
}

fn location() -> Result<()> {
    ClassBuilder::new(slots::location::PHP_NAME)
        .property(pub_int("start", 0))
        .property(pub_int("end", 0))
        .property(pub_nullable("startToken"))
        .property(pub_nullable("endToken"))
        .property(pub_nullable("source"))
        .method(
            FunctionBuilder::new("__construct", location_construct)
                .arg(
                    ext_php_rs::args::Arg::new(
                        "startToken",
                        DataType::Object(Some("GraphQL\\Language\\Token")),
                    )
                    .default("null")
                    .allow_null(),
                )
                .arg(
                    ext_php_rs::args::Arg::new(
                        "endToken",
                        DataType::Object(Some("GraphQL\\Language\\Token")),
                    )
                    .default("null")
                    .allow_null(),
                )
                .arg(
                    ext_php_rs::args::Arg::new(
                        "source",
                        DataType::Object(Some("GraphQL\\Language\\Source")),
                    )
                    .default("null")
                    .allow_null(),
                ),
            MethodFlags::Public,
        )
        .method(
            FunctionBuilder::new("create", location_create)
                .arg(ext_php_rs::args::Arg::new("start", DataType::Long))
                .arg(ext_php_rs::args::Arg::new("end", DataType::Long))
                .returns(
                    DataType::Object(Some("GraphQL\\Language\\AST\\Location")),
                    false,
                    false,
                ),
            MethodFlags::Public | MethodFlags::Static,
        )
        .method(
            FunctionBuilder::new("toArray", location_to_array)
                .returns(DataType::Array, false, false),
            MethodFlags::Public,
        )
        .registration(slots::location::set)
        .register()
}

extern "C" fn location_construct(ex: &mut ExecuteData, _retval: &mut Zval) {
    let n_args = unsafe { ex.This.u2.num_args } as usize;
    let st_clone = if n_args > 0 {
        unsafe { ex.zend_call_arg(0) }.filter(|z| !z.is_null()).map(|z| z.shallow_clone())
    } else { None };
    let et_clone = if n_args > 1 {
        unsafe { ex.zend_call_arg(1) }.filter(|z| !z.is_null()).map(|z| z.shallow_clone())
    } else { None };
    let sr_clone = if n_args > 2 {
        unsafe { ex.zend_call_arg(2) }.filter(|z| !z.is_null()).map(|z| z.shallow_clone())
    } else { None };

    // Compute start/end from the tokens before we move them into properties.
    let (start, end) = match (&st_clone, &et_clone) {
        (Some(s), Some(e)) => {
            let sv = s.object().and_then(|o| o.get_property::<i64>("start").ok()).unwrap_or(0);
            let ev = e.object().and_then(|o| o.get_property::<i64>("end").ok()).unwrap_or(0);
            (Some(sv), Some(ev))
        }
        _ => (None, None),
    };

    if let Some(this) = ex.get_self() {
        if let Some(st) = st_clone {
            let _ = this.set_property("startToken", st);
        }
        if let Some(et) = et_clone {
            let _ = this.set_property("endToken", et);
        }
        if let Some(sr) = sr_clone {
            let _ = this.set_property("source", sr);
        }
        if let (Some(s), Some(e)) = (start, end) {
            let _ = this.set_property("start", s);
            let _ = this.set_property("end", e);
        }
    }
}

extern "C" fn location_create(ex: &mut ExecuteData, retval: &mut Zval) {
    let n_args = unsafe { ex.This.u2.num_args } as usize;
    let start = if n_args > 0 {
        unsafe { ex.zend_call_arg(0) }.and_then(|z| z.long()).unwrap_or(0)
    } else { 0 };
    let end = if n_args > 1 {
        unsafe { ex.zend_call_arg(1) }.and_then(|z| z.long()).unwrap_or(0)
    } else { 0 };
    let mut loc = ext_php_rs::types::ZendObject::new(slots::location::get());
    let _ = loc.set_property("start", start);
    let _ = loc.set_property("end", end);
    *retval = crate::classes::obj_to_zval(loc);
}

extern "C" fn location_to_array(ex: &mut ExecuteData, retval: &mut Zval) {
    let mut ht = ZendHashTable::new();
    if let Some(this) = ex.get_self() {
        let start = this.get_property::<i64>("start").unwrap_or(0);
        let end = this.get_property::<i64>("end").unwrap_or(0);
        let _ = ht.insert("start", start);
        let _ = ht.insert("end", end);
    }
    retval.set_hashtable(ht);
}
