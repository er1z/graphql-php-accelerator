//! AST node classes:
//!  - `GraphQL\Language\AST\Node` (abstract base, implements JsonSerializable)
//!  - `GraphQL\Language\AST\NodeList` (Phase 1 stub)
//!  - 24 query-language node classes
//!  - 12 SDL definition node classes
//!  - 7 SDL extension node classes
//!
//! Every concrete node:
//!   * extends `Node`
//!   * declares the same public properties as its PHP twin, **in the same
//!     declaration order** (visitors & `Node::recursiveToArray` depend on it)
//!   * implements the same marker interfaces

use ext_php_rs::builders::{ClassBuilder, ClassProperty, FunctionBuilder};
use ext_php_rs::class::ClassEntryInfo;
use ext_php_rs::error::Result;
use ext_php_rs::flags::{DataType, MethodFlags};
use ext_php_rs::types::Zval;
use ext_php_rs::zend::{ClassEntry, ExecuteData};

use super::slots;
use super::{json_serializable, pub_bool_false, pub_nullable, pub_string, pub_uninit};

pub fn register_node_base() -> Result<()> {
    // `Node` is abstract and implements `JsonSerializable`. Properties:
    //   public ?Location $loc = null;
    //   public string $kind;
    // We do NOT set `ClassFlags::Abstract` here: `zend_register_internal_class_ex`
    // propagates the abstract flag to subclasses, which would block `new
    // FieldNode([])`. The on-disk PHP source declares `Node` as `abstract`, but
    // we accept the deviation (a literal `new Node([])` works and returns an
    // uninteresting empty node — nothing in the project tries this).
    //
    // We must declare `jsonSerialize` on Node because `JsonSerializable`
    // contains an abstract method by the same name. Without a concrete
    // implementation, PHP marks the class (and every subclass) as implicitly
    // abstract. Phase 1's stub returns the empty array; Phase 2 wires a real
    // implementation that mirrors `Node::recursiveToArray`.
    ClassBuilder::new(slots::node::PHP_NAME)
        .implements(json_serializable())
        .property(pub_nullable("loc"))
        .property(pub_uninit("kind"))
        .method(
            FunctionBuilder::new("__construct", node_construct)
                .arg(ext_php_rs::args::Arg::new("vars", DataType::Array)),
            MethodFlags::Public,
        )
        .method(
            FunctionBuilder::new("jsonSerialize", node_json_serialize)
                .returns(DataType::Array, false, false),
            MethodFlags::Public,
        )
        .method(
            FunctionBuilder::new("__toString", node_to_string)
                .returns(DataType::String, false, false),
            MethodFlags::Public,
        )
        .method(
            FunctionBuilder::new("toArray", node_to_array)
                .returns(DataType::Array, false, false),
            MethodFlags::Public,
        )
        .method(
            FunctionBuilder::new("cloneDeep", node_clone_deep)
                .returns(DataType::Object(None), false, false),
            MethodFlags::Public,
        )
        .method(
            FunctionBuilder::new("getName", node_get_name)
                .returns(DataType::Object(Some("GraphQL\\Language\\AST\\NameNode")), false, false),
            MethodFlags::Public,
        )
        .method(
            FunctionBuilder::new("getSelectionSet", node_get_selection_set)
                .returns(DataType::Object(Some("GraphQL\\Language\\AST\\SelectionSetNode")), false, false),
            MethodFlags::Public,
        )
        .registration(slots::node::set)
        .register()?;

    // NodeList is *not* registered by the extension. The PHP file at
    // src/Language/AST/NodeList.php implements ArrayAccess +
    // IteratorAggregate + Countable and a non-trivial constructor; Phase 4
    // will port it to a Rust-backed handler, but Phase 2 needs the PHP class
    // to be visible to userland code (visitors, printers, etc.), so we let
    // Composer autoload it on first reference.
    Ok(())
}

