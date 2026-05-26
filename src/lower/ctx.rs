//! Per-parse lowering context.

use ext_php_rs::types::{ZendObject, Zval};

use crate::classes::slots;
use crate::options::ParserOptions;

pub struct LowerCtx {
    pub body: String,
    /// Zval holding the `GraphQL\Language\Source` object — shared across every
    /// `Location` produced in this parse.
    pub source_zv: Zval,
    pub options: ParserOptions,
    /// Cumulative byte length of each line (line_starts[i] = byte offset where
    /// line `i+1` begins). Used to translate apollo-parser's byte offsets into
    /// PHP `Source::getLocation()`-compatible (line, col) pairs *if* a future
    /// phase requires it. Phase 2 only needs offset → Location bridging.
    pub line_starts: Vec<usize>,
    /// Synthetic-prefix length: the number of source bytes we wrapped around
    /// the user-supplied text when implementing partial parsers via
    /// `parseValue`/`parseType` (see PRD §6.2). Subtracted from every emitted
    /// offset so that `Location::$start`/`$end` reference the user's
    /// coordinates, not our wrapper.
    pub prefix_offset: u32,
}

impl LowerCtx {
    pub fn new(body: String, source_zv: Zval, options: ParserOptions) -> Self {
        let line_starts = build_line_starts(&body);
        Self {
            body,
            source_zv,
            options,
            line_starts,
            prefix_offset: 0,
        }
    }

    pub fn with_prefix_offset(mut self, prefix: u32) -> Self {
        self.prefix_offset = prefix;
        self
    }

    /// Emit a `Location` Zval (object) for the given text range, honouring
    /// the `noLocation` option (returns a null Zval in that case).
    pub fn location_zval(&self, range: apollo_parser::TextRange) -> Zval {
        if self.options.no_location {
            return Zval::new();
        }
        let start = u32::from(range.start()).saturating_sub(self.prefix_offset);
        let end = u32::from(range.end()).saturating_sub(self.prefix_offset);

        let ce = slots::location::get();
        let mut obj = ZendObject::new(ce);
        let _ = obj.set_property("start", start as i64);
        let _ = obj.set_property("end", end as i64);
        let _ = obj.set_property("source", self.source_zv.shallow_clone());
        // startToken / endToken intentionally left null — see PRD §3.2.
        let mut zv = Zval::new();
        zv = crate::classes::obj_to_zval(obj);
        zv
    }
}

fn build_line_starts(body: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\r' => {
                let next = i + 1;
                if next < bytes.len() && bytes[next] == b'\n' {
                    starts.push(next + 1);
                    i = next + 1;
                } else {
                    starts.push(next);
                    i = next;
                }
            }
            b'\n' => {
                starts.push(i + 1);
                i += 1;
            }
            _ => i += 1,
        }
    }
    starts
}
