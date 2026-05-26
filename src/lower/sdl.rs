//! Lower SDL (schema definition language) constructs.
//!
//! Phase 3 territory; Phase 2 lands enough scaffolding that SDL definitions
//! at the top level don't crash. Description strings, directive composition,
//! and extension resilience are covered here too because graphql-php's
//! ParserTest exercises a handful of SDL features even in its "executable"
//! subset (e.g. via `parse()`'s ability to round-trip mixed documents).

use apollo_parser::cst as cst;
use apollo_parser::cst::CstNode;
use ext_php_rs::types::Zval;

use crate::classes::slots;
use super::helpers::*;
use super::types::lower_type;
use super::values::lower_value;
use super::LowerCtx;

fn description_zval(desc: Option<cst::Description>, ctx: &LowerCtx) -> Option<Zval> {
    let d = desc?;
    let sv = d.string_value()?;
    // Reuse the value lowering for StringValue.
    let value = cst::Value::StringValue(sv);
    Some(super::values::lower_value(&value, ctx))
}

pub fn lower_schema_definition(s: &cst::SchemaDefinition, ctx: &LowerCtx) -> Zval {
    let range = s.syntax().text_range();
    let directives = lower_directives_opt(s.directives(), ctx);
    let mut op_types: Vec<Zval> = Vec::new();
    for o in s.root_operation_type_definitions() {
        op_types.push(lower_root_operation_type_definition(&o, ctx));
    }
    let operation_types = node_list_from_vec(op_types);
    let mut builder = new_node(slots::schema_definition_node::get(), range, ctx)
        .prop("directives", directives)
        .prop("operationTypes", operation_types);
    if let Some(desc) = description_zval(s.description(), ctx) {
        builder = builder.prop("description", desc);
    }
    builder.finish()
}

fn lower_root_operation_type_definition(
    o: &cst::RootOperationTypeDefinition,
    ctx: &LowerCtx,
) -> Zval {
    let range = o.syntax().text_range();
    let operation = o
        .operation_type()
        .and_then(|ot| {
            if ot.query_token().is_some() {
                Some("query")
            } else if ot.mutation_token().is_some() {
                Some("mutation")
            } else if ot.subscription_token().is_some() {
                Some("subscription")
            } else {
                None
            }
        })
        .unwrap_or("query");
    let typ = o
        .named_type()
        .map(|nt| {
            let name = nt
                .name()
                .map(|n| name_node(&n, ctx))
                .unwrap_or(Zval::new());
            new_node(slots::named_type_node::get(), nt.syntax().text_range(), ctx)
                .prop("name", name)
                .finish()
        })
        .unwrap_or(Zval::new());
    new_node(slots::operation_type_definition_node::get(), range, ctx)
        .prop_str("operation", operation)
        .prop("type", typ)
        .finish()
}

pub fn lower_directive_definition(d: &cst::DirectiveDefinition, ctx: &LowerCtx) -> Zval {
    let range = d.syntax().text_range();
    let name = d
        .name()
        .map(|n| name_node(&n, ctx))
        .unwrap_or(Zval::new());
    let arguments = lower_arguments_definition(d.arguments_definition(), ctx);
    let repeatable = d.repeatable_token().is_some();
    let mut locations: Vec<Zval> = Vec::new();
    if let Some(dl) = d.directive_locations() {
        for loc in dl.directive_locations() {
            let txt = loc.syntax().text().to_string();
            let trimmed = txt.trim().to_string();
            let r = loc.syntax().text_range();
            let nn = new_node(slots::name_node::get(), r, ctx)
                .prop_str("value", &trimmed)
                .finish();
            locations.push(nn);
        }
    }
    let locations_list = node_list_from_vec(locations);

    let mut builder = new_node(slots::directive_definition_node::get(), range, ctx)
        .prop("name", name);
    if let Some(desc) = description_zval(d.description(), ctx) {
        builder = builder.prop("description", desc);
    }
    builder
        .prop("arguments", arguments)
        .prop_bool("repeatable", repeatable)
        .prop("locations", locations_list)
        .finish()
}

fn lower_arguments_definition(
    args: Option<cst::ArgumentsDefinition>,
    ctx: &LowerCtx,
) -> Zval {
    let mut items: Vec<Zval> = Vec::new();
    if let Some(a) = args {
        for ivd in a.input_value_definitions() {
            items.push(lower_input_value_definition(&ivd, ctx));
        }
    }
    node_list_from_vec(items)
}

pub fn lower_input_value_definition_for_partial(
    i: &cst::InputValueDefinition,
    ctx: &LowerCtx,
) -> Zval {
    lower_input_value_definition(i, ctx)
}

