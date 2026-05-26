--TEST--
graphql_accelerator: DirectiveLocation / Parser constants
--EXTENSIONS--
graphql_accelerator
--FILE--
<?php
echo GraphQL\Language\DirectiveLocation::QUERY, "\n";
echo GraphQL\Language\DirectiveLocation::IFACE, "\n";
echo GraphQL\Language\Parser::DEFAULT_RECURSION_LIMIT, "\n";
?>
--EXPECT--
QUERY
INTERFACE
256
