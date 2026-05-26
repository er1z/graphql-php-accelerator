--TEST--
graphql_accelerator: extension loads and reports the expected name/version
--EXTENSIONS--
graphql_accelerator
--FILE--
<?php
var_dump(extension_loaded('graphql_accelerator'));
?>
--EXPECT--
bool(true)