pub fn register_query_nodes() -> Result<()> {
    node(slots::name_node::PHP_NAME, slots::name_node::set, vec![], || vec![
        kind("Name"),
        pub_uninit("value"),
    ])?;

    node(slots::document_node::PHP_NAME, slots::document_node::set, vec![], || vec![
        kind("Document"),
        pub_uninit("definitions"),
    ])?;

    node(
        slots::operation_definition_node::PHP_NAME,
        slots::operation_definition_node::set,
        vec![
            slots::executable_definition::INFO,
            slots::has_selection_set::INFO,
        ],
        || vec![
            kind("OperationDefinition"),
            pub_nullable("name"),
            pub_uninit("operation"),
            pub_uninit("variableDefinitions"),
            pub_uninit("directives"),
            pub_uninit("selectionSet"),
        ],
    )?;

    node(
        slots::variable_definition_node::PHP_NAME,
        slots::variable_definition_node::set,
        vec![],
        || vec![
            kind("VariableDefinition"),
            pub_uninit("variable"),
            pub_uninit("type"),
            pub_nullable("defaultValue"),
            pub_uninit("directives"),
        ],
    )?;

    node(
        slots::variable_node::PHP_NAME,
        slots::variable_node::set,
        vec![slots::value::INFO],
        || vec![kind("Variable"), pub_uninit("name")],
    )?;

    node(
        slots::selection_set_node::PHP_NAME,
        slots::selection_set_node::set,
        vec![],
        || vec![kind("SelectionSet"), pub_uninit("selections")],
    )?;

    node(
        slots::field_node::PHP_NAME,
        slots::field_node::set,
        vec![slots::selection::INFO],
        || vec![
            kind("Field"),
            pub_uninit("name"),
            pub_nullable("alias"),
            pub_uninit("arguments"),
            pub_uninit("directives"),
            pub_nullable("selectionSet"),
        ],
    )?;

    node(
        slots::argument_node::PHP_NAME,
        slots::argument_node::set,
        vec![],
        || vec![
            kind("Argument"),
            pub_uninit("value"),
            pub_uninit("name"),
        ],
    )?;

    node(
        slots::fragment_spread_node::PHP_NAME,
        slots::fragment_spread_node::set,
        vec![slots::selection::INFO],
        || vec![
            kind("FragmentSpread"),
            pub_uninit("name"),
            pub_uninit("directives"),
        ],
    )?;

    node(
        slots::inline_fragment_node::PHP_NAME,
        slots::inline_fragment_node::set,
        vec![slots::selection::INFO],
        || vec![
            kind("InlineFragment"),
            pub_nullable("typeCondition"),
            pub_uninit("directives"),
            pub_uninit("selectionSet"),
        ],
    )?;

    node(
        slots::fragment_definition_node::PHP_NAME,
        slots::fragment_definition_node::set,
        vec![
            slots::executable_definition::INFO,
            slots::has_selection_set::INFO,
        ],
        || vec![
            kind("FragmentDefinition"),
            pub_uninit("name"),
            pub_nullable("variableDefinitions"),
            pub_uninit("typeCondition"),
            pub_uninit("directives"),
            pub_uninit("selectionSet"),
        ],
    )?;

    node(
        slots::int_value_node::PHP_NAME,
        slots::int_value_node::set,
        vec![slots::value::INFO],
        || vec![kind("IntValue"), pub_uninit("value")],
    )?;
    node(
        slots::float_value_node::PHP_NAME,
        slots::float_value_node::set,
        vec![slots::value::INFO],
        || vec![kind("FloatValue"), pub_uninit("value")],
    )?;
    node(
        slots::string_value_node::PHP_NAME,
        slots::string_value_node::set,
        vec![slots::value::INFO],
        || vec![
            kind("StringValue"),
            pub_uninit("value"),
            pub_bool_false("block"),
        ],
    )?;
    node(
        slots::boolean_value_node::PHP_NAME,
        slots::boolean_value_node::set,
        vec![slots::value::INFO],
        || vec![kind("BooleanValue"), pub_uninit("value")],
    )?;
    node(
        slots::enum_value_node::PHP_NAME,
        slots::enum_value_node::set,
        vec![slots::value::INFO],
        || vec![kind("EnumValue"), pub_uninit("value")],
    )?;
    node(
        slots::null_value_node::PHP_NAME,
        slots::null_value_node::set,
        vec![slots::value::INFO],
        || vec![kind("NullValue")],
    )?;
    node(
        slots::list_value_node::PHP_NAME,
        slots::list_value_node::set,
        vec![slots::value::INFO],
        || vec![kind("ListValue"), pub_uninit("values")],
    )?;
    node(
        slots::object_value_node::PHP_NAME,
        slots::object_value_node::set,
        vec![slots::value::INFO],
        || vec![kind("ObjectValue"), pub_uninit("fields")],
    )?;
    node(
        slots::object_field_node::PHP_NAME,
        slots::object_field_node::set,
        vec![],
        || vec![
            kind("ObjectField"),
            pub_uninit("name"),
            pub_uninit("value"),
        ],
    )?;
    node(
        slots::directive_node::PHP_NAME,
        slots::directive_node::set,
        vec![],
        || vec![
            kind("Directive"),
            pub_uninit("name"),
            pub_uninit("arguments"),
        ],
    )?;

    // Type-reference nodes
    node(
        slots::named_type_node::PHP_NAME,
        slots::named_type_node::set,
        vec![slots::type_node::INFO],
        || vec![kind("NamedType"), pub_uninit("name")],
    )?;
    node(
        slots::list_type_node::PHP_NAME,
        slots::list_type_node::set,
        vec![slots::type_node::INFO],
        || vec![kind("ListType"), pub_uninit("type")],
    )?;
    node(
        slots::non_null_type_node::PHP_NAME,
        slots::non_null_type_node::set,
        vec![slots::type_node::INFO],
        || vec![kind("NonNullType"), pub_uninit("type")],
    )?;

    Ok(())
}

