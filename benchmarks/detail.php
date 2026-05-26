<?php declare(strict_types=1);

/**
 * Detailed PHP vs. Rust extension benchmark for webonyx/graphql-php's parser.
 *
 * Compares the on-disk pure-PHP Parser against the apollo-parser-backed
 * `graphql_accelerator` extension across six dimensions:
 *
 *   A. Headline latency  — 8 representative queries, full distribution stats.
 *   B. Location cost     — same queries, with `noLocation: false` so each
 *                          parse allocates a Location for every node.
 *   C. Memory per parse  — peak-memory delta around a single parse.
 *   D. Throughput        — calls/sec at high iteration count.
 *   E. Synthetic stress  — deeply-nested types, wide selection sets,
 *                          long fragment chains.
 *   F. Partial parsers   — Parser::parseValue, Parser::parseType,
 *                          Parser::name, Parser::argumentsDefinition.
 *
 * Usage:
 *   php ext/benchmarks/detail.php [path/to/libgraphql_accelerator.so]
 *
 *   # Single-engine mode (called internally by the comparison runner):
 *   php                                          ext/benchmarks/detail.php --single
 *   php -d extension=path/to/graphql_accelerator.so   ext/benchmarks/detail.php --single
 */

require __DIR__ . '/../graphql-php/vendor/autoload.php';

use GraphQL\Language\Parser;

// ── Fixture corpus ───────────────────────────────────────────────────────────

function build_queries(): array
{
    $fixtureDir = __DIR__ . '/graphql-php/tests/Language';

    $qs = [];
    $qs['tiny (28 B)']            = '{ hero { name } }';
    $qs['simple (90 B)']          = 'query HeroName($episode: Episode) { hero(episode: $episode) { name friends { name } } }';
    $qs['product page (320 B)']     = build_synthetic('product');
    $qs['feed + fragments (540 B)'] = build_synthetic('feed');
    $qs['introspection (580 B)']  = build_synthetic('introspection');
    $qs['reporting (900 B)']      = build_synthetic('reporting');
    $qs['kitchen-sink (1.1 kB)']  = (string) file_get_contents($fixtureDir . '/kitchen-sink.graphql');
    $qs['SDL (2.4 kB)']           = (string) file_get_contents($fixtureDir . '/schema-kitchen-sink.graphql');

    return $qs;
}

function build_synthetic(string $kind): string
{
    return match ($kind) {
        'product' => <<<'GQL'
query ProductPage($id: ID!, $currency: Currency = USD) {
  product(id: $id) {
    id name description
    price(currency: $currency) { amount currency }
    images(first: 5) { url width height alt }
    variants { id sku attributes { name value } }
    reviews(first: 3) { rating body author { name } }
  }
}
GQL,
        'feed' => <<<'GQL'
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
GQL,
        'introspection' => <<<'GQL'
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
GQL,
        'reporting' => <<<'GQL'
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
GQL,
        default => throw new InvalidArgumentException("unknown synthetic: $kind"),
    };
}

function build_synthetic_stress(): array
{
    $out = [];

    // Deeply-nested field chain
    $out['deep selection (32 levels)'] = '{ ' . str_repeat('a { ', 32) . 'x' . str_repeat(' }', 32) . ' }';

    // Wide flat selection set
    $fields = [];
    for ($i = 0; $i < 200; $i++) $fields[] = "f$i";
    $out['wide selections (200 fields)'] = '{ ' . implode(' ', $fields) . ' }';

    // Many list-nesting levels in a type reference (parseType-style)
    $depth = 24;
    $out['deeply-nested arg type'] =
        'query Q($v: ' . str_repeat('[', $depth) . 'Int' . str_repeat(']', $depth) . ') { f }';

    // Many fragments, used densely
    $fragments = [];
    for ($i = 0; $i < 20; $i++) $fragments[] = "fragment F$i on T { x$i y$i }";
    $spreads = [];
    for ($i = 0; $i < 20; $i++) $spreads[] = "...F$i";
    $out['20 fragments × 20 spreads']
        = '{ ' . implode(' ', $spreads) . ' } ' . implode("\n", $fragments);

    // Wide SDL (50 types)
    $types = ['type Query { a: A0 }'];
    for ($i = 0; $i < 50; $i++) $types[] = "type A$i { f$i: A" . (($i + 1) % 50) . " }";
    $out['SDL 50 mutually-recursive types'] = implode("\n", $types);

    // ObjectValue with many fields
    $kv = [];
    for ($i = 0; $i < 100; $i++) $kv[] = "k$i: $i";
    $out['ObjectValue with 100 fields']
        = '{ f(arg: { ' . implode(', ', $kv) . ' }) }';

    return $out;
}

