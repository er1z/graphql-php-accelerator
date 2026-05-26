//! Per-class static slots and `ClassEntryInfo` getters.
//!
//! `ext-php-rs`'s `ClassBuilder::register` requires a registration callback
//! `fn(&'static mut ClassEntry)` for storing the freshly-built class pointer
//! somewhere we can retrieve it later (notably for `extends` / `implements`
//! relationships).
//!
//! Each class gets its own module with:
//!  - `static mut CE: Option<&ClassEntry>` (set during MINIT, read forever after)
//!  - `pub fn get() -> &'static ClassEntry`
//!  - `pub fn set(ce: &'static mut ClassEntry)` (passed to `.registration(...)`)
//!  - `pub const INFO: ClassEntryInfo` (passed to `.extends(...)` / `.implements(...)`)
//!  - `pub const PHP_NAME: &str` (passed to `ClassBuilder::new(...)`)

#[macro_export]
macro_rules! class_slot {
    ($vis:vis $name:ident, $php_name:literal) => {
        $vis mod $name {
            use ::ext_php_rs::class::ClassEntryInfo;
            use ::ext_php_rs::zend::ClassEntry;
            static mut CE: Option<&'static ClassEntry> = None;

            #[allow(static_mut_refs, dead_code)]
            pub fn get() -> &'static ClassEntry {
                unsafe { CE.expect(concat!("not registered: ", $php_name)) }
            }
            pub fn set(ce: &'static mut ClassEntry) {
                unsafe { CE = Some(ce); }
            }
            pub const PHP_NAME: &'static str = $php_name;
            #[allow(dead_code)]
            pub const INFO: ClassEntryInfo = (get, $php_name);
        }
    };
}

// --- Support classes ----------------------------------------------------------
class_slot!(pub source_location,    "GraphQL\\Language\\SourceLocation");
class_slot!(pub source,             "GraphQL\\Language\\Source");
class_slot!(pub token,              "GraphQL\\Language\\Token");
class_slot!(pub lexer,              "GraphQL\\Language\\Lexer");
class_slot!(pub directive_location, "GraphQL\\Language\\DirectiveLocation");
class_slot!(pub node_kind,          "GraphQL\\Language\\AST\\NodeKind");
class_slot!(pub location,           "GraphQL\\Language\\AST\\Location");

// --- Node base & list ---------------------------------------------------------
class_slot!(pub node,               "GraphQL\\Language\\AST\\Node");
class_slot!(pub node_list,          "GraphQL\\Language\\AST\\NodeList");

// --- Marker interfaces --------------------------------------------------------
class_slot!(pub definition,            "GraphQL\\Language\\AST\\DefinitionNode");
class_slot!(pub executable_definition, "GraphQL\\Language\\AST\\ExecutableDefinitionNode");
class_slot!(pub selection,             "GraphQL\\Language\\AST\\SelectionNode");
class_slot!(pub type_node,             "GraphQL\\Language\\AST\\TypeNode");
class_slot!(pub value,                 "GraphQL\\Language\\AST\\ValueNode");
class_slot!(pub has_selection_set,     "GraphQL\\Language\\AST\\HasSelectionSet");
class_slot!(pub type_system_definition, "GraphQL\\Language\\AST\\TypeSystemDefinitionNode");
class_slot!(pub type_system_extension,  "GraphQL\\Language\\AST\\TypeSystemExtensionNode");
class_slot!(pub type_definition,        "GraphQL\\Language\\AST\\TypeDefinitionNode");
class_slot!(pub type_extension,         "GraphQL\\Language\\AST\\TypeExtensionNode");

