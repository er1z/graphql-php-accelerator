# Add cargo-override as an extra prerequisite of 'all' (no recipe redefinition).
# It depends on graphql_accelerator.la so phpize links stub.c first, then we
# overwrite .libs/graphql_accelerator.so with the real cargo output.
all: cargo-override.stamp

cargo-override.stamp: graphql_accelerator.la
	$(CARGO) build --release
	@if [ -f target/release/libgraphql_accelerator.dylib ]; then \
		cp target/release/libgraphql_accelerator.dylib target/release/libgraphql_accelerator.so; \
	fi
	$(INSTALL) -m 0755 target/release/libgraphql_accelerator.so .libs/graphql_accelerator.so
	@touch cargo-override.stamp

install: cargo-override.stamp
	$(INSTALL) -d $(EXTENSION_DIR)
	$(INSTALL) -m 0755 target/release/libgraphql_accelerator.so $(EXTENSION_DIR)/graphql_accelerator.so

.PHONY: install
