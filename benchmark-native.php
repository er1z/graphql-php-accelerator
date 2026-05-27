<?php declare(strict_types=1);

/**
 * GraphQL parser benchmark — PHP baseline vs. C extension.
 *
 * Usage:
 *   # Run both and print a comparison table (default):
 *   php benchmark-native.php [path/to/graphql_accelerator.so]
 *
 *   # Single-engine mode (called internally by the comparison runner):
 *   php                                      benchmark-native.php --single
 *   php -d extension=graphql_accelerator.so       benchmark-native.php --single
 */

require __DIR__ . '/vendor/autoload.php';

use GraphQL\Language\Parser;

// ── Queries ──────────────────────────────────────────────────────────────────

function build_queries(): array
{
    $qs = [];

    $qs['tiny (28 B)'] = '{ hero { name } }';

    $qs['simple (90 B)'] =
        'query HeroName($episode: Episode) { hero(episode: $episode) { name friends { name } } }';

    $qs['product page (320 B)'] = <<<'GQL'
query ProductPage($id: ID!, $currency: Currency = USD) {
  product(id: $id) {
    id name description
    price(currency: $currency) { amount currency }
    images(first: 5) { url width height alt }
    variants { id sku attributes { name value } }
    reviews(first: 3) { rating body author { name } }
  }
}
GQL;

    $qs['feed + fragments (540 B)'] = <<<'GQL'
query Feed($after: String, $limit: Int = 20) {
  feed(after: $after, limit: $limit) {
    pageInfo { hasNextPage endCursor }
    edges {
      node {
        ...PostFields
        author { ...UserFields }
        comments(first: 2) { ...CommentFields }
      }
    }
  }
}
fragment PostFields on Post { id title body createdAt likes }
fragment UserFields on User { id name avatarUrl }
fragment CommentFields on Comment { id body author { ...UserFields } }
GQL;

    $qs['introspection (580 B)'] = <<<'GQL'
query IntrospectionQuery {
  __schema {
    queryType { name }
    mutationType { name }
    subscriptionType { name }
    types {
      kind name description
      fields(includeDeprecated: true) {
        name description isDeprecated deprecationReason
        type { kind name ofType { kind name ofType { kind name } } }
      }
      inputFields { name description type { kind name } }
      interfaces { name }
      enumValues(includeDeprecated: true) { name description isDeprecated }
      possibleTypes { name }
    }
    directives {
      name description locations
      args { name description type { kind name } defaultValue }
    }
  }
}
GQL;

    $qs['reporting (900 B)'] = <<<'GQL'
query DashboardReport($from: Date!, $to: Date!, $groupBy: [Dimension!]!, $filters: FilterInput) {
  report(from: $from, to: $to, groupBy: $groupBy, filters: $filters) {
    metadata { generatedAt totalRows }
    dimensions { key label type }
    metrics { key label unit description }
    rows {
      dimensions { key value label }
      metrics { key value formatted trend { direction pct } }
    }
    totals { key value formatted }
    comparison(period: PREVIOUS_PERIOD) {
      rows { dimensions { key value } metrics { key value delta pct } }
    }
  }
}
GQL;

    $qs['kitchen-sink (1.1 kB)'] =
        (string) file_get_contents(__DIR__ . '/tests/Language/kitchen-sink.graphql');

    // Schema SDL (uses block-string descriptions — not supported by extension)
    $qs['SDL (2.4 kB)'] =
        (string) file_get_contents(__DIR__ . '/tests/Language/schema-kitchen-sink.graphql');

    return $qs;
}

// ── Single-engine mode ────────────────────────────────────────────────────────

function run_single(): never
{
    $warmup = 100;
    $iters  = 1000;

    $queries = build_queries();

    $results = [];
    foreach ($queries as $label => $src) {
        // Warmup
        for ($i = 0; $i < $warmup; $i++) {
            try {
                Parser::parse($src, ['noLocation' => true]);
            } catch (Throwable) {
                break; // skip rest of warmup if unsupported
            }
        }

        $times = [];
        $error = null;
        for ($i = 0; $i < $iters; $i++) {
            $t0 = hrtime(true);
            try {
                Parser::parse($src, ['noLocation' => true]);
                $times[] = (hrtime(true) - $t0) / 1_000; // µs
            } catch (Throwable $e) {
                $error = $e->getMessage();
                break;
            }
        }

        if ($error !== null || $times === []) {
            $results[$label] = ['error' => $error ?? 'no data'];
        } else {
            sort($times);
            $n = count($times);
            $results[$label] = [
                'min'    => $times[0],
                'p50'    => $times[(int) ($n * 0.50)],
                'p95'    => $times[(int) ($n * 0.95)],
                'mean'   => array_sum($times) / $n,
                'max'    => $times[$n - 1],
                'cps'    => 1_000_000 / (array_sum($times) / $n),
            ];
        }
    }

    $engine = extension_loaded('graphql_accelerator') ? 'extension' : 'php';
    echo json_encode(['engine' => $engine, 'results' => $results], JSON_PRETTY_PRINT) . "\n";
    exit(0);
}

// ── Comparison mode ───────────────────────────────────────────────────────────

