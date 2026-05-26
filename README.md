# ext-graphql_accelerator

Native GraphQL parser accelerator for `webonyx/graphql-php`, written in Rust
on top of [`apollo-parser`](https://crates.io/crates/apollo-parser) and
[`ext-php-rs`](https://crates.io/crates/ext-php-rs).

See `../PRD2.md` for the full design document.

## Status

**Phase 1–6 (parser path) — complete.** `tests/Language/` passes
**225/225 (100%)** with the extension loaded (the 2 remaining tests are
PHPUnit `@skipped` fixtures inherited from PRD.md — deep-recursion stress
tests).

`benchmark-native.php` reports a **geomean 11–16× speedup** across 8 query
sizes (range 7.4×–20.9×). Zero `n/a` queries — every GraphQL document the
benchmark exercises is supported natively (the previous libgraphqlparser-era
extension had several `n/a` rows for SDL constructs).

Full per-section results — latency, location cost, memory, throughput,
synthetic stress, partial parsers — are in
[`../benchmarks/RESULTS.md`](../benchmarks/RESULTS.md).

## What's done

- 53 native PHP class entries: `Source`, `SourceLocation`, `Location`,
  `DirectiveLocation`, `Node`, 43 concrete AST nodes, 10 marker interfaces,
  and `Parser`. (`Token`, `Lexer`, `NodeKind`, and `NodeList` autoload from
  the on-disk PHP files — they hit non-hot code paths.)
- `<SOF>` → … → `<EOF>` `Token` chain materialised on the document's
  `Location` (comments included, whitespace and commas skipped), built
  from a single re-lex pass over the source via apollo's `Lexer`.
- `Parser::parse`, `Parser::parseValue`, `Parser::parseType`, and 17
  `__callStatic` partial parsers (`name`, `argumentsDefinition`,
  `directiveLocations`, `implementsInterfaces`, `unionMemberTypes`, …).
- Native handling for `noLocation`, `allowLegacySDLEmptyFields`,
  `allowLegacySDLImplementsInterfaces`, `experimentalFragmentVariables`,
  and `recursionLimit` options.
- Block-string parsing with the `\"""` → `"""` spec escape.
- `Node::__construct`/`__toString`/`toArray`/`jsonSerialize`/`cloneDeep`/
  `getName`, `Location::create`/`toArray`, `Source::getLocation`,
  `SourceLocation::jsonSerialize` all implemented in Rust.
- `SyntaxError` thrown via `PhpException::new` + scoped
  `zend_update_property` for the inherited `Error` typed properties.
- 7 phpt smoke tests in `tests/phpt/` (extension loads, all expected
  classes/interfaces registered, constants correct, property order
  matches the PHP source, `Parser::parse` parses end-to-end, instanceof
  graph parity, no AST autoload triggered).

## Build

```bash
cd ext
cargo build --release
# → target/release/libgraphql_accelerator.so
```

Requires:

- PHP 8.1+ with header files (`php-config --includes`)
- Rust 1.95+
- `libclang-dev` (pulled in by `ext-php-rs-bindgen`)

## Smoke-load

```bash
php -d extension=$(pwd)/target/release/libgraphql_accelerator.so \
    -r 'var_dump(extension_loaded("graphql_accelerator"));'
# bool(true)
```

## Run the extension-local phpt suite

```bash
TEST_PHP_EXECUTABLE=$(which php) \
    php $(php -r 'echo PHP_BINDIR;')/../lib/php/build/run-tests.php \
    -d "extension=$(pwd)/target/release/libgraphql_accelerator.so" \
    -q tests/phpt/
# Tests passed : 7 (100.0%)
```

## Run the project's PHPUnit Language suite

```bash
cd ..
php -d extension=$(pwd)/ext/target/release/libgraphql_accelerator.so \
    vendor/bin/phpunit tests/Language/ --no-progress
# Tests: 227, Assertions: 14299, Skipped: 2 (100% effective pass)
```

## Benchmark

```bash
cd ..
php benchmark-native.php $(pwd)/ext/target/release/libgraphql_accelerator.so
```

## Layout

```
ext/
├── Cargo.toml            crate name = "graphql_accelerator"
├── Cargo.lock
├── build.rs              empty — placeholder for cargo-php hook-up
├── rust-toolchain.toml   pins MSRV to 1.95
├── src/
│   ├── lib.rs            #[php_module] entry, MINIT wiring
│   ├── options.rs        ParserOptions
│   ├── source.rs         Source coercion / construction
│   ├── tokens.rs         <SOF>…<EOF> Token-chain builder
│   ├── errors.rs         apollo-parser error → graphql-php SyntaxError mapper
│   ├── classes/
│   │   ├── mod.rs        ClassProperty helpers, JsonSerializable shim
│   │   ├── slots.rs      per-class static slots & ClassEntryInfo getters
│   │   ├── support.rs    SourceLocation, Source, DirectiveLocation, Location
│   │   ├── interfaces.rs 10 marker interfaces
│   │   ├── nodes.rs      Node + 43 concrete AST node classes
│   │   └── parser.rs     Parser + __callStatic dispatch
│   └── lower/
│       ├── mod.rs        document/value/type entry points
│       ├── ctx.rs        LowerCtx (line table, source zv, options)
│       ├── helpers.rs    ZBoxObj builder, NodeList construction
│       ├── values.rs     value-literal lowering + block-string decode
│       ├── types.rs      type-reference lowering
│       ├── selections.rs operation/fragment/field lowering
│       └── sdl.rs        SDL definition + extension lowering
└── tests/
    └── phpt/             extension-local smoke tests
```