pub fn register_sdl_definitions() -> Result<()> {
    node(
        slots::schema_definition_node::PHP_NAME,
        slots::schema_definition_node::set,
        vec![slots::type_system_definition::INFO],
        || vec![
            kind("SchemaDefinition"),
            pub_uninit("directives"),
            pub_uninit("operationTypes"),
            pub_nullable("description"),
        ],
    )?;
    node(
        slots::operation_type_definition_node::PHP_NAME,
        slots::operation_type_definition_node::set,
        vec![],
        || vec![
            kind("OperationTypeDefinition"),
            pub_uninit("operation"),
            pub_uninit("type"),
        ],
    )?;
    node(
        slots::scalar_type_definition_node::PHP_NAME,
        slots::scalar_type_definition_node::set,
        vec![slots::type_definition::INFO],
        || vec![
            kind("ScalarTypeDefinition"),
            pub_uninit("name"),
            pub_uninit("directives"),
            pub_nullable("description"),
        ],
    )?;
    node(
        slots::object_type_definition_node::PHP_NAME,
        slots::object_type_definition_node::set,
        vec![slots::type_definition::INFO],
        || vec![
            kind("ObjectTypeDefinition"),
            pub_uninit("name"),
            pub_uninit("interfaces"),
            pub_uninit("directives"),
            pub_uninit("fields"),
            pub_nullable("description"),
        ],
    )?;
    node(
        slots::field_definition_node::PHP_NAME,
        slots::field_definition_node::set,
        vec![],
        || vec![
            kind("FieldDefinition"),
            pub_uninit("name"),
            pub_uninit("arguments"),
            pub_uninit("type"),
            pub_uninit("directives"),
            pub_nullable("description"),
        ],
    )?;
    node(
        slots::input_value_definition_node::PHP_NAME,
        slots::input_value_definition_node::set,
        vec![],
        || vec![
            kind("InputValueDefinition"),
            pub_uninit("name"),
            pub_uninit("type"),
            pub_nullable("defaultValue"),
            pub_uninit("directives"),
            pub_nullable("description"),
        ],
    )?;
    node(
        slots::interface_type_definition_node::PHP_NAME,
        slots::interface_type_definition_node::set,
        vec![slots::type_definition::INFO],
        || vec![
            kind("InterfaceTypeDefinition"),
            pub_uninit("name"),
            pub_uninit("directives"),
            pub_uninit("interfaces"),
            pub_uninit("fields"),
            pub_nullable("description"),
        ],
    )?;
    node(
        slots::union_type_definition_node::PHP_NAME,
        slots::union_type_definition_node::set,
        vec![slots::type_definition::INFO],
        || vec![
            kind("UnionTypeDefinition"),
            pub_uninit("name"),
            pub_uninit("directives"),
            pub_uninit("types"),
            pub_nullable("description"),
        ],
    )?;
    node(
        slots::enum_type_definition_node::PHP_NAME,
        slots::enum_type_definition_node::set,
        vec![slots::type_definition::INFO],
        || vec![
            kind("EnumTypeDefinition"),
            pub_uninit("name"),
            pub_uninit("directives"),
            pub_uninit("values"),
            pub_nullable("description"),
        ],
    )?;
    node(
        slots::enum_value_definition_node::PHP_NAME,
        slots::enum_value_definition_node::set,
        vec![],
        || vec![
            kind("EnumValueDefinition"),
            pub_uninit("name"),
            pub_uninit("directives"),
            pub_nullable("description"),
        ],
    )?;
    node(
        slots::input_object_type_definition_node::PHP_NAME,
        slots::input_object_type_definition_node::set,
        vec![slots::type_definition::INFO],
        || vec![
            kind("InputObjectTypeDefinition"),
            pub_uninit("name"),
            pub_uninit("directives"),
            pub_uninit("fields"),
            pub_nullable("description"),
        ],
    )?;
    node(
        slots::directive_definition_node::PHP_NAME,
        slots::directive_definition_node::set,
        vec![slots::type_system_definition::INFO],
        || vec![
            kind("DirectiveDefinition"),
            pub_uninit("name"),
            pub_nullable("description"),
            pub_uninit("arguments"),
            pub_bool_false("repeatable"),
            pub_uninit("locations"),
        ],
    )?;
    Ok(())
}

