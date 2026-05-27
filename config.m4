dnl config.m4 for graphql_accelerator

PHP_ARG_ENABLE([graphql_accelerator],
  [whether to enable graphql_accelerator support],
  [AS_HELP_STRING([--enable-graphql_accelerator],
    [Enable graphql_accelerator support])],
  [no])

if test "$PHP_GRAPHQL_ACCELERATOR" != "no"; then
  AC_PATH_PROG([CARGO], [cargo], [no])
  if test "$CARGO" = "no"; then
    AC_MSG_ERROR([cargo not found; install the Rust toolchain from https://rustup.rs])
  fi

  PHP_SUBST([CARGO])
  PHP_NEW_EXTENSION(graphql_accelerator, stub.c, $ext_shared)
  PHP_ADD_MAKEFILE_FRAGMENT
fi
