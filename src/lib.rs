//! Native PHP extension that accelerates webonyx/graphql-php.

use ext_php_rs::prelude::*;

mod classes;
mod errors;
mod lower;
mod options;
mod source;
mod tokens;

#[php_module]
#[php(startup = startup)]
pub fn module(module: ModuleBuilder) -> ModuleBuilder {
    module
}

unsafe fn startup(_ty: i32, _mod_num: i32) -> i32 {
    match classes::register_all() {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("graphql_accelerator MINIT failure: {err:?}");
            1
        }
    }
}
