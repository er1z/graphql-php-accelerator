--TEST--
graphql_accelerator: Composer's autoloader is not asked for AST node classes
--EXTENSIONS--
graphql_accelerator
--FILE--
<?php
// Limit the check to the *AST* namespace plus `Parser` — `Token`, `Lexer`,
// `NodeList`, and `NodeKind` autoload from the on-disk PHP files (see
// PRD2 §9 Phase 1 status notes).
$asked = [];
spl_autoload_register(function ($name) use (&$asked): void {
    $asked[] = $name;
});

$kinds = [
    GraphQL\Language\DirectiveLocation::QUERY,
    GraphQL\Language\Parser::DEFAULT_RECURSION_LIMIT,
];

$objects = [
    new GraphQL\Language\AST\NameNode([]),
    new GraphQL\Language\AST\DocumentNode([]),
    new GraphQL\Language\AST\FieldNode([]),
    new GraphQL\Language\AST\IntValueNode([]),
    new GraphQL\Language\AST\NamedTypeNode([]),
    new GraphQL\Language\AST\ObjectTypeDefinitionNode([]),
    new GraphQL\Language\AST\DirectiveDefinitionNode([]),
];

foreach (['GraphQL\Language\AST\Node', 'GraphQL\Language\AST\Location'] as $c) {
    new ReflectionClass($c);
}

$allowed = [
    'GraphQL\\Language\\AST\\NodeList' => true,
    'GraphQL\\Language\\AST\\NodeKind' => true,
];
$badHits = array_values(array_filter(
    $asked,
    static fn ($n) => str_starts_with($n, 'GraphQL\\Language\\AST\\') && !isset($allowed[$n]),
));
if ($badHits !== []) {
    echo "FAIL: autoloader asked for:\n";
    foreach ($badHits as $h) echo "  $h\n";
    exit(1);
}
echo "ok\n";
?>
--EXPECT--
ok