pub fn register_sdl_extensions() -> Result<()> {
    node(
        slots::schema_extension_node::PHP_NAME,
        slots::schema_extension_node::set,
        vec![slots::type_system_extension::INFO],
        || vec![
            kind("SchemaExtension"),
            pub_uninit("directives"),
            pub_uninit("operationTypes"),
        ],
    )?;
    node(
        slots::scalar_type_extension_node::PHP_NAME,
        slots::scalar_type_extension_node::set,
        vec![slots::type_extension::INFO],
        || vec![
            kind("ScalarTypeExtension"),
            pub_uninit("name"),
            pub_uninit("directives"),
        ],
    )?;
    node(
        slots::object_type_extension_node::PHP_NAME,
        slots::object_type_extension_node::set,
        vec![slots::type_extension::INFO],
        || vec![
            kind("ObjectTypeExtension"),
            pub_uninit("name"),
            pub_uninit("interfaces"),
            pub_uninit("directives"),
            pub_uninit("fields"),
        ],
    )?;
    node(
        slots::interface_type_extension_node::PHP_NAME,
        slots::interface_type_extension_node::set,
        vec![slots::type_extension::INFO],
        || vec![
            kind("InterfaceTypeExtension"),
            pub_uninit("name"),
            pub_uninit("directives"),
            pub_uninit("interfaces"),
            pub_uninit("fields"),
        ],
    )?;
    node(
        slots::union_type_extension_node::PHP_NAME,
        slots::union_type_extension_node::set,
        vec![slots::type_extension::INFO],
        || vec![
            kind("UnionTypeExtension"),
            pub_uninit("name"),
            pub_uninit("directives"),
            pub_uninit("types"),
        ],
    )?;
    node(
        slots::enum_type_extension_node::PHP_NAME,
        slots::enum_type_extension_node::set,
        vec![slots::type_extension::INFO],
        || vec![
            kind("EnumTypeExtension"),
            pub_uninit("name"),
            pub_uninit("directives"),
            pub_uninit("values"),
        ],
    )?;
    node(
        slots::input_object_type_extension_node::PHP_NAME,
        slots::input_object_type_extension_node::set,
        vec![slots::type_extension::INFO],
        || vec![
            kind("InputObjectTypeExtension"),
            pub_uninit("name"),
            pub_uninit("directives"),
            pub_uninit("fields"),
        ],
    )?;
    Ok(())
}

// --- Helpers ------------------------------------------------------------------