pub fn lower_field_definition_for_partial(
    fd: &cst::FieldDefinition,
    ctx: &LowerCtx,
) -> Zval {
    lower_field_definition(fd, ctx)
}

fn lower_input_value_definition(i: &cst::InputValueDefinition, ctx: &LowerCtx) -> Zval {
    let range = i.syntax().text_range();
    let name = i.name().map(|n| name_node(&n, ctx)).unwrap_or(Zval::new());
    let typ = i.ty().map(|t| lower_type(&t, ctx)).unwrap_or(Zval::new());
    let default_value = i
        .default_value()
        .and_then(|dv| dv.value())
        .map(|v| lower_value(&v, ctx));
    let directives = lower_directives_opt(i.directives(), ctx);

    let mut builder = new_node(slots::input_value_definition_node::get(), range, ctx)
        .prop("name", name)
        .prop("type", typ);
    if let Some(dv) = default_value {
        builder = builder.prop("defaultValue", dv);
    }
    builder = builder.prop("directives", directives);
    if let Some(desc) = description_zval(i.description(), ctx) {
        builder = builder.prop("description", desc);
    }
    builder.finish()
}

fn lower_field_definition(fd: &cst::FieldDefinition, ctx: &LowerCtx) -> Zval {
    let range = fd.syntax().text_range();
    let name = fd.name().map(|n| name_node(&n, ctx)).unwrap_or(Zval::new());
    let args = lower_arguments_definition(fd.arguments_definition(), ctx);
    let typ = fd.ty().map(|t| lower_type(&t, ctx)).unwrap_or(Zval::new());
    let directives = lower_directives_opt(fd.directives(), ctx);
    let mut builder = new_node(slots::field_definition_node::get(), range, ctx)
        .prop("name", name)
        .prop("arguments", args)
        .prop("type", typ)
        .prop("directives", directives);
    if let Some(desc) = description_zval(fd.description(), ctx) {
        builder = builder.prop("description", desc);
    }
    builder.finish()
}

fn lower_fields_definition(fields: Option<cst::FieldsDefinition>, ctx: &LowerCtx) -> Zval {
    let mut items: Vec<Zval> = Vec::new();
    if let Some(f) = fields {
        for fd in f.field_definitions() {
            items.push(lower_field_definition(&fd, ctx));
        }
    }
    node_list_from_vec(items)
}

fn lower_implements_interfaces(
    ii: Option<cst::ImplementsInterfaces>,
    ctx: &LowerCtx,
) -> Zval {
    let mut items: Vec<Zval> = Vec::new();
    if let Some(ii) = ii {
        for nt in ii.named_types() {
            let name = nt
                .name()
                .map(|n| name_node(&n, ctx))
                .unwrap_or(Zval::new());
            let zv = new_node(slots::named_type_node::get(), nt.syntax().text_range(), ctx)
                .prop("name", name)
                .finish();
            items.push(zv);
        }
    }
    node_list_from_vec(items)
}

pub fn lower_scalar_type_definition(s: &cst::ScalarTypeDefinition, ctx: &LowerCtx) -> Zval {
    let range = s.syntax().text_range();
    let name = s.name().map(|n| name_node(&n, ctx)).unwrap_or(Zval::new());
    let directives = lower_directives_opt(s.directives(), ctx);
    let mut builder = new_node(slots::scalar_type_definition_node::get(), range, ctx)
        .prop("name", name)
        .prop("directives", directives);
    if let Some(desc) = description_zval(s.description(), ctx) {
        builder = builder.prop("description", desc);
    }
    builder.finish()
}

pub fn lower_object_type_definition(o: &cst::ObjectTypeDefinition, ctx: &LowerCtx) -> Zval {
    let range = o.syntax().text_range();
    let name = o.name().map(|n| name_node(&n, ctx)).unwrap_or(Zval::new());
    let interfaces = lower_implements_interfaces(o.implements_interfaces(), ctx);
    let directives = lower_directives_opt(o.directives(), ctx);
    let fields = lower_fields_definition(o.fields_definition(), ctx);
    let mut builder = new_node(slots::object_type_definition_node::get(), range, ctx)
        .prop("name", name)
        .prop("interfaces", interfaces)
        .prop("directives", directives)
        .prop("fields", fields);
    if let Some(desc) = description_zval(o.description(), ctx) {
        builder = builder.prop("description", desc);
    }
    builder.finish()
}

