//! Class registration for the extension.
//!
//! Every class declared here is installed during `MINIT`, so that
//! `class_exists('GraphQL\\…', false)` returns `true` before the Composer
//! autoloader has a chance to require the PHP source files.

use ext_php_rs::builders::ClassProperty;
use ext_php_rs::flags::PropertyFlags;
use ext_php_rs::types::Zval;

pub mod slots;
mod support;
mod interfaces;
mod nodes;
mod parser;

/// `ClassEntryInfo` for `JsonSerializable` (an ext/json built-in interface).
/// `ClassEntryInfo` for `JsonSerializable` (an ext/json built-in interface).
///
/// `ext-php-rs`'s `ClassEntry::try_find` looks up classes in `EG(class_table)`,
/// which is `NULL` during MINIT — the place where our user-side `startup`
/// runs. We bypass that by referencing the statically-linked symbol
/// `php_json_serializable_ce` from ext/json directly.
/// Convert a `ZBox<ZendObject>` into an owned `Zval` without leaking a refcount.
///
/// `ext-php-rs` 0.15.x's `Zval::set_object` calls `inc_count()` internally, so
/// the naive `zv = crate::classes::obj_to_zval(obj)` pattern leaves the object with
/// `refcount = 2`. That leaks one ref per object — over many objects PHP's
/// arena fills up with unreachable Zvals and eventually corrupts when the
/// engine bookkeeping diverges from reality. This helper mirrors what
/// `<ZBox<ZendObject> as IntoZval>::set_zval` does: dec before into_raw so
/// the net refcount stays at 1.
pub fn obj_to_zval(obj: ext_php_rs::boxed::ZBox<ext_php_rs::types::ZendObject>) -> Zval {
    use ext_php_rs::convert::IntoZval;
    let mut zv = Zval::new();
    let _ = IntoZval::set_zval(obj, &mut zv, false);
    zv
}

pub fn json_serializable() -> ext_php_rs::class::ClassEntryInfo {
    extern "C" {
        static mut php_json_serializable_ce: *mut ext_php_rs::zend::ClassEntry;
    }
    fn ce() -> &'static ext_php_rs::zend::ClassEntry {
        unsafe {
            let p = php_json_serializable_ce;
            assert!(!p.is_null(), "php_json_serializable_ce is null — ext/json not loaded?");
            &*p
        }
    }
    (ce, "JsonSerializable")
}

pub fn register_all() -> ext_php_rs::error::Result<()> {
    // Order matters: parents/interfaces before children.
    support::register_pre_node()?;       // SourceLocation, Source, Token, Lexer, DirectiveLocation, NodeKind, Location
    interfaces::register_all()?;          // 11 marker interfaces (incl. Node)
    nodes::register_node_base()?;         // Node (abstract), NodeList
    nodes::register_query_nodes()?;       // 24 query-language node classes
    nodes::register_sdl_definitions()?;   // 12 SDL definition node classes
    nodes::register_sdl_extensions()?;    // 7 SDL extension node classes
    parser::register()?;                  // Parser (entry point — mutes Parser.php)
    Ok(())
}

// --- ClassProperty helpers ----------------------------------------------------

/// `public string $name = "<default>";`
pub(crate) fn pub_string(name: &'static str, default: &'static str) -> ClassProperty {
    let owned = default.to_string();
    ClassProperty {
        name: name.into(),
        flags: PropertyFlags::Public,
        default: Some(Box::new(move || {
            let mut zv = Zval::new();
            zv.set_string(&owned, true)?;
            Ok(zv)
        })),
        docs: &[],
        ty: None,
        nullable: false,
        readonly: false,
        default_stub: None,
    }
}

/// `public ?Foo $name = null;` — typed-nullable property, defaults to null.
pub(crate) fn pub_nullable(name: &'static str) -> ClassProperty {
    ClassProperty {
        name: name.into(),
        flags: PropertyFlags::Public,
        default: Some(Box::new(|| Ok(Zval::new()))),
        docs: &[],
        ty: None,
        nullable: true,
        readonly: false,
        default_stub: Some("null".into()),
    }
}

/// `public Foo $name;` — declared but uninitialized (no default).
pub(crate) fn pub_uninit(name: &'static str) -> ClassProperty {
    ClassProperty {
        name: name.into(),
        flags: PropertyFlags::Public,
        default: None,
        docs: &[],
        ty: None,
        nullable: false,
        readonly: false,
        default_stub: None,
    }
}

/// `public bool $name = false;`
pub(crate) fn pub_bool_false(name: &'static str) -> ClassProperty {
    ClassProperty {
        name: name.into(),
        flags: PropertyFlags::Public,
        default: Some(Box::new(|| {
            let mut zv = Zval::new();
            zv.set_bool(false);
            Ok(zv)
        })),
        docs: &[],
        ty: None,
        nullable: false,
        readonly: false,
        default_stub: Some("false".into()),
    }
}

/// `public int $name = <n>;`
pub(crate) fn pub_int(name: &'static str, value: i64) -> ClassProperty {
    ClassProperty {
        name: name.into(),
        flags: PropertyFlags::Public,
        default: Some(Box::new(move || {
            let mut zv = Zval::new();
            zv.set_long(value);
            Ok(zv)
        })),
        docs: &[],
        ty: None,
        nullable: false,
        readonly: false,
        default_stub: Some(value.to_string()),
    }
}
