//! `ParserOptions` — the second argument to `Parser::parse()` and friends.
//!
//! PHP signature: `Parser::parse(string|Source $source, array $options = [])`.
//! Recognised keys:
//!
//! | key | PHP type | default |
//! | --- | -------- | ------- |
//! | `noLocation` | `bool` | `false` |
//! | `allowLegacySDLEmptyFields` | `bool` | `false` |
//! | `allowLegacySDLImplementsInterfaces` | `bool` | `false` |
//! | `experimentalFragmentVariables` | `bool` | `false` |
//! | `recursionLimit` | `int` (0 ⇒ unlimited) | 256 |

use ext_php_rs::error::Result as ZResult;
use ext_php_rs::types::Zval;

pub const DEFAULT_RECURSION_LIMIT: usize = 256;

#[derive(Debug, Clone, Copy)]
pub struct ParserOptions {
    pub no_location: bool,
    pub allow_legacy_sdl_empty_fields: bool,
    pub allow_legacy_sdl_implements_interfaces: bool,
    pub experimental_fragment_variables: bool,
    pub recursion_limit: usize,
}

impl Default for ParserOptions {
    fn default() -> Self {
        Self {
            no_location: false,
            allow_legacy_sdl_empty_fields: false,
            allow_legacy_sdl_implements_interfaces: false,
            experimental_fragment_variables: false,
            recursion_limit: DEFAULT_RECURSION_LIMIT,
        }
    }
}

impl ParserOptions {
    /// Parse from a userland PHP array. `None` and `null` ⇒ defaults.
    pub fn from_zval(zv: Option<&Zval>) -> ZResult<Self> {
        let mut out = Self::default();
        let Some(zv) = zv else { return Ok(out) };
        if zv.is_null() {
            return Ok(out);
        }
        let Some(ht) = zv.array() else { return Ok(out) };

        if let Some(v) = ht.get("noLocation").and_then(|z| z.bool()) {
            out.no_location = v;
        }
        if let Some(v) = ht.get("allowLegacySDLEmptyFields").and_then(|z| z.bool()) {
            out.allow_legacy_sdl_empty_fields = v;
        }
        if let Some(v) = ht.get("allowLegacySDLImplementsInterfaces").and_then(|z| z.bool()) {
            out.allow_legacy_sdl_implements_interfaces = v;
        }
        if let Some(v) = ht.get("experimentalFragmentVariables").and_then(|z| z.bool()) {
            out.experimental_fragment_variables = v;
        }
        if let Some(z) = ht.get("recursionLimit") {
            if let Some(n) = z.long() {
                // PHP convention: 0 means "no limit". apollo-parser wants a real number.
                out.recursion_limit = if n <= 0 { usize::MAX } else { n as usize };
            }
        }

        Ok(out)
    }

    /// `recursion_limit` clamped for apollo-parser. Apollo's default is 4096;
    /// we always pass our value (which defaults to 256 to match PHP).
    pub fn apollo_recursion_limit(&self) -> usize {
        self.recursion_limit
    }
}
