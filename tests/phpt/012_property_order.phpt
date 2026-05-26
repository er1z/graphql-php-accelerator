--TEST--
graphql_accelerator: get_object_vars order matches the on-disk PHP class
--EXTENSIONS--
graphql_accelerator
--FILE--
<?php
// FieldNode constructed via empty-array __construct (inherited from Node).
// Property declaration order matters: visitors and Node::recursiveToArray
// walk get_object_vars() in declaration order.
$f = new GraphQL\Language\AST\FieldNode([]);
print_r(array_keys(get_object_vars($f)));

$d = new GraphQL\Language\AST\DocumentNode([]);
print_r(array_keys(get_object_vars($d)));

$op = new GraphQL\Language\AST\OperationDefinitionNode([]);
print_r(array_keys(get_object_vars($op)));
?>
--EXPECT--
Array
(
    [0] => loc
    [1] => kind
    [2] => name
    [3] => alias
    [4] => arguments
    [5] => directives
    [6] => selectionSet
)
Array
(
    [0] => loc
    [1] => kind
    [2] => definitions
)
Array
(
    [0] => loc
    [1] => kind
    [2] => name
    [3] => operation
    [4] => variableDefinitions
    [5] => directives
    [6] => selectionSet
)