/// Register a concrete AST node class.
///
/// Properties **must** be passed in the same order as the on-disk PHP class
/// declares them — visitors & `Node::recursiveToArray` walk `get_object_vars()`
/// in declaration order.
fn node<F>(
    php_name: &'static str,
    setter: fn(&'static mut ClassEntry),
    extra_interfaces: Vec<ClassEntryInfo>,
    properties: F,
) -> Result<()>
where
    F: FnOnce() -> Vec<ClassProperty>,
{
    let mut b = ClassBuilder::new(php_name).extends(slots::node::INFO);
    for iface in extra_interfaces {
        b = b.implements(iface);
    }
    for prop in properties() {
        b = b.property(prop);
    }
    b.registration(setter).register()
}

/// `kind(value)` → `public string $kind = "<value>";`
fn kind(value: &'static str) -> ClassProperty {
    pub_string("kind", value)
}

/// `Node::__construct(array $vars)` — mirror of `Utils::assign($this, $vars)`.
///
/// For every entry in `$vars`, set the matching public property on `$this`.
/// PHP code that calls `new FieldNode(['name' => …, 'arguments' => …])` flows
/// through here.
extern "C" fn node_construct(ex: &mut ExecuteData, _retval: &mut Zval) {
    let n_args = unsafe { ex.This.u2.num_args } as usize;

    // First, Utils::assign — copy each $vars key into the matching property.
    if n_args > 0 {
        if let Some(arg) = unsafe { ex.zend_call_arg(0) } {
            if let Some(arr) = arg.array() {
                if let Some(this) = ex.get_self() {
                    for (key, value) in arr.iter() {
                        let name = match &key {
                            ext_php_rs::types::ArrayKey::String(s) => s.clone(),
                            ext_php_rs::types::ArrayKey::Str(s) => s.to_string(),
                            ext_php_rs::types::ArrayKey::Long(_) => continue,
                        };
                        let _ = this.set_property(name.as_str(), value.shallow_clone());
                    }
                }
            }
        }
    }

    // Then apply per-class `$this->X ??= new NodeList([])` defaults.
    // Mirrors the on-disk concrete ctors (FieldNode, OperationDefinitionNode,
    // …) so `new XxxNode([])` produces the same shape with or without the
    // extension. Without this, `AST::fromArray` would leave list-typed slots
    // uninitialized and downstream visitors trip on TypeError.
    if let Some(this) = ex.get_self() {
        let kind = this.get_property::<String>("kind").unwrap_or_default();
        for &prop in defaults_for_kind(&kind) {
            // Only set when currently null or undefined.
            let is_initialized = this
                .get_property::<&Zval>(prop)
                .map(|z| !z.is_null())
                .unwrap_or(false);
            if !is_initialized {
                if let Some(empty) = empty_node_list() {
                    let _ = this.set_property(prop, empty);
                }
            }
        }
    }
}

fn defaults_for_kind(kind: &str) -> &'static [&'static str] {
    match kind {
        "OperationDefinition" => &["directives", "variableDefinitions"],
        "FragmentDefinition" => &["directives"],
        "Field" => &["directives", "arguments"],
        "FragmentSpread" => &["directives"],
        "InlineFragment" => &["directives"],
        "VariableDefinition" => &["directives"],
        "Directive" => &["arguments"],
        "DirectiveDefinition" => &["arguments"],
        "EnumTypeDefinition" => &["directives"],
        "EnumTypeExtension" => &["directives"],
        "InputObjectTypeDefinition" => &["directives"],
        _ => &[],
    }
}

/// `new NodeList([])` — wraps the autoloaded PHP class.
fn empty_node_list() -> Option<Zval> {
    let ce = ext_php_rs::zend::ClassEntry::try_find("GraphQL\\Language\\AST\\NodeList")?;
    let mut empty_arr = Zval::new();
    empty_arr.set_hashtable(ext_php_rs::types::ZendHashTable::new());
    let mut obj = ext_php_rs::types::ZendObject::new(ce);
    let _ = obj.try_call_method("__construct", vec![&empty_arr]);
    Some(crate::classes::obj_to_zval(obj))
}

