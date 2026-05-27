# graphql_accelerator

PHP extension to accellerate GraphQL parser of `webonyx/graphql-php`. Written in Rust
on top of [`apollo-parser`](https://crates.io/crates/apollo-parser) and
[`ext-php-rs`](https://crates.io/crates/ext-php-rs).

## Why

PHP is not the most efficient technology on parsing ASTs at a good performance. During
the work on the application that heavily uses the GraphQL server. Unfortunately, our current
workflow makes usage of stored queries a challenge, so came across the idea of leveraging
query parsing to more performant technology

## How it works

As the original GraphQL PHP parser call is hardcoded, I decided not to fork the library
and add something to the parser. Instead, this extension injects an extension-based
parser to the PHP classes runtime to skip autoloading of the original ones. So, the only
task to be done is to add the module to your PHP installation. All the batteries
are already included.

This approach has some limitations, but based on my benchmarks, it's the most performant.
Initially, I started with a fork of PHP libgraphqlparser wrapper, but the original C library
is unmaintained, so not all features were available.

Using apollo-parser provides predictive stability, and – what surprised me – offered way better
performance.

## Disclaimer

This extension is not production-ready, and you are responsible for any damage
caused by using it. It was built as a challenge, yet I will try to use it by myself
where applicable. Also, the codebase was built by AI-vibing but with making sure
all the good software craftsmanship principles are still followed, so:

- all known memory leaks are covered
- development was focused on verifying if new logic keeps original unit tests still to be successful
- this project was published once it started to bring at least noticeable benefits

## Outcomes

`benchmark-native.php` reports a **geomean 11–16× speedup** across 8 query
sizes (range 7.4×–20.9×). Full per-section results — latency, location cost, memory, throughput,
synthetic stress, partial parsers — are in
[`benchmarks/RESULTS.md`](../benchmarks/RESULTS.md).

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

## Testing

### Smoke-load

```bash
php -d extension=$(pwd)/target/release/libgraphql_accelerator.so \
    -r 'var_dump(extension_loaded("graphql_accelerator"));'
# bool(true)
```

### Run the extension-local phpt suite

```bash
TEST_PHP_EXECUTABLE=$(which php) \
    php $(php -r 'echo PHP_BINDIR;')/../lib/php/build/run-tests.php \
    -d "extension=$(pwd)/target/release/libgraphql_accelerator.so" \
    -q tests/phpt/
# Tests passed : 7 (100.0%)
```

### Run the project's PHPUnit Language suite

```bash
cd graphql-php
php -d extension=$(pwd)/../ext/target/release/libgraphql_accelerator.so \
    vendor/bin/phpunit tests/Language/ --no-progress
# Tests: 227, Assertions: 14299, Skipped: 2 (100% effective pass)
```

### Benchmark

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