// ── Stats ─────────────────────────────────────────────────────────────────────

function stats(array $times): array
{
    sort($times);
    $n = count($times);
    $mean = array_sum($times) / $n;
    $sumsq = 0.0;
    foreach ($times as $t) $sumsq += ($t - $mean) ** 2;
    $stdev = sqrt($sumsq / $n);
    return [
        'min'   => $times[0],
        'p50'   => $times[(int) ($n * 0.50)],
        'p95'   => $times[min($n - 1, (int) ($n * 0.95))],
        'p99'   => $times[min($n - 1, (int) ($n * 0.99))],
        'max'   => $times[$n - 1],
        'mean'  => $mean,
        'stdev' => $stdev,
        'cps'   => 1_000_000 / $mean,
        'n'     => $n,
    ];
}

function time_parse(string $src, array $options, int $iters): array
{
    $times = [];
    for ($i = 0; $i < $iters; $i++) {
        $t0 = hrtime(true);
        try {
            Parser::parse($src, $options);
            $times[] = (hrtime(true) - $t0) / 1_000;
        } catch (Throwable $e) {
            return ['error' => $e->getMessage()];
        }
    }
    return stats($times);
}

function time_call(callable $fn, int $iters): array
{
    $times = [];
    for ($i = 0; $i < $iters; $i++) {
        $t0 = hrtime(true);
        try {
            $fn();
            $times[] = (hrtime(true) - $t0) / 1_000;
        } catch (Throwable $e) {
            return ['error' => $e->getMessage()];
        }
    }
    return stats($times);
}

function measure_memory(string $src, array $options): array
{
    // Run a single parse and capture peak memory delta (in bytes).
    gc_collect_cycles();
    $beforePeak = memory_get_usage(true);
    $beforeReal = memory_get_usage(false);

    $samples = [];
    $count = 100;
    for ($i = 0; $i < $count; $i++) {
        $before = memory_get_usage(false);
        $doc = Parser::parse($src, $options);
        $after = memory_get_usage(false);
        $samples[] = $after - $before;
        unset($doc);
    }
    sort($samples);
    return [
        'p50' => $samples[(int) ($count * 0.50)],
        'p95' => $samples[(int) ($count * 0.95)],
        'mean' => array_sum($samples) / $count,
    ];
}

// ── Single-engine mode ───────────────────────────────────────────────────────

