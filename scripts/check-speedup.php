<?php declare(strict_types=1);

/**
 * Compares two JSON files produced by `benchmarks/detail.php --single` and
 * fails (exit 1) when the extension does not improve the geometric-mean
 * headline latency by at least SPEEDUP_THRESHOLD (default 0.10 = 10 %).
 *
 * Usage: php scripts/check-speedup.php bench-php.json bench-ext.json
 * Env:   SPEEDUP_THRESHOLD=0.10
 */

if ($argc !== 3) {
    fwrite(STDERR, "Usage: check-speedup.php <php.json> <ext.json>\n");
    exit(2);
}

$threshold = (float)(getenv('SPEEDUP_THRESHOLD') ?: '0.10');

$phpRaw = @file_get_contents($argv[1]);
$extRaw = @file_get_contents($argv[2]);

if ($phpRaw === false || $extRaw === false) {
    fwrite(STDERR, "ERROR: cannot read input files\n");
    exit(2);
}

/** @var array<string,mixed>|null $php */
$php = json_decode($phpRaw, true);
/** @var array<string,mixed>|null $ext */
$ext = json_decode($extRaw, true);

if (!is_array($php['corpus'] ?? null) || !is_array($ext['corpus'] ?? null)) {
    fwrite(STDERR, "ERROR: 'corpus' key missing or not an array in one or both JSON files\n");
    exit(2);
}

/** @var array<string,array<string,float>> $phpCorpus */
$phpCorpus = $php['corpus'];
/** @var array<string,array<string,float>> $extCorpus */
$extCorpus = $ext['corpus'];

$logSum = 0.0;
$count  = 0;
$rows   = [];

foreach ($phpCorpus as $label => $phpStats) {
    $phpMean = $phpStats['mean'] ?? null;
    $extMean = $extCorpus[$label]['mean'] ?? null;

    if (!is_numeric($phpMean) || !is_numeric($extMean)) {
        continue; // entry errored in one run — skip
    }

    $phpMean = (float)$phpMean;
    $extMean = (float)$extMean;

    if ($phpMean <= 0.0 || $extMean <= 0.0) {
        continue;
    }

    $ratio   = $phpMean / $extMean;      // >1 means extension is faster
    $speedup = 1.0 - 1.0 / $ratio;      // fractional improvement (0.10 = 10 %)
    $logSum += log($ratio);
    $count++;
    $rows[] = [$label, $phpMean, $extMean, $speedup, $ratio];
}

if ($count === 0) {
    fwrite(STDERR, "ERROR: no comparable corpus entries found in both files\n");
    exit(2);
}

$geoMeanRatio   = exp($logSum / $count);
$overallSpeedup = 1.0 - 1.0 / $geoMeanRatio;

// ── Table ────────────────────────────────────────────────────────────────────
$col = 44;
$sep = str_repeat('─', $col + 52);

printf(
    "\nPHP %-10s vs Extension %-10s   (%d queries compared)\n\n",
    $php['php_version'] ?? '?',
    $ext['php_version'] ?? '?',
    $count
);
printf("%-{$col}s  %12s  %12s  %9s  %6s\n", 'Query', 'PHP (µs)', 'Ext (µs)', 'Speedup', 'Ratio');
echo $sep . "\n";

foreach ($rows as [$label, $phpMean, $extMean, $speedup, $ratio]) {
    $sign = $speedup >= 0 ? '+' : '';
    printf(
        "%-{$col}s  %12s  %12s  %8s%%  %5.2fx\n",
        $label,
        number_format($phpMean * 1e6, 2),
        number_format($extMean * 1e6, 2),
        $sign . number_format($speedup * 100, 1),
        $ratio
    );
}

echo $sep . "\n";
printf(
    "Geometric mean speedup:  %+.1f%%   (required: ≥%.0f%%)\n\n",
    $overallSpeedup * 100,
    $threshold * 100
);

if ($overallSpeedup < $threshold) {
    printf(
        "FAIL: %.1f%% < %.0f%% — extension does not meet the performance gate.\n",
        $overallSpeedup * 100,
        $threshold * 100
    );
    exit(1);
}

printf(
    "PASS: %.1f%% ≥ %.0f%% — extension meets the performance gate.\n",
    $overallSpeedup * 100,
    $threshold * 100
);
exit(0);