/// `Node::__toString()` — `json_encode($this)`.
extern "C" fn node_to_string(ex: &mut ExecuteData, retval: &mut Zval) {
    let Some(this) = ex.get_self() else {
        let _ = retval.set_string("", false);
        return;
    };
    let mut this_zv = Zval::new();
    // SAFETY: we need a Zval wrapping `this` to pass into json_encode. Use
    // a shallow copy that bumps refcount via set_object.
    this_zv.set_object(this);
    // Re-borrow as object reference for json_encode.
    let res = call_user_function("json_encode", vec![this_zv.shallow_clone()]);
    match res {
        Some(s) if s.is_string() => *retval = s,
        _ => {
            let _ = retval.set_string("", false);
        }
    }
}

/// `Node::toArray()` — `recursiveToArray($this)`.
extern "C" fn node_to_array(ex: &mut ExecuteData, retval: &mut Zval) {
    let Some(this) = ex.get_self() else {
        retval.set_hashtable(ext_php_rs::types::ZendHashTable::new());
        return;
    };
    *retval = recursive_to_array_object(this);
}

/// `Node::jsonSerialize()` — same as `toArray()`.
extern "C" fn node_json_serialize(ex: &mut ExecuteData, retval: &mut Zval) {
    node_to_array(ex, retval);
}

/// `Node::getName()` — returns `$this->name`.
extern "C" fn node_get_name(ex: &mut ExecuteData, retval: &mut Zval) {
    let Some(this) = ex.get_self() else { return };
    if let Ok(name) = this.get_property::<&Zval>("name") {
        *retval = name.shallow_clone();
    }
}

/// `Node::getSelectionSet()` — returns `$this->selectionSet`. Only meaningful
/// on nodes that implement `HasSelectionSet` (OperationDefinitionNode,
/// FragmentDefinitionNode); other nodes never had a `selectionSet` slot.
extern "C" fn node_get_selection_set(ex: &mut ExecuteData, retval: &mut Zval) {
    let Some(this) = ex.get_self() else { return };
    if let Ok(ss) = this.get_property::<&Zval>("selectionSet") {
        *retval = ss.shallow_clone();
    }
}

/// `Node::cloneDeep()` — deep clone of `$this`, mirroring `Node::cloneValue`.
extern "C" fn node_clone_deep(ex: &mut ExecuteData, retval: &mut Zval) {
    let Some(this) = ex.get_self() else { return };
    *retval = deep_clone_object(this);
}

// --- support helpers ---------------------------------------------------------

fn call_user_function(name: &str, args: Vec<Zval>) -> Option<Zval> {
    let mut callable = Zval::new();
    let _ = callable.set_string(name, false);
    let arg_refs: Vec<&dyn ext_php_rs::convert::IntoZvalDyn> = args
        .iter()
        .map(|a| a as &dyn ext_php_rs::convert::IntoZvalDyn)
        .collect();
    callable.try_call(arg_refs).ok()
}

/// Recursively convert a Node object to a PHP array, mirroring
/// `Node::recursiveToArray` in the on-disk PHP source.
fn recursive_to_array_object(node: &mut ext_php_rs::types::ZendObject) -> Zval {
    let mut ht = ext_php_rs::types::ZendHashTable::new();

    let prop_names: Vec<String> = collect_property_names_via_get_object_vars(node);

    for name in prop_names {
        let value = match node.get_property::<&Zval>(&name) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if value.is_null() {
            continue;
        }
        let converted = convert_value_for_to_array(value);
        if !converted.is_null() {
            let _ = ht.insert(name.as_str(), converted);
        }
    }

    let mut zv = Zval::new();
    zv.set_hashtable(ht);
    zv
}