function run_single(): never
{
    $warmup = 100;
    $iters  = 1500;

    $queries = build_queries();
    $stress  = build_synthetic_stress();

    $out = [
        'engine' => extension_loaded('graphql_accelerator') ? 'extension' : 'php',
        'php_version' => PHP_VERSION,
        'memory_limit' => ini_get('memory_limit'),
        'warmup' => $warmup,
        'iters' => $iters,
        'corpus' => [],
        'corpus_with_loc' => [],
        'memory' => [],
        'throughput' => [],
        'stress' => [],
        'partials' => [],
    ];

    // Warm-up over all queries to amortize class-loading.
    foreach ($queries as $src) {
        for ($i = 0; $i < $warmup; $i++) {
            try { Parser::parse($src, ['noLocation' => true]); } catch (Throwable) { break; }
        }
    }

    // A) Headline latency (noLocation = true)
    foreach ($queries as $label => $src) {
        $out['corpus'][$label] = time_parse($src, ['noLocation' => true], $iters);
    }

    // B) noLocation = false (i.e. allocates Location for every node)
    foreach ($queries as $label => $src) {
        $out['corpus_with_loc'][$label] = time_parse($src, [], $iters);
    }

    // C) Memory per parse (noLocation = false for realistic numbers)
    foreach ($queries as $label => $src) {
        try {
            $out['memory'][$label] = measure_memory($src, []);
        } catch (Throwable $e) {
            $out['memory'][$label] = ['error' => $e->getMessage()];
        }
    }

    // D) Throughput (long burn-in of one mid-size query)
    $tput = [];
    foreach (['simple (90 B)', 'kitchen-sink (1.1 kB)', 'SDL (2.4 kB)'] as $label) {
        $src = $queries[$label];
        try {
            $t0 = hrtime(true);
            $n = 5000;
            for ($i = 0; $i < $n; $i++) Parser::parse($src, ['noLocation' => true]);
            $elapsed = (hrtime(true) - $t0) / 1_000_000_000; // s
            $tput[$label] = ['parses' => $n, 'elapsed_s' => $elapsed, 'cps' => $n / $elapsed];
        } catch (Throwable $e) {
            $tput[$label] = ['error' => $e->getMessage()];
        }
    }
    $out['throughput'] = $tput;

    // E) Synthetic stress
    foreach ($stress as $label => $src) {
        $out['stress'][$label] = time_parse($src, ['noLocation' => true], 600);
    }

    // F) Partial parsers
    $out['partials']['parseValue: short int']      = time_call(fn() => Parser::parseValue('42'), 2000);
    $out['partials']['parseValue: list of 50']     = time_call(
        fn() => Parser::parseValue('[' . implode(',', range(1, 50)) . ']'),
        2000
    );
    $out['partials']['parseValue: object 20 keys'] = time_call(
        fn() => Parser::parseValue('{ ' . implode(',', array_map(fn($i) => "k$i: $i", range(1, 20))) . ' }'),
        2000
    );
    $out['partials']['parseType: List<NonNull>']   = time_call(fn() => Parser::parseType('[Foo!]'), 2000);
    $out['partials']['Parser::name']               = time_call(fn() => Parser::name('Foo'), 2000);
    $out['partials']['Parser::argumentsDefinition'] = time_call(
        fn() => Parser::argumentsDefinition('(a: Int, b: String!, c: [Float])'),
        2000
    );

    echo json_encode($out, JSON_PRETTY_PRINT) . "\n";
    exit(0);
}

// ── Comparison mode ──────────────────────────────────────────────────────────

function run_comparison(string $soPath): void
{
    $self = __FILE__;
    $php  = PHP_BINARY;

    echo "[1/2] Running PHP baseline …\n";
    $phpOut = shell_exec("$php " . escapeshellarg($self) . " --single 2>/dev/null");

    echo "[2/2] Running native extension …\n";
    $extOut = shell_exec("$php -d extension=" . escapeshellarg($soPath) . " " . escapeshellarg($self) . " --single 2>/dev/null");

    $php = $phpOut ? json_decode($phpOut, true) : null;
    $ext = $extOut ? json_decode($extOut, true) : null;

    if (!$php) { fwrite(STDERR, "ERROR: PHP baseline produced no JSON\n"); exit(1); }
    if (!$ext) { fwrite(STDERR, "ERROR: Extension produced no JSON — check path: $soPath\n"); exit(1); }

    print_header($php['php_version']);
    print_section_a($php, $ext);
    print_section_b($php, $ext);
    print_section_c($php, $ext);
    print_section_d($php, $ext);
    print_section_e($php, $ext);
    print_section_f($php, $ext);
    print_footer($php['warmup'], $php['iters'], $soPath);
}

// ── Render helpers ───────────────────────────────────────────────────────────

function print_header(string $phpVer): void
{
    $bar = str_repeat('═', 90);
    echo "\n╔{$bar}╗\n";
    echo "║ GraphQL Parser — detailed PHP vs. native extension benchmark                             ║\n";
    echo "║ PHP {$phpVer} · noLocation toggled per section · warmup 100, hot-loop 1500 unless noted    ║\n";
    echo "╚{$bar}╝\n";
}