// --- Query-language node classes (24) -----------------------------------------
class_slot!(pub name_node,                  "GraphQL\\Language\\AST\\NameNode");
class_slot!(pub document_node,              "GraphQL\\Language\\AST\\DocumentNode");
class_slot!(pub operation_definition_node,  "GraphQL\\Language\\AST\\OperationDefinitionNode");
class_slot!(pub variable_definition_node,   "GraphQL\\Language\\AST\\VariableDefinitionNode");
class_slot!(pub variable_node,              "GraphQL\\Language\\AST\\VariableNode");
class_slot!(pub selection_set_node,         "GraphQL\\Language\\AST\\SelectionSetNode");
class_slot!(pub field_node,                 "GraphQL\\Language\\AST\\FieldNode");
class_slot!(pub argument_node,              "GraphQL\\Language\\AST\\ArgumentNode");
class_slot!(pub fragment_spread_node,       "GraphQL\\Language\\AST\\FragmentSpreadNode");
class_slot!(pub inline_fragment_node,       "GraphQL\\Language\\AST\\InlineFragmentNode");
class_slot!(pub fragment_definition_node,   "GraphQL\\Language\\AST\\FragmentDefinitionNode");
class_slot!(pub int_value_node,             "GraphQL\\Language\\AST\\IntValueNode");
class_slot!(pub float_value_node,           "GraphQL\\Language\\AST\\FloatValueNode");
class_slot!(pub string_value_node,          "GraphQL\\Language\\AST\\StringValueNode");
class_slot!(pub boolean_value_node,         "GraphQL\\Language\\AST\\BooleanValueNode");
class_slot!(pub enum_value_node,            "GraphQL\\Language\\AST\\EnumValueNode");
class_slot!(pub null_value_node,            "GraphQL\\Language\\AST\\NullValueNode");
class_slot!(pub list_value_node,            "GraphQL\\Language\\AST\\ListValueNode");
class_slot!(pub object_value_node,          "GraphQL\\Language\\AST\\ObjectValueNode");
class_slot!(pub object_field_node,          "GraphQL\\Language\\AST\\ObjectFieldNode");
class_slot!(pub directive_node,             "GraphQL\\Language\\AST\\DirectiveNode");
class_slot!(pub named_type_node,            "GraphQL\\Language\\AST\\NamedTypeNode");
class_slot!(pub list_type_node,             "GraphQL\\Language\\AST\\ListTypeNode");
class_slot!(pub non_null_type_node,         "GraphQL\\Language\\AST\\NonNullTypeNode");

// --- SDL definition node classes (12) -----------------------------------------
class_slot!(pub schema_definition_node,        "GraphQL\\Language\\AST\\SchemaDefinitionNode");
class_slot!(pub operation_type_definition_node, "GraphQL\\Language\\AST\\OperationTypeDefinitionNode");
class_slot!(pub scalar_type_definition_node,    "GraphQL\\Language\\AST\\ScalarTypeDefinitionNode");
class_slot!(pub object_type_definition_node,    "GraphQL\\Language\\AST\\ObjectTypeDefinitionNode");
class_slot!(pub field_definition_node,          "GraphQL\\Language\\AST\\FieldDefinitionNode");
class_slot!(pub input_value_definition_node,    "GraphQL\\Language\\AST\\InputValueDefinitionNode");
class_slot!(pub interface_type_definition_node, "GraphQL\\Language\\AST\\InterfaceTypeDefinitionNode");
class_slot!(pub union_type_definition_node,     "GraphQL\\Language\\AST\\UnionTypeDefinitionNode");
class_slot!(pub enum_type_definition_node,      "GraphQL\\Language\\AST\\EnumTypeDefinitionNode");
class_slot!(pub enum_value_definition_node,     "GraphQL\\Language\\AST\\EnumValueDefinitionNode");
class_slot!(pub input_object_type_definition_node, "GraphQL\\Language\\AST\\InputObjectTypeDefinitionNode");
class_slot!(pub directive_definition_node,      "GraphQL\\Language\\AST\\DirectiveDefinitionNode");

// --- SDL extension node classes (7) -------------------------------------------
class_slot!(pub schema_extension_node,             "GraphQL\\Language\\AST\\SchemaExtensionNode");
class_slot!(pub scalar_type_extension_node,        "GraphQL\\Language\\AST\\ScalarTypeExtensionNode");
class_slot!(pub object_type_extension_node,        "GraphQL\\Language\\AST\\ObjectTypeExtensionNode");
class_slot!(pub interface_type_extension_node,     "GraphQL\\Language\\AST\\InterfaceTypeExtensionNode");
class_slot!(pub union_type_extension_node,         "GraphQL\\Language\\AST\\UnionTypeExtensionNode");
class_slot!(pub enum_type_extension_node,          "GraphQL\\Language\\AST\\EnumTypeExtensionNode");
class_slot!(pub input_object_type_extension_node,  "GraphQL\\Language\\AST\\InputObjectTypeExtensionNode");

// --- Parser entry point -------------------------------------------------------
class_slot!(pub parser, "GraphQL\\Language\\Parser");
