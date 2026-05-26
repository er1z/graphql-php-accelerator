//! Marker interfaces for the AST node hierarchy.
//!
//! `Node` is the abstract base class; we register it here so that it can be
//! `extended` from each concrete node class registered in `nodes.rs`. The 10
//! marker interfaces below have **no** methods — they exist solely so that
//! `$node instanceof SelectionNode` etc. continues to work.

use ext_php_rs::builders::ClassBuilder;
use ext_php_rs::error::Result;
use ext_php_rs::flags::ClassFlags;

use super::slots;

pub fn register_all() -> Result<()> {
    iface(slots::definition::PHP_NAME, slots::definition::set, None)?;

    iface(
        slots::executable_definition::PHP_NAME,
        slots::executable_definition::set,
        Some(slots::definition::INFO),
    )?;

    iface(slots::selection::PHP_NAME,         slots::selection::set,         None)?;
    iface(slots::type_node::PHP_NAME,         slots::type_node::set,         None)?;
    iface(slots::value::PHP_NAME,             slots::value::set,             None)?;
    iface(slots::has_selection_set::PHP_NAME, slots::has_selection_set::set, None)?;

    iface(
        slots::type_system_definition::PHP_NAME,
        slots::type_system_definition::set,
        Some(slots::definition::INFO),
    )?;
    iface(
        slots::type_system_extension::PHP_NAME,
        slots::type_system_extension::set,
        Some(slots::definition::INFO),
    )?;
    iface(
        slots::type_definition::PHP_NAME,
        slots::type_definition::set,
        Some(slots::type_system_definition::INFO),
    )?;
    iface(
        slots::type_extension::PHP_NAME,
        slots::type_extension::set,
        Some(slots::type_system_extension::INFO),
    )?;
    Ok(())
}

fn iface(
    name: &'static str,
    setter: fn(&'static mut ext_php_rs::zend::ClassEntry),
    extends: Option<ext_php_rs::class::ClassEntryInfo>,
) -> Result<()> {
    // For interface-to-interface inheritance we go through `.implements()` —
    // `zend_register_internal_interface` ignores the `extends` slot, but the
    // post-register loop wires up implements via `zend_do_implement_interface`,
    // which is what makes `$x instanceof ParentIface` true.
    let mut b = ClassBuilder::new(name).flags(ClassFlags::Interface);
    if let Some(parent) = extends {
        b = b.implements(parent);
    }
    b.registration(setter).register()
}