function print_section_a(array $php, array $ext): void
{
    echo "\n── A. Headline latency (noLocation: true) ─────────────────────────────────────────────────\n";
    printf("%-26s │ %12s %12s │ %12s %12s │ %8s\n", '', 'PHP p50', 'PHP p99', 'Ext p50', 'Ext p99', 'Speedup');
    printf("%-26s │ %12s %12s │ %12s %12s │ %8s\n", 'Query', 'µs', 'µs', 'µs', 'µs', 'mean ×');
    echo str_repeat('─', 96), "\n";

    $ratios = [];
    foreach ($php['corpus'] as $label => $pr) {
        $er = $ext['corpus'][$label] ?? null;
        if (isset($pr['error']) || !$er || isset($er['error'])) {
            printf("%-26s │ %12s %12s │ %12s %12s │ %8s\n",
                $label, isset($pr['error']) ? 'n/a' : 'ok',
                '', isset($er['error']) ? 'n/a' : 'ok', '', 'n/a');
            continue;
        }
        $ratio = $pr['mean'] / $er['mean'];
        $ratios[] = $ratio;
        printf("%-26s │ %12.1f %12.1f │ %12.1f %12.1f │ %7.1f×\n",
            $label, $pr['p50'], $pr['p99'], $er['p50'], $er['p99'], $ratio);
    }
    if ($ratios) {
        $geomean = pow(array_product($ratios), 1 / count($ratios));
        echo str_repeat('─', 96), "\n";
        printf("%-26s │ %12s %12s │ %12s %12s │ %7.1f×\n",
            'GEOMEAN', '', '', '', '', $geomean);
    }
}

function print_section_b(array $php, array $ext): void
{
    echo "\n── B. Location cost: noLocation: false vs. true (same query) ──────────────────────────────\n";
    printf("%-26s │ %14s %14s │ %14s %14s │ %8s\n",
        '', 'PHP +loc', 'PHP −loc', 'Ext +loc', 'Ext −loc', 'Ext lift');
    printf("%-26s │ %14s %14s │ %14s %14s │ %8s\n",
        'Query', 'mean µs', 'mean µs', 'mean µs', 'mean µs', '+loc/−loc');
    echo str_repeat('─', 100), "\n";

    foreach ($php['corpus'] as $label => $prNoloc) {
        $prLoc = $php['corpus_with_loc'][$label] ?? null;
        $erNoloc = $ext['corpus'][$label] ?? null;
        $erLoc = $ext['corpus_with_loc'][$label] ?? null;

        if (isset($prNoloc['error']) || !$prLoc || isset($prLoc['error'])
            || !$erNoloc || isset($erNoloc['error']) || !$erLoc || isset($erLoc['error'])) {
            continue;
        }
        $extLift = $erLoc['mean'] / $erNoloc['mean'];
        printf("%-26s │ %14.1f %14.1f │ %14.1f %14.1f │ %7.2f×\n",
            $label, $prLoc['mean'], $prNoloc['mean'], $erLoc['mean'], $erNoloc['mean'], $extLift);
    }
}

function print_section_c(array $php, array $ext): void
{
    echo "\n── C. Memory per parse (peak delta, noLocation: false) ────────────────────────────────────\n";
    printf("%-26s │ %14s %14s │ %14s %14s │ %8s\n",
        '', 'PHP p50', 'PHP p95', 'Ext p50', 'Ext p95', 'Reduction');
    printf("%-26s │ %14s %14s │ %14s %14s │ %8s\n",
        'Query', 'bytes', 'bytes', 'bytes', 'bytes', 'PHP/Ext');
    echo str_repeat('─', 100), "\n";

    foreach ($php['memory'] as $label => $pm) {
        $em = $ext['memory'][$label] ?? null;
        if (isset($pm['error']) || !$em || isset($em['error'])) continue;
        $ratio = ($em['p50'] > 0) ? $pm['p50'] / $em['p50'] : 0.0;
        printf("%-26s │ %14s %14s │ %14s %14s │ %7.2f×\n",
            $label,
            number_format((int) $pm['p50']),
            number_format((int) $pm['p95']),
            number_format((int) $em['p50']),
            number_format((int) $em['p95']),
            $ratio);
    }
}

