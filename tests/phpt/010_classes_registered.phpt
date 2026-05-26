--TEST--
graphql_accelerator: every promised class/interface exists with autoload=false
--EXTENSIONS--
graphql_accelerator
--FILE--
<?php
$expected = [
    // Support classes
    'GraphQL\Language\SourceLocation',
    'GraphQL\Language\Source',
    'GraphQL\Language\DirectiveLocation',
    'GraphQL\Language\AST\Location',
    'GraphQL\Language\AST\Node',
    'GraphQL\Language\Parser',
    // A representative slice of node classes
    'GraphQL\Language\AST\NameNode',
    'GraphQL\Language\AST\DocumentNode',
    'GraphQL\Language\AST\FieldNode',
    'GraphQL\Language\AST\OperationDefinitionNode',
    'GraphQL\Language\AST\StringValueNode',
    'GraphQL\Language\AST\ObjectTypeDefinitionNode',
    'GraphQL\Language\AST\ObjectTypeExtensionNode',
    'GraphQL\Language\AST\DirectiveDefinitionNode',
];
$expectedInterfaces = [
    'GraphQL\Language\AST\DefinitionNode',
    'GraphQL\Language\AST\ExecutableDefinitionNode',
    'GraphQL\Language\AST\SelectionNode',
    'GraphQL\Language\AST\TypeNode',
    'GraphQL\Language\AST\ValueNode',
    'GraphQL\Language\AST\HasSelectionSet',
    'GraphQL\Language\AST\TypeSystemDefinitionNode',
    'GraphQL\Language\AST\TypeSystemExtensionNode',
    'GraphQL\Language\AST\TypeDefinitionNode',
    'GraphQL\Language\AST\TypeExtensionNode',
];
foreach ($expected as $c) {
    if (!class_exists($c, false)) {
        echo "MISSING CLASS: $c\n";
    }
}
foreach ($expectedInterfaces as $i) {
    if (!interface_exists($i, false)) {
        echo "MISSING IFACE: $i\n";
    }
}
echo "done\n";
?>
--EXPECT--
done
