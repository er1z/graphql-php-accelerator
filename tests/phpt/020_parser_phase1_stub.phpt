--TEST--
graphql_accelerator: Parser::parse round-trips a simple query
--EXTENSIONS--
graphql_accelerator
--FILE--
<?php
$doc = GraphQL\Language\Parser::parse('{ hero }');
echo get_class($doc), "\n";
echo $doc->kind, "\n";
echo count($doc->definitions), "\n";
$op = $doc->definitions[0];
echo $op->operation, "\n";
$f = $op->selectionSet->selections[0];
echo $f->name->value, "\n";
?>
--EXPECT--
GraphQL\Language\AST\DocumentNode
Document
1
query
hero
