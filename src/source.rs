//! Build / extract a `GraphQL\Language\Source` PHP object.

use ext_php_rs::types::{ZendObject, Zval};

use crate::classes::slots;

pub struct SourceInput {
    pub body: String,
    /// The Zval representation of the `Source` (a PHP object). Re-used across
    /// every `Location` produced by this parse.
    pub source_zv: Zval,
}

/// Resolve the user-supplied source into a UTF-8 body string and a `Source`
/// object zval. If they passed a `Source` instance already, we re-use it.
pub fn coerce_source(arg: &Zval) -> Result<SourceInput, &'static str> {
    if let Some(obj) = arg.object() {
        if obj.instance_of(slots::source::get()) {
            let body = obj
                .get_property::<String>("body")
                .map_err(|_| "Source missing $body")?;
            return Ok(SourceInput {
                body,
                source_zv: arg.shallow_clone(),
            });
        }
    }
    if let Some(s) = arg.str() {
        let body = s.to_string();
        let source_zv = build_source_object(&body, None);
        return Ok(SourceInput { body, source_zv });
    }
    Err("source must be a string or an instance of GraphQL\\Language\\Source")
}

/// Construct a `Source` PHP object with `body` set; other fields keep their
/// declared defaults (`length = 0`, `name = "GraphQL request"`, `locationOffset
/// = null`).
pub fn build_source_object(body: &str, name: Option<&str>) -> Zval {
    let ce = slots::source::get();
    let mut obj = ZendObject::new(ce);
    let _ = obj.set_property("body", body);
    let _ = obj.set_property("length", body.chars().count() as i64);
    if let Some(n) = name {
        if !n.is_empty() {
            let _ = obj.set_property("name", n);
        }
    }
    // locationOffset must be initialized (typed property; PHP errors on null
    // access). Default to SourceLocation(1, 1) — same as the PHP ctor.
    let mut sl = ZendObject::new(slots::source_location::get());
    let _ = sl.set_property("line", 1_i64);
    let _ = sl.set_property("column", 1_i64);
    let _ = obj.set_property("locationOffset", crate::classes::obj_to_zval(sl));

    crate::classes::obj_to_zval(obj)
}
