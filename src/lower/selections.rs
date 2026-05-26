//! Lower executable-language constructs: OperationDefinition, FragmentDefinition,
//! SelectionSet, Field, FragmentSpread, InlineFragment, VariableDefinitions.

use apollo_parser::cst as cst;
use apollo_parser::cst::CstNode;
use ext_php_rs::types::Zval;

use crate::classes::slots;
use super::helpers::*;
use super::types::lower_type;
use super::values::lower_value;
use super::LowerCtx;

pub fn lower_operation_definition(op: &cst::OperationDefinition, ctx: &LowerCtx) -> Zval {
    let range = op.syntax().text_range();
    let name = op.name().map(|n| name_node(&n, ctx));
    let operation_str = op
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

    let var_defs = lower_variable_definitions(op.variable_definitions(), ctx);
    let directives = lower_directives_opt(op.directives(), ctx);
    let selection_set = op
        .selection_set()
        .map(|s| lower_selection_set(&s, ctx))
        .unwrap_or(Zval::new());

    let mut builder = new_node(slots::operation_definition_node::get(), range, ctx);
    if let Some(name_zv) = name {
        builder = builder.prop("name", name_zv);
    }
    builder
        .prop_str("operation", operation_str)
        .prop("variableDefinitions", var_defs)
        .prop("directives", directives)
        .prop("selectionSet", selection_set)
        .finish()
}

pub fn lower_fragment_definition(f: &cst::FragmentDefinition, ctx: &LowerCtx) -> Zval {
    let range = f.syntax().text_range();
    let name = f
        .fragment_name()
        .and_then(|fn_| fn_.name())
        .map(|n| name_node(&n, ctx))
        .unwrap_or(Zval::new());
    let type_condition = f
        .type_condition()
        .and_then(|tc| tc.named_type())
        .map(|nt| {
            let name = nt.name().map(|n| name_node(&n, ctx)).unwrap_or(Zval::new());
            new_node(slots::named_type_node::get(), nt.syntax().text_range(), ctx)
                .prop("name", name)
                .finish()
        })
        .unwrap_or(Zval::new());
    let directives = lower_directives_opt(f.directives(), ctx);
    let selection_set = f
        .selection_set()
        .map(|s| lower_selection_set(&s, ctx))
        .unwrap_or(Zval::new());

    new_node(slots::fragment_definition_node::get(), range, ctx)
        .prop("name", name)
        .prop("typeCondition", type_condition)
        .prop("directives", directives)
        .prop("selectionSet", selection_set)
        .finish()
}

pub fn lower_selection_set(ss: &cst::SelectionSet, ctx: &LowerCtx) -> Zval {
    let mut items: Vec<Zval> = Vec::new();
    for sel in ss.selections() {
        items.push(lower_selection(&sel, ctx));
    }
    let selections = node_list_from_vec(items);
    new_node(slots::selection_set_node::get(), ss.syntax().text_range(), ctx)
        .prop("selections", selections)
        .finish()
}

fn lower_selection(sel: &cst::Selection, ctx: &LowerCtx) -> Zval {
    use cst::Selection::*;
    match sel {
        Field(f) => lower_field(f, ctx),
        FragmentSpread(fs) => lower_fragment_spread(fs, ctx),
        InlineFragment(inf) => lower_inline_fragment(inf, ctx),
    }
}

