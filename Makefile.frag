# Add cargo-override as an extra prerequisite of 'all' (no recipe redefinition).
# It depends on graphql_accelerator.la so phpize links stub.c first, then we
# overwrite .libs/graphql_accelerator.so with the real cargo output.
all: cargo-override

cargo-override: graphql_accelerator.la
	$(CARGO) build --release

install: cargo-override
	$(INSTALL) -d $(EXTENSION_DIR)
	$(INSTALL) -m 0755 target/release/libgraphql_accelerator.so $(EXTENSION_DIR)/graphql_accelerator.so

.PHONY: cargo-override install