pub fn lower_interface_type_definition(
    i: &cst::InterfaceTypeDefinition,
    ctx: &LowerCtx,
) -> Zval {
    let range = i.syntax().text_range();
    let name = i.name().map(|n| name_node(&n, ctx)).unwrap_or(Zval::new());
    let directives = lower_directives_opt(i.directives(), ctx);
    let interfaces = lower_implements_interfaces(i.implements_interfaces(), ctx);
    let fields = lower_fields_definition(i.fields_definition(), ctx);
    let mut builder = new_node(slots::interface_type_definition_node::get(), range, ctx)
        .prop("name", name)
        .prop("directives", directives)
        .prop("interfaces", interfaces)
        .prop("fields", fields);
    if let Some(desc) = description_zval(i.description(), ctx) {
        builder = builder.prop("description", desc);
    }
    builder.finish()
}

pub fn lower_union_type_definition(u: &cst::UnionTypeDefinition, ctx: &LowerCtx) -> Zval {
    let range = u.syntax().text_range();
    let name = u.name().map(|n| name_node(&n, ctx)).unwrap_or(Zval::new());
    let directives = lower_directives_opt(u.directives(), ctx);
    let mut types_vec: Vec<Zval> = Vec::new();
    if let Some(um) = u.union_member_types() {
        for nt in um.named_types() {
            let name = nt
                .name()
                .map(|n| name_node(&n, ctx))
                .unwrap_or(Zval::new());
            types_vec.push(
                new_node(slots::named_type_node::get(), nt.syntax().text_range(), ctx)
                    .prop("name", name)
                    .finish(),
            );
        }
    }
    let types = node_list_from_vec(types_vec);
    let mut builder = new_node(slots::union_type_definition_node::get(), range, ctx)
        .prop("name", name)
        .prop("directives", directives)
        .prop("types", types);
    if let Some(desc) = description_zval(u.description(), ctx) {
        builder = builder.prop("description", desc);
    }
    builder.finish()
}

pub fn lower_enum_type_definition(e: &cst::EnumTypeDefinition, ctx: &LowerCtx) -> Zval {
    let range = e.syntax().text_range();
    let name = e.name().map(|n| name_node(&n, ctx)).unwrap_or(Zval::new());
    let directives = lower_directives_opt(e.directives(), ctx);
    let mut values_vec: Vec<Zval> = Vec::new();
    if let Some(evd) = e.enum_values_definition() {
        for v in evd.enum_value_definitions() {
            values_vec.push(lower_enum_value_definition(&v, ctx));
        }
    }
    let values = node_list_from_vec(values_vec);
    let mut builder = new_node(slots::enum_type_definition_node::get(), range, ctx)
        .prop("name", name)
        .prop("directives", directives)
        .prop("values", values);
    if let Some(desc) = description_zval(e.description(), ctx) {
        builder = builder.prop("description", desc);
    }
    builder.finish()
}

fn lower_enum_value_definition(
    v: &cst::EnumValueDefinition,
    ctx: &LowerCtx,
) -> Zval {
    let range = v.syntax().text_range();
    let name = v
        .enum_value()
        .and_then(|ev| ev.name())
        .map(|n| name_node(&n, ctx))
        .unwrap_or(Zval::new());
    let directives = lower_directives_opt(v.directives(), ctx);
    let mut builder = new_node(slots::enum_value_definition_node::get(), range, ctx)
        .prop("name", name)
        .prop("directives", directives);
    if let Some(desc) = description_zval(v.description(), ctx) {
        builder = builder.prop("description", desc);
    }
    builder.finish()
}

pub fn lower_input_object_type_definition(
    i: &cst::InputObjectTypeDefinition,
    ctx: &LowerCtx,
) -> Zval {
    let range = i.syntax().text_range();
    let name = i.name().map(|n| name_node(&n, ctx)).unwrap_or(Zval::new());
    let directives = lower_directives_opt(i.directives(), ctx);
    let mut fields_vec: Vec<Zval> = Vec::new();
    if let Some(fd) = i.input_fields_definition() {
        for ivd in fd.input_value_definitions() {
            fields_vec.push(lower_input_value_definition(&ivd, ctx));
        }
    }
    let fields = node_list_from_vec(fields_vec);
    let mut builder = new_node(slots::input_object_type_definition_node::get(), range, ctx)
        .prop("name", name)
        .prop("directives", directives)
        .prop("fields", fields);
    if let Some(desc) = description_zval(i.description(), ctx) {
        builder = builder.prop("description", desc);
    }
    builder.finish()
}

pub fn lower_schema_extension(s: &cst::SchemaExtension, ctx: &LowerCtx) -> Zval {
    let range = s.syntax().text_range();
    let directives = lower_directives_opt(s.directives(), ctx);
    let mut op_types_vec: Vec<Zval> = Vec::new();
    for o in s.root_operation_type_definitions() {
        op_types_vec.push(lower_root_operation_type_definition(&o, ctx));
    }
    let operation_types = node_list_from_vec(op_types_vec);
    new_node(slots::schema_extension_node::get(), range, ctx)
        .prop("directives", directives)
        .prop("operationTypes", operation_types)
        .finish()
}