function print_section_d(array $php, array $ext): void
{
    echo "\n── D. Throughput (parses/sec, 5000-iteration loop, noLocation: true) ──────────────────────\n";
    printf("%-26s │ %18s │ %18s │ %8s\n", 'Query', 'PHP calls/s', 'Ext calls/s', 'Speedup');
    echo str_repeat('─', 80), "\n";
    foreach ($php['throughput'] as $label => $pt) {
        $et = $ext['throughput'][$label] ?? null;
        if (isset($pt['error']) || !$et || isset($et['error'])) continue;
        $ratio = $et['cps'] / $pt['cps'];
        printf("%-26s │ %18s │ %18s │ %7.1f×\n",
            $label,
            number_format((int) $pt['cps']),
            number_format((int) $et['cps']),
            $ratio);
    }
}

function print_section_e(array $php, array $ext): void
{
    echo "\n── E. Synthetic stress (noLocation: true) ─────────────────────────────────────────────────\n";
    printf("%-34s │ %12s %12s │ %12s %12s │ %8s\n",
        '', 'PHP p50', 'PHP mean', 'Ext p50', 'Ext mean', 'Speedup');
    printf("%-34s │ %12s %12s │ %12s %12s │ %8s\n",
        'Query', 'µs', 'µs', 'µs', 'µs', 'mean ×');
    echo str_repeat('─', 104), "\n";
    foreach ($php['stress'] as $label => $pr) {
        $er = $ext['stress'][$label] ?? null;
        if (isset($pr['error']) || !$er || isset($er['error'])) {
            printf("%-34s │ %12s │ %12s │ %8s\n", $label,
                isset($pr['error']) ? 'PHP n/a' : '',
                isset($er['error']) ? 'Ext n/a' : '',
                'n/a');
            continue;
        }
        $ratio = $pr['mean'] / $er['mean'];
        printf("%-34s │ %12.1f %12.1f │ %12.1f %12.1f │ %7.1f×\n",
            $label, $pr['p50'], $pr['mean'], $er['p50'], $er['mean'], $ratio);
    }
}

function print_section_f(array $php, array $ext): void
{
    echo "\n── F. Partial parsers (parseValue, parseType, Parser::name, …) ────────────────────────────\n";
    printf("%-34s │ %12s %12s │ %12s %12s │ %8s\n",
        '', 'PHP p50', 'PHP mean', 'Ext p50', 'Ext mean', 'Speedup');
    printf("%-34s │ %12s %12s │ %12s %12s │ %8s\n",
        'Call', 'µs', 'µs', 'µs', 'µs', 'mean ×');
    echo str_repeat('─', 104), "\n";
    foreach ($php['partials'] as $label => $pr) {
        $er = $ext['partials'][$label] ?? null;
        if (isset($pr['error']) || !$er || isset($er['error'])) {
            printf("%-34s │ %12s │ %12s │ %8s\n", $label,
                isset($pr['error']) ? 'PHP n/a' : '',
                isset($er['error']) ? 'Ext n/a' : '',
                'n/a');
            continue;
        }
        $ratio = $pr['mean'] / $er['mean'];
        printf("%-34s │ %12.1f %12.1f │ %12.1f %12.1f │ %7.1f×\n",
            $label, $pr['p50'], $pr['mean'], $er['p50'], $er['mean'], $ratio);
    }
}

function print_footer(int $warmup, int $iters, string $soPath): void
{
    echo "\n";
    echo "Run config: warmup=$warmup, hot-loop=$iters iterations per query, stress=600 iters,\n";
    echo "            partials=2000 iters, throughput=5000 iters per query.\n";
    echo "Extension:  $soPath\n";
    $size = @filesize($soPath);
    if ($size) echo "Size:       ", number_format((int) $size), " bytes\n";
    echo "Method:     hrtime(true) wall-clock per call, both engines in separate processes\n";
    echo "            via shell_exec to isolate opcache and Composer-autoload state.\n";
}

// ── Entry ────────────────────────────────────────────────────────────────────

if (in_array('--single', $argv, true)) {
    run_single();
}

$soPath = $argv[1] ?? __DIR__ . '/../target/release/libgraphql_accelerator.so';
if (!is_file($soPath)) {
    fwrite(STDERR, "ERROR: Extension .so not found at $soPath\n");
    fwrite(STDERR, "Pass a path: php " . basename(__FILE__) . " /path/to/libgraphql_accelerator.so\n");
    exit(1);
}
$soPath = realpath($soPath);
run_comparison($soPath);
