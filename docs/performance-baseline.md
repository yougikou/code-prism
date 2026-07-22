# Performance baseline

This document records the first repeatable performance baseline for CodePrism. It is intended to keep later optimizations measurable instead of relying on subjective UI or scan impressions.

## Web bundle

Measured with `npm run build` in `web/` on 2026-07-22:

| Artifact | Before | After |
| --- | ---: | ---: |
| Initial JavaScript, gzip | 516.82 kB | 91.19 kB |
| Initial JavaScript, raw | 1,638.61 kB | 286.46 kB |
| Dashboard page, gzip | included above | 19.33 kB |
| Charts, gzip | included above | 227.14 kB |

The dashboard, execution, and configuration pages are loaded on demand. The chart runtime is only loaded with the dashboard, and ECharts registers only the chart types and components used by CodePrism. The chart chunk remains intentionally separate and may still exceed Vite's raw 500 kB warning threshold; it is not part of the initial application route.

## Request behavior

- Identical in-flight dashboard view and trend requests share one network request.
- Successful view responses are cached for 15 seconds and trend responses for 30 seconds.
- Each consumer has its own abort signal. The underlying fetch is cancelled when all consumers have left.
- Scan-job polling starts at 2 seconds, backs off to at most 15 seconds while progress is unchanged, resets when progress advances, and pauses while the document is hidden.

The cache is deliberately short-lived and process-local. Scan execution and mutation endpoints are not cached.

## Scanner persistence

Metrics for a file, match rows for an analyzer result, and intermediate blocks for a cross-file analyzer are each persisted in a transaction. This preserves existing row-level SQL and error behavior while avoiding an implicit commit for every inserted row. Match-content cache entries become visible only after the transaction commits.

Existing query indexes cover the primary scan, analyzer, finding, file, content, and cross-file grouping access paths in `crates/database/src/init.sql`. Any future index should be justified with `EXPLAIN QUERY PLAN` against a representative database rather than added speculatively.

## Reproducing the baseline

```bash
cd web
npm run lint
npm test
npm run build

cd ..
cargo fmt --all -- --check
CODEPRISM_SKIP_WEB_BUILD=1 cargo clippy --workspace --all-targets -- -D warnings
CODEPRISM_SKIP_WEB_BUILD=1 cargo test --workspace
```

Record the generated Vite artifact sizes and the machine, dataset, and command used for any scanner benchmark. Bundle numbers are useful for regression comparison but are not cross-machine scan-throughput measurements.