fn lower_field(f: &cst::Field, ctx: &LowerCtx) -> Zval {
    let range = f.syntax().text_range();
    let alias = f
        .alias()
        .and_then(|a| a.name())
        .map(|n| name_node(&n, ctx));
    let name = f.name().map(|n| name_node(&n, ctx)).unwrap_or(Zval::new());
    let mut args_vec: Vec<Zval> = Vec::new();
    if let Some(args) = f.arguments() {
        for a in args.arguments() {
            args_vec.push(lower_argument(&a, ctx));
        }
    }
    let arguments = node_list_from_vec(args_vec);
    let directives = lower_directives_opt(f.directives(), ctx);
    let selection_set = f.selection_set().map(|s| lower_selection_set(&s, ctx));

    let mut builder = new_node(slots::field_node::get(), range, ctx)
        .prop("name", name);
    if let Some(a) = alias {
        builder = builder.prop("alias", a);
    }
    builder = builder
        .prop("arguments", arguments)
        .prop("directives", directives);
    if let Some(ss) = selection_set {
        builder = builder.prop("selectionSet", ss);
    }
    builder.finish()
}

fn lower_fragment_spread(fs: &cst::FragmentSpread, ctx: &LowerCtx) -> Zval {
    let range = fs.syntax().text_range();
    let name = fs
        .fragment_name()
        .and_then(|fn_| fn_.name())
        .map(|n| name_node(&n, ctx))
        .unwrap_or(Zval::new());
    let directives = lower_directives_opt(fs.directives(), ctx);
    new_node(slots::fragment_spread_node::get(), range, ctx)
        .prop("name", name)
        .prop("directives", directives)
        .finish()
}

fn lower_inline_fragment(inf: &cst::InlineFragment, ctx: &LowerCtx) -> Zval {
    let range = inf.syntax().text_range();
    let type_condition = inf
        .type_condition()
        .and_then(|tc| tc.named_type())
        .map(|nt| {
            let name = nt.name().map(|n| name_node(&n, ctx)).unwrap_or(Zval::new());
            new_node(slots::named_type_node::get(), nt.syntax().text_range(), ctx)
                .prop("name", name)
                .finish()
        });
    let directives = lower_directives_opt(inf.directives(), ctx);
    let selection_set = inf
        .selection_set()
        .map(|s| lower_selection_set(&s, ctx))
        .unwrap_or(Zval::new());

    let mut builder = new_node(slots::inline_fragment_node::get(), range, ctx);
    if let Some(tc) = type_condition {
        builder = builder.prop("typeCondition", tc);
    }
    builder
        .prop("directives", directives)
        .prop("selectionSet", selection_set)
        .finish()
}

fn lower_variable_definitions(
    vds: Option<cst::VariableDefinitions>,
    ctx: &LowerCtx,
) -> Zval {
    let mut items: Vec<Zval> = Vec::new();
    if let Some(vds) = vds {
        for v in vds.variable_definitions() {
            items.push(lower_variable_definition(&v, ctx));
        }
    }
    node_list_from_vec(items)
}

pub fn lower_selection_set_for_partial(
    ss: &cst::SelectionSet,
    ctx: &LowerCtx,
) -> Zval {
    lower_selection_set(ss, ctx)
}

pub fn lower_variable_definition_for_partial(
    v: &cst::VariableDefinition,
    ctx: &LowerCtx,
) -> Zval {
    lower_variable_definition(v, ctx)
}

fn lower_variable_definition(v: &cst::VariableDefinition, ctx: &LowerCtx) -> Zval {
    let range = v.syntax().text_range();
    let variable = v
        .variable()
        .map(|var| {
            let name = variable_name_node(&var, ctx);
            new_node(slots::variable_node::get(), var.syntax().text_range(), ctx)
                .prop("name", name)
                .finish()
        })
        .unwrap_or(Zval::new());
    let typ = v.ty().map(|t| lower_type(&t, ctx)).unwrap_or(Zval::new());
    let default_value = v
        .default_value()
        .and_then(|dv| dv.value())
        .map(|val| lower_value(&val, ctx));
    let directives = lower_directives_opt(v.directives(), ctx);

    let mut builder = new_node(slots::variable_definition_node::get(), range, ctx)
        .prop("variable", variable)
        .prop("type", typ);
    if let Some(dv) = default_value {
        builder = builder.prop("defaultValue", dv);
    }
    builder.prop("directives", directives).finish()
}