function run_comparison(string $soPath): void
{
    $self = __FILE__;
    $php  = PHP_BINARY;

    echo "Running PHP baseline …\n";
    $phpOut = shell_exec("$php " . escapeshellarg($self) . " --single 2>/dev/null");

    echo "Running C extension …\n";
    $extOut = shell_exec("$php -d extension=" . escapeshellarg($soPath) . " " . escapeshellarg($self) . " --single 2>/dev/null");

    $phpData = $phpOut ? json_decode($phpOut, true) : null;
    $extData = $extOut ? json_decode($extOut, true) : null;

    if (!$phpData) {
        echo "ERROR: could not collect PHP baseline results\n";
        exit(1);
    }
    if (!$extData) {
        echo "ERROR: could not collect extension results — check path: $soPath\n";
        exit(1);
    }

    $phpVersion = PHP_VERSION;
    echo "\n";
    echo "╔══════════════════════════════════════════════════════════════════════════════════════════╗\n";
    echo "║  GraphQL Parser Benchmark — PHP $phpVersion                                        ║\n";
    echo "║  Iterations: 1000  |  Warmup: 100  |  noLocation: true                               ║\n";
    echo "╚══════════════════════════════════════════════════════════════════════════════════════════╝\n\n";

    printf("%-28s │ %10s %10s %10s %10s │ %10s %10s %10s %10s │ %8s\n",
        '',
        'PHP p50', 'PHP p95', 'PHP mean', 'PHP calls/s',
        'Ext p50', 'Ext p95', 'Ext mean', 'Ext calls/s',
        'Speedup');
    printf("%-28s │ %10s %10s %10s %10s │ %10s %10s %10s %10s │ %8s\n",
        'Query',
        'µs', 'µs', 'µs', '',
        'µs', 'µs', 'µs', '',
        '×');
    printf("%s\n", str_repeat('─', 125));

    $queries = build_queries();
    foreach (array_keys($queries) as $label) {
        $pr = $phpData['results'][$label] ?? null;
        $er = $extData['results'][$label] ?? null;

        if (isset($pr['error'])) {
            $phpCols = sprintf("%10s %10s %10s %10s", 'n/a', 'n/a', 'n/a', 'n/a');
        } else {
            $phpCols = sprintf("%10.1f %10.1f %10.1f %10s",
                $pr['p50'], $pr['p95'], $pr['mean'],
                number_format((int) $pr['cps']));
        }

        if (isset($er['error'])) {
            $extCols = sprintf("%10s %10s %10s %10s", 'n/a', 'n/a', 'n/a', 'n/a');
            $speedup = '   n/a';
            $note    = '  ← unsupported';
        } else {
            $extCols = sprintf("%10.1f %10.1f %10.1f %10s",
                $er['p50'], $er['p95'], $er['mean'],
                number_format((int) $er['cps']));
            $ratio   = isset($pr['mean']) ? $pr['mean'] / $er['mean'] : 0;
            $speedup = sprintf("%7.1f×", $ratio);
            $note    = '';
        }

        printf("%-28s │ %s │ %s │ %s%s\n",
            $label, $phpCols, $extCols, $speedup, $note);
    }

    // Summary row
    $phpTotMean = 0; $extTotMean = 0; $n = 0;
    $queries_data = build_queries();
    foreach (array_keys($queries_data) as $label) {
        $pr = $phpData['results'][$label] ?? null;
        $er = $extData['results'][$label] ?? null;
        if (!isset($pr['error']) && !isset($er['error'])) {
            $phpTotMean += $pr['mean'];
            $extTotMean += $er['mean'];
            $n++;
        }
    }
    printf("%s\n", str_repeat('─', 125));
    if ($n > 0) {
        $avgSpeedup   = $phpTotMean / $extTotMean;
        $phpAvgMean   = $phpTotMean / $n;
        $extAvgMean   = $extTotMean / $n;
        printf("%-28s │ %10s %10s %10.1f %10s │ %10s %10s %10.1f %10s │ %7.1f×\n",
            "AVERAGE (supported queries)",
            '', '', $phpAvgMean, '',
            '', '', $extAvgMean, '',
            $avgSpeedup);
    }

    echo "\n";
    echo "Notes:\n";
    echo "  n/a = query uses syntax not supported by libgraphqlparser (description strings in SDL,\n";
    echo "        directives on variable definitions, experimentalFragmentVariables).\n";
    echo "  p50 / p95 = 50th / 95th percentile latency.\n";
}

// ── Entry point ───────────────────────────────────────────────────────────────

if (in_array('--single', $argv, true)) {
    run_single();
}

// Find the .so path
$soPath = null;
foreach ($argv as $arg) {
    if (str_ends_with($arg, '.so') || str_ends_with($arg, '.dylib')) {
        $soPath = $arg;
        break;
    }
}

if ($soPath === null) {
    // Try to auto-detect
    $candidate = dirname(__DIR__) . '/graphql-parser-php/modules/graphql_accelerator.so';
    if (file_exists($candidate)) {
        $soPath = $candidate;
    }
}

if ($soPath === null || !file_exists($soPath)) {
    echo "Usage: php benchmark-native.php [path/to/graphql_accelerator.so]\n";
    echo "       (auto-detect looks for ../graphql-parser-php/modules/graphql_accelerator.so)\n";
    exit(1);
}

run_comparison($soPath);
