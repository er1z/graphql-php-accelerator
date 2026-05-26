//! Lower GraphQL type references.

use apollo_parser::cst as cst;
use apollo_parser::cst::CstNode;
use ext_php_rs::types::Zval;

use crate::classes::slots;
use super::helpers::*;
use super::LowerCtx;

pub fn lower_type(t: &cst::Type, ctx: &LowerCtx) -> Zval {
    use cst::Type::*;
    let range = t.syntax().text_range();
    match t {
        NamedType(n) => {
            let name = n.name().map(|n| name_node(&n, ctx)).unwrap_or(Zval::new());
            new_node(slots::named_type_node::get(), range, ctx)
                .prop("name", name)
                .finish()
        }
        ListType(l) => {
            let inner = l
                .ty()
                .map(|inner_t| lower_type(&inner_t, ctx))
                .unwrap_or(Zval::new());
            new_node(slots::list_type_node::get(), range, ctx)
                .prop("type", inner)
                .finish()
        }
        NonNullType(n) => {
            let inner = if let Some(named) = n.named_type() {
                let name = named
                    .name()
                    .map(|nn| name_node(&nn, ctx))
                    .unwrap_or(Zval::new());
                new_node(
                    slots::named_type_node::get(),
                    named.syntax().text_range(),
                    ctx,
                )
                .prop("name", name)
                .finish()
            } else if let Some(list) = n.list_type() {
                let inner_t = list
                    .ty()
                    .map(|inner| lower_type(&inner, ctx))
                    .unwrap_or(Zval::new());
                new_node(
                    slots::list_type_node::get(),
                    list.syntax().text_range(),
                    ctx,
                )
                .prop("type", inner_t)
                .finish()
            } else {
                Zval::new()
            };
            new_node(slots::non_null_type_node::get(), range, ctx)
                .prop("type", inner)
                .finish()
        }
    }
}
