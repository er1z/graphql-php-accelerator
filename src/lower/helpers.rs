//! Low-level Zval/ZendObject construction helpers.

use apollo_parser::cst as cst;
use apollo_parser::cst::CstNode;
use ext_php_rs::types::{ZendObject, Zval};
use ext_php_rs::zend::ClassEntry;

use crate::classes::slots;

use super::LowerCtx;

/// Wrapper around `ZBox<ZendObject>` exposing a fluent `.prop()` builder. The
/// `prop` method takes the value Zval **by value** (consuming it). Reuse a
/// Zval by calling `shallow_clone()` at the call site.
pub struct ZBoxObj(pub ext_php_rs::boxed::ZBox<ZendObject>);

impl ZBoxObj {
    pub fn prop(mut self, name: &str, value: Zval) -> Self {
        let _ = self.0.set_property(name, value);
        self
    }

    pub fn prop_str(mut self, name: &str, value: &str) -> Self {
        let _ = self.0.set_property(name, value);
        self
    }

    pub fn prop_bool(mut self, name: &str, value: bool) -> Self {
        let _ = self.0.set_property(name, value);
        self
    }

    pub fn finish(self) -> Zval {
        let mut zv = Zval::new();
        zv = crate::classes::obj_to_zval(self.0);
        zv
    }
}

/// Build a fresh PHP node instance and pre-set `loc`.
pub fn new_node(ce: &'static ClassEntry, range: apollo_parser::TextRange, ctx: &LowerCtx) -> ZBoxObj {
    let mut obj = ZendObject::new(ce);
    let loc = ctx.location_zval(range);
    let _ = obj.set_property("loc", loc);
    ZBoxObj(obj)
}

/// Construct a `NameNode` from an apollo-parser `Name` cst node.
pub fn name_node(name: &cst::Name, ctx: &LowerCtx) -> Zval {
    let text = name.text().to_string();
    new_node(slots::name_node::get(), name.syntax().text_range(), ctx)
        .prop_str("value", &text)
        .finish()
}

/// Construct a `NameNode` from a free-form value + range. Used by partial
/// parsers (e.g. `directiveLocations`) where the lexer-level token is not a
/// `cst::Name`.
pub fn name_node_with_value(value: &str, range: apollo_parser::TextRange, ctx: &LowerCtx) -> Zval {
    new_node(slots::name_node::get(), range, ctx)
        .prop_str("value", value)
        .finish()
}

/// Trim the "$" off a Variable cst node's name token and lower it as NameNode.
pub fn variable_name_node(var: &cst::Variable, ctx: &LowerCtx) -> Zval {
    let trimmed = var
        .name()
        .map(|n| n.text().to_string())
        .unwrap_or_default();
    let range = var.syntax().text_range();
    let start = u32::from(range.start()) + 1; // skip leading `$`
    let end = u32::from(range.end());
    let pseudo = apollo_parser::TextRange::new(start.into(), end.into());
    new_node(slots::name_node::get(), pseudo, ctx)
        .prop_str("value", &trimmed)
        .finish()
}

/// Build a `GraphQL\Language\AST\NodeList` PHP object wrapping a vector of
/// Zvals. Calls the PHP-side constructor — composer autoload supplies the
/// implementation.
pub fn node_list_from_vec(items: Vec<Zval>) -> Zval {
    let Some(ce) = ClassEntry::try_find("GraphQL\\Language\\AST\\NodeList") else {
        // Safety net only — at runtime composer should have NodeList ready
        // to autoload on first reference.
        return plain_array_from_vec(items);
    };

    let mut arr_zv = Zval::new();
    arr_zv.set_hashtable(items_to_ht(items));

    let mut obj = ZendObject::new(ce);
    let _ = obj.try_call_method("__construct", vec![&arr_zv]);

    let mut zv = Zval::new();
    zv = crate::classes::obj_to_zval(obj);
    zv
}

fn plain_array_from_vec(items: Vec<Zval>) -> Zval {
    let mut zv = Zval::new();
    zv.set_hashtable(items_to_ht(items));
    zv
}

fn items_to_ht(items: Vec<Zval>) -> ext_php_rs::boxed::ZBox<ext_php_rs::types::ZendHashTable> {
    let mut ht = ext_php_rs::types::ZendHashTable::new();
    for item in items {
        let _ = ht.push(item);
    }
    ht
}

/// Lower an optional Directives CST node into a NodeList of DirectiveNodes.
pub fn lower_directives_opt(dirs: Option<cst::Directives>, ctx: &LowerCtx) -> Zval {
    let mut out: Vec<Zval> = Vec::new();
    if let Some(d) = dirs {
        for d in d.directives() {
            out.push(lower_directive(&d, ctx));
        }
    }
    node_list_from_vec(out)
}

pub fn lower_directive(d: &cst::Directive, ctx: &LowerCtx) -> Zval {
    let name = d.name().map(|n| name_node(&n, ctx)).unwrap_or_else(Zval::new);
    let mut args_vec: Vec<Zval> = Vec::new();
    if let Some(args) = d.arguments() {
        for a in args.arguments() {
            args_vec.push(lower_argument(&a, ctx));
        }
    }
    let arguments = node_list_from_vec(args_vec);
    new_node(slots::directive_node::get(), d.syntax().text_range(), ctx)
        .prop("name", name)
        .prop("arguments", arguments)
        .finish()
}

pub fn lower_argument(a: &cst::Argument, ctx: &LowerCtx) -> Zval {
    let name = a.name().map(|n| name_node(&n, ctx)).unwrap_or_else(Zval::new);
    let value = a
        .value()
        .map(|v| super::values::lower_value(&v, ctx))
        .unwrap_or_else(Zval::new);
    new_node(slots::argument_node::get(), a.syntax().text_range(), ctx)
        .prop("value", value)
        .prop("name", name)
        .finish()
}
