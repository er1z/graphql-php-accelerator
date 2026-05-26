--TEST--
graphql_accelerator: instanceof graph matches PHP (Node, marker interfaces, JsonSerializable)
--EXTENSIONS--
graphql_accelerator
--FILE--
<?php
$specs = [
    // class => [interfaces it must implement (besides JsonSerializable)]
    'GraphQL\Language\AST\FieldNode'              => ['GraphQL\Language\AST\SelectionNode'],
    'GraphQL\Language\AST\InlineFragmentNode'     => ['GraphQL\Language\AST\SelectionNode'],
    'GraphQL\Language\AST\FragmentSpreadNode'     => ['GraphQL\Language\AST\SelectionNode'],
    'GraphQL\Language\AST\OperationDefinitionNode' => ['GraphQL\Language\AST\ExecutableDefinitionNode', 'GraphQL\Language\AST\HasSelectionSet'],
    'GraphQL\Language\AST\FragmentDefinitionNode' => ['GraphQL\Language\AST\ExecutableDefinitionNode', 'GraphQL\Language\AST\HasSelectionSet'],
    'GraphQL\Language\AST\IntValueNode'           => ['GraphQL\Language\AST\ValueNode'],
    'GraphQL\Language\AST\StringValueNode'        => ['GraphQL\Language\AST\ValueNode'],
    'GraphQL\Language\AST\NamedTypeNode'          => ['GraphQL\Language\AST\TypeNode'],
    'GraphQL\Language\AST\ListTypeNode'           => ['GraphQL\Language\AST\TypeNode'],
    'GraphQL\Language\AST\NonNullTypeNode'        => ['GraphQL\Language\AST\TypeNode'],
    'GraphQL\Language\AST\ObjectTypeDefinitionNode'    => ['GraphQL\Language\AST\TypeDefinitionNode'],
    'GraphQL\Language\AST\InterfaceTypeDefinitionNode' => ['GraphQL\Language\AST\TypeDefinitionNode'],
    'GraphQL\Language\AST\ObjectTypeExtensionNode'     => ['GraphQL\Language\AST\TypeExtensionNode'],
];
foreach ($specs as $cls => $expected) {
    $parents = class_parents($cls);
    if (!isset($parents['GraphQL\Language\AST\Node'])) {
        echo "WRONG PARENT for $cls\n";
        continue;
    }
    $impl = class_implements($cls);
    if (!isset($impl['JsonSerializable'])) {
        echo "MISSING JsonSerializable on $cls\n";
    }
    foreach ($expected as $iface) {
        if (!isset($impl[$iface])) {
            echo "MISSING $iface on $cls\n";
        }
    }
}
echo "done\n";
?>
--EXPECT--
done
