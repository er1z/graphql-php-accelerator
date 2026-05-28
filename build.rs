fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" {
        // PHP symbols (zend_malloc, emalloc, zend_ce_exception, etc.) are not
        // available as a link-time library — they're resolved at runtime when PHP
        // dlopen()s the extension. macOS's linker rejects undefined symbols by
        // default, so we must opt into dynamic_lookup for this to work.
        println!("cargo:rustc-link-arg=-undefined");
        println!("cargo:rustc-link-arg=dynamic_lookup");
    }
}