pub fn lower_scalar_type_extension(s: &cst::ScalarTypeExtension, ctx: &LowerCtx) -> Zval {
    let range = s.syntax().text_range();
    let name = s.name().map(|n| name_node(&n, ctx)).unwrap_or(Zval::new());
    let directives = lower_directives_opt(s.directives(), ctx);
    new_node(slots::scalar_type_extension_node::get(), range, ctx)
        .prop("name", name)
        .prop("directives", directives)
        .finish()
}

pub fn lower_object_type_extension(o: &cst::ObjectTypeExtension, ctx: &LowerCtx) -> Zval {
    let range = o.syntax().text_range();
    let name = o.name().map(|n| name_node(&n, ctx)).unwrap_or(Zval::new());
    let interfaces = lower_implements_interfaces(o.implements_interfaces(), ctx);
    let directives = lower_directives_opt(o.directives(), ctx);
    let fields = lower_fields_definition(o.fields_definition(), ctx);
    new_node(slots::object_type_extension_node::get(), range, ctx)
        .prop("name", name)
        .prop("interfaces", interfaces)
        .prop("directives", directives)
        .prop("fields", fields)
        .finish()
}

pub fn lower_interface_type_extension(
    i: &cst::InterfaceTypeExtension,
    ctx: &LowerCtx,
) -> Zval {
    let range = i.syntax().text_range();
    let name = i.name().map(|n| name_node(&n, ctx)).unwrap_or(Zval::new());
    let directives = lower_directives_opt(i.directives(), ctx);
    let interfaces = lower_implements_interfaces(i.implements_interfaces(), ctx);
    let fields = lower_fields_definition(i.fields_definition(), ctx);
    new_node(slots::interface_type_extension_node::get(), range, ctx)
        .prop("name", name)
        .prop("directives", directives)
        .prop("interfaces", interfaces)
        .prop("fields", fields)
        .finish()
}

pub fn lower_union_type_extension(u: &cst::UnionTypeExtension, ctx: &LowerCtx) -> Zval {
    let range = u.syntax().text_range();
    let name = u.name().map(|n| name_node(&n, ctx)).unwrap_or(Zval::new());
    let directives = lower_directives_opt(u.directives(), ctx);
    let mut types_vec: Vec<Zval> = Vec::new();
    if let Some(um) = u.union_member_types() {
        for nt in um.named_types() {
            let name = nt
                .name()
                .map(|n| name_node(&n, ctx))
                .unwrap_or(Zval::new());
            types_vec.push(
                new_node(slots::named_type_node::get(), nt.syntax().text_range(), ctx)
                    .prop("name", name)
                    .finish(),
            );
        }
    }
    let types = node_list_from_vec(types_vec);
    new_node(slots::union_type_extension_node::get(), range, ctx)
        .prop("name", name)
        .prop("directives", directives)
        .prop("types", types)
        .finish()
}

pub fn lower_enum_type_extension(e: &cst::EnumTypeExtension, ctx: &LowerCtx) -> Zval {
    let range = e.syntax().text_range();
    let name = e.name().map(|n| name_node(&n, ctx)).unwrap_or(Zval::new());
    let directives = lower_directives_opt(e.directives(), ctx);
    let mut values_vec: Vec<Zval> = Vec::new();
    if let Some(evd) = e.enum_values_definition() {
        for v in evd.enum_value_definitions() {
            values_vec.push(lower_enum_value_definition(&v, ctx));
        }
    }
    let values = node_list_from_vec(values_vec);
    new_node(slots::enum_type_extension_node::get(), range, ctx)
        .prop("name", name)
        .prop("directives", directives)
        .prop("values", values)
        .finish()
}

pub fn lower_input_object_type_extension(
    i: &cst::InputObjectTypeExtension,
    ctx: &LowerCtx,
) -> Zval {
    let range = i.syntax().text_range();
    let name = i.name().map(|n| name_node(&n, ctx)).unwrap_or(Zval::new());
    let directives = lower_directives_opt(i.directives(), ctx);
    let mut fields_vec: Vec<Zval> = Vec::new();
    if let Some(fd) = i.input_fields_definition() {
        for ivd in fd.input_value_definitions() {
            fields_vec.push(lower_input_value_definition(&ivd, ctx));
        }
    }
    let fields = node_list_from_vec(fields_vec);
    new_node(slots::input_object_type_extension_node::get(), range, ctx)
        .prop("name", name)
        .prop("directives", directives)
        .prop("fields", fields)
        .finish()
}