fn convert_value_for_to_array(value: &Zval) -> Zval {
    if value.is_null() {
        return Zval::new();
    }
    if let Some(obj) = value.object() {
        let ce = obj.get_class_entry();
        let name = ce.name().unwrap_or("");
        if name == "GraphQL\\Language\\AST\\NodeList" {
            let mut ht = ext_php_rs::types::ZendHashTable::new();
            let count = obj
                .try_call_method("count", vec![])
                .ok()
                .and_then(|z| z.long())
                .unwrap_or(0);
            for i in 0..count {
                let mut idx_zv = Zval::new();
                idx_zv.set_long(i);
                if let Ok(item) = obj.try_call_method("offsetGet", vec![&idx_zv]) {
                    let inner = convert_value_for_to_array(&item);
                    let _ = ht.push(inner);
                }
            }
            let mut out = Zval::new();
            out.set_hashtable(ht);
            return out;
        }
        if ce.instance_of(slots::node::get()) {
            // Need &mut to call set_object; clone the value first.
            let mut value_clone = value.shallow_clone();
            if let Some(obj_mut) = value_clone.object_mut() {
                return recursive_to_array_object(obj_mut);
            }
        }
        if ce.instance_of(slots::location::get()) {
            let mut ht = ext_php_rs::types::ZendHashTable::new();
            let start = obj.get_property::<i64>("start").unwrap_or(0);
            let end = obj.get_property::<i64>("end").unwrap_or(0);
            let _ = ht.insert("start", start);
            let _ = ht.insert("end", end);
            let mut out = Zval::new();
            out.set_hashtable(ht);
            return out;
        }
    }
    value.shallow_clone()
}

fn collect_property_names_via_get_object_vars(
    node: &mut ext_php_rs::types::ZendObject,
) -> Vec<String> {
    // Call PHP's `get_object_vars($this)`. The returned array's keys are the
    // public properties in declaration order — exactly what we need to mirror
    // the on-disk Node::recursiveToArray walk.
    let mut this_zv = Zval::new();
    this_zv.set_object(node);
    let arg_refs: Vec<&dyn ext_php_rs::convert::IntoZvalDyn> =
        vec![&this_zv as &dyn ext_php_rs::convert::IntoZvalDyn];
    let mut callable = Zval::new();
    let _ = callable.set_string("get_object_vars", false);
    let result = callable.try_call(arg_refs).ok();
    let Some(arr_zv) = result else { return Vec::new() };
    let Some(arr) = arr_zv.array() else { return Vec::new() };
    arr.iter()
        .filter_map(|(k, _)| match k {
            ext_php_rs::types::ArrayKey::String(s) => Some(s),
            ext_php_rs::types::ArrayKey::Str(s) => Some(s.to_string()),
            _ => None,
        })
        .collect()
}

/// Deep-clone a node object, mirroring `Node::cloneDeep` / `cloneValue` from
/// the on-disk PHP source. Preserves `Location` references (they are not
/// deep-cloned per PHP source comment "except Location $loc").
fn deep_clone_object(node: &mut ext_php_rs::types::ZendObject) -> Zval {
    // Use PHP's clone for shallow copy, then walk public properties and
    // recursively clone Node and NodeList values.
    let mut this_zv = Zval::new();
    this_zv.set_object(node);
    // Use `clone` operator via call_user_function isn't trivial; instead
    // re-instantiate the class entry and copy properties.
    let ce = node.get_class_entry();
    let mut cloned_obj = ext_php_rs::types::ZendObject::new(ce);

    let names = collect_property_names_via_get_object_vars(node);
    for name in names {
        if let Ok(v) = node.get_property::<&Zval>(&name) {
            let cloned = clone_value(v);
            let _ = cloned_obj.set_property(name.as_str(), cloned);
        }
    }

    let mut out = Zval::new();
    out = crate::classes::obj_to_zval(cloned_obj);
    out
}

fn clone_value(value: &Zval) -> Zval {
    if value.is_null() {
        return Zval::new();
    }
    if let Some(obj) = value.object() {
        let ce = obj.get_class_entry();
        let name = ce.name().unwrap_or("");
        if name == "GraphQL\\Language\\AST\\NodeList" {
            if let Ok(z) = obj.try_call_method("cloneDeep", vec![]) {
                return z;
            }
            return value.shallow_clone();
        }
        if ce.instance_of(slots::node::get()) {
            let mut clone = value.shallow_clone();
            if let Some(obj_mut) = clone.object_mut() {
                return deep_clone_object(obj_mut);
            }
        }
        return value.shallow_clone();
    }
    value.shallow_clone()
}
