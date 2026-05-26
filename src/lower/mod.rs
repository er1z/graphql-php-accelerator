//! CST → PHP AST lowering for `Parser::parse()`.
//!
//! apollo-parser produces a strongly-typed CST (`apollo_parser::cst::*`); we
//! walk that tree and emit PHP `Zval`s wrapping our registered AST classes.
//! Each entry function takes a fresh `LowerCtx` borrowing the source body and
//! the parsed `Source` PHP object, plus the parser options.

use apollo_parser::cst as cst;
use apollo_parser::cst::CstNode;
use apollo_parser::SyntaxTree;
use ext_php_rs::types::{ZendObject, Zval};

mod ctx;
mod helpers;
mod sdl;
mod selections;
mod types;
mod values;

pub use ctx::LowerCtx;
pub use helpers::{lower_argument, lower_directive, name_node, name_node_with_value, new_node, node_list_from_vec};
pub use sdl::{lower_field_definition_for_partial, lower_input_value_definition_for_partial};
pub use selections::{lower_selection_set_for_partial, lower_variable_definition_for_partial};
pub(crate) use helpers::*;

use crate::classes::slots;

/// Lower the full document. Returns a `DocumentNode` PHP object as a Zval.
pub fn lower_document(tree: &SyntaxTree<cst::Document>, ctx: &LowerCtx) -> Zval {
    let doc = tree.document();
    let mut definitions: Vec<Zval> = Vec::new();
    for def in doc.definitions() {
        if let Some(zv) = lower_definition(&def, ctx) {
            definitions.push(zv);
        }
    }
    let definitions_zv = node_list_from_vec(definitions);

    let mut obj = ZendObject::new(slots::document_node::get());
    let _ = obj.set_property("definitions", definitions_zv);
    let loc = ctx.location_zval(doc.syntax().text_range());
    let _ = obj.set_property("loc", loc);
    let mut zv = Zval::new();
    zv = crate::classes::obj_to_zval(obj);
    zv
}

/// Lower a single top-level definition (executable or SDL).
pub fn lower_definition(def: &cst::Definition, ctx: &LowerCtx) -> Option<Zval> {
    use cst::Definition::*;
    match def {
        OperationDefinition(op) => Some(selections::lower_operation_definition(op, ctx)),
        FragmentDefinition(f) => Some(selections::lower_fragment_definition(f, ctx)),
        DirectiveDefinition(d) => Some(sdl::lower_directive_definition(d, ctx)),
        SchemaDefinition(s) => Some(sdl::lower_schema_definition(s, ctx)),
        ScalarTypeDefinition(s) => Some(sdl::lower_scalar_type_definition(s, ctx)),
        ObjectTypeDefinition(o) => Some(sdl::lower_object_type_definition(o, ctx)),
        InterfaceTypeDefinition(i) => Some(sdl::lower_interface_type_definition(i, ctx)),
        UnionTypeDefinition(u) => Some(sdl::lower_union_type_definition(u, ctx)),
        EnumTypeDefinition(e) => Some(sdl::lower_enum_type_definition(e, ctx)),
        InputObjectTypeDefinition(i) => Some(sdl::lower_input_object_type_definition(i, ctx)),
        SchemaExtension(s) => Some(sdl::lower_schema_extension(s, ctx)),
        ScalarTypeExtension(s) => Some(sdl::lower_scalar_type_extension(s, ctx)),
        ObjectTypeExtension(o) => Some(sdl::lower_object_type_extension(o, ctx)),
        InterfaceTypeExtension(i) => Some(sdl::lower_interface_type_extension(i, ctx)),
        UnionTypeExtension(u) => Some(sdl::lower_union_type_extension(u, ctx)),
        EnumTypeExtension(e) => Some(sdl::lower_enum_type_extension(e, ctx)),
        InputObjectTypeExtension(i) => Some(sdl::lower_input_object_type_extension(i, ctx)),
    }
}

/// Lower a value literal (used by `parseValue`).
pub fn lower_value(v: &cst::Value, ctx: &LowerCtx) -> Zval {
    values::lower_value(v, ctx)
}

/// Lower a type reference (used by `parseType`).
pub fn lower_type(t: &cst::Type, ctx: &LowerCtx) -> Zval {
    types::lower_type(t, ctx)
}
