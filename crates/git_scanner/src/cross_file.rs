//! Cross-file pipeline coordinator. Analyzers own domain decisions; this module
//! owns database access, protocol validation, failure isolation, and cleanup.

use codeprism_analyzer::FileProcessor;
use codeprism_core::{FinalizeFinding, FinalizeOutput, IntermediateBlock, TAG_METRIC};
use sqlx::{Pool, Sqlite, Transaction};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Default)]
pub struct FinalizeReport {
    pub errors: HashMap<String, String>,
}

pub async fn finalize_all(
    scan_id: i64,
    pool: &Pool<Sqlite>,
    analyzers: &HashMap<String, Box<dyn FileProcessor>>,
) -> FinalizeReport {
    let mut report = FinalizeReport::default();
    for (name, analyzer) in analyzers {
        println!("Running cross-file aggregation for '{}'...", name);
        if let Err(error) = finalize_one(scan_id, pool, name, analyzer.as_ref()).await {
            eprintln!("Cross-file analyzer '{}' failed: {}", name, error);
            report.errors.insert(name.clone(), error.to_string());
        }
    }
    report
}

async fn finalize_one(
    scan_id: i64,
    pool: &Pool<Sqlite>,
    analyzer_id: &str,
    analyzer: &dyn FileProcessor,
) -> anyhow::Result<()> {
    let blocks = load_blocks(scan_id, pool, analyzer_id).await?;
    let output = match analyzer.finalize(blocks.clone()).await {
        Ok(output) => output,
        Err(first_error) if analyzer.is_transient_error(&first_error) => {
            analyzer.reset();
            analyzer.finalize(blocks).await.map_err(|retry_error| {
                anyhow::anyhow!("{}; retry failed: {}", first_error, retry_error)
            })?
        }
        Err(error) => return Err(error),
    };
    validate_output(&output)?;

    let mut tx = pool.begin().await?;
    persist_output(scan_id, analyzer_id, output, &mut tx).await?;
    sqlx::query("DELETE FROM intermediate_blocks WHERE scan_id = ? AND analyzer_id = ?")
        .bind(scan_id)
        .bind(analyzer_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

async fn load_blocks(
    scan_id: i64,
    pool: &Pool<Sqlite>,
    analyzer_id: &str,
) -> anyhow::Result<Vec<IntermediateBlock>> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, Option<i64>, Option<i64>, Option<i64>, Option<String>, Option<String>)>(
        "SELECT file_path, group_key, blob_data, int_data1, int_data2, int_data3, str_data1, str_data2 \
         FROM intermediate_blocks WHERE scan_id = ? AND analyzer_id = ? ORDER BY group_key, file_path",
    )
    .bind(scan_id)
    .bind(analyzer_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(
                file_path,
                group_key,
                blob_data,
                int_data1,
                int_data2,
                int_data3,
                str_data1,
                str_data2,
            )| IntermediateBlock {
                analyzer_id: analyzer_id.to_string(),
                file_path,
                group_key,
                blob_data,
                int_data1,
                int_data2,
                int_data3,
                str_data1,
                str_data2,
            },
        )
        .collect())
}

fn validate_output(output: &FinalizeOutput) -> anyhow::Result<()> {
    let mut finding_keys = HashSet::new();
    for finding in &output.findings {
        if finding.finding_key.trim().is_empty() {
            anyhow::bail!("finding_key must not be empty");
        }
        if !finding_keys.insert(&finding.finding_key) {
            anyhow::bail!("duplicate finding_key '{}'", finding.finding_key);
        }
        for occurrence in &finding.occurrences {
            if occurrence.file_path.trim().is_empty()
                || occurrence.line_start < 0
                || occurrence.line_end < occurrence.line_start
            {
                anyhow::bail!("invalid occurrence in finding '{}'", finding.finding_key);
            }
            if !matches!(occurrence.side.as_deref(), None | Some("0") | Some("1")) {
                anyhow::bail!("invalid side in finding '{}'", finding.finding_key);
            }
            if !matches!(
                occurrence.change_type.as_deref(),
                None | Some("A") | Some("M") | Some("D")
            ) {
                anyhow::bail!("invalid change_type in finding '{}'", finding.finding_key);
            }
        }
        for metric in &finding.metrics {
            if metric.metric_key.trim().is_empty()
                || metric.file_path.trim().is_empty()
                || !metric.value_before.is_finite()
                || !metric.value_after.is_finite()
            {
                anyhow::bail!("invalid metric in finding '{}'", finding.finding_key);
            }
            if !matches!(
                metric.change_type.as_deref(),
                None | Some("A") | Some("M") | Some("D")
            ) {
                anyhow::bail!(
                    "invalid metric change_type in finding '{}'",
                    finding.finding_key
                );
            }
        }
    }
    Ok(())
}

async fn persist_output(
    scan_id: i64,
    analyzer_id: &str,
    output: FinalizeOutput,
    tx: &mut Transaction<'_, Sqlite>,
) -> anyhow::Result<()> {
    let stored_analyzer_id = format!("{}_aggregated", analyzer_id);
    for finding in output.findings {
        persist_finding(scan_id, &stored_analyzer_id, finding, tx).await?;
    }
    Ok(())
}

async fn persist_finding(
    scan_id: i64,
    analyzer_id: &str,
    finding: FinalizeFinding,
    tx: &mut Transaction<'_, Sqlite>,
) -> anyhow::Result<()> {
    let content = finding.content.unwrap_or_default();
    let content_hash = codeprism_core::hash_match_content(&content);
    let (content_id, stored_content): (i64, String) = sqlx::query_as(
        "INSERT INTO match_contents (content_hash, content, content_bytes, line_count) VALUES (?, ?, ?, ?) \
         ON CONFLICT(content_hash) DO UPDATE SET content_hash = excluded.content_hash RETURNING id, content",
    )
    .bind(&content_hash).bind(&content).bind(content.len() as i64)
    .bind(content.lines().count() as i64).fetch_one(&mut **tx).await?;
    if stored_content != content {
        anyhow::bail!("SHA-256 collision while storing finding content");
    }

    for occurrence in &finding.occurrences {
        let side = occurrence
            .side
            .as_deref()
            .and_then(|value| value.parse::<i64>().ok());
        sqlx::query(
            "INSERT INTO matches (scan_id, file_path, analyzer_id, content_id, finding_key, line_start, line_end, side, change_type) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(scan_id).bind(&occurrence.file_path).bind(analyzer_id).bind(content_id)
        .bind(&finding.finding_key).bind(occurrence.line_start as i64).bind(occurrence.line_end as i64)
        .bind(side).bind(occurrence.change_type.as_deref().unwrap_or("A"))
        .execute(&mut **tx).await?;
    }
    for metric in &finding.metrics {
        let mut tags = finding.tags.clone();
        tags.extend(metric.tags.clone());
        tags.insert(TAG_METRIC.to_string(), metric.metric_key.clone());
        let sorted: BTreeMap<_, _> = tags.iter().collect();
        let tags_json = serde_json::to_string(&sorted)?;
        sqlx::query(
            "INSERT INTO metrics (scan_id, file_path, change_type, tech_stack, analyzer_id, content_id, finding_key, tags, value_before, value_after, scope) \
             VALUES (?, ?, ?, NULL, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(scan_id).bind(&metric.file_path).bind(metric.change_type.as_deref().unwrap_or("A"))
        .bind(analyzer_id).bind(content_id).bind(&finding.finding_key).bind(tags_json)
        .bind(metric.value_before).bind(metric.value_after).bind(&metric.scope)
        .execute(&mut **tx).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use codeprism_analyzer::Analyzer;
    use codeprism_core::{FinalizeMetric, FinalizeOccurrence, MatchDetail, MetricEntry};

    struct FakeAnalyzer {
        id: &'static str,
        fail: bool,
    }

    impl Analyzer for FakeAnalyzer {
        fn id(&self) -> &str {
            self.id
        }
        fn analyze(&self, _: &str, _: &str) -> Vec<MetricEntry> {
            vec![]
        }
        fn extract_matches(&self, _: &str, _: &str) -> Vec<MatchDetail> {
            vec![]
        }
        fn as_file_processor(&self) -> Option<&dyn FileProcessor> {
            Some(self)
        }
    }

    #[async_trait]
    impl FileProcessor for FakeAnalyzer {
        fn extract_blocks(&self, _: &str, _: &str) -> Vec<IntermediateBlock> {
            vec![]
        }
        async fn finalize(&self, blocks: Vec<IntermediateBlock>) -> anyhow::Result<FinalizeOutput> {
            if self.fail {
                anyhow::bail!("intentional failure");
            }
            let block = &blocks[0];
            Ok(FinalizeOutput {
                findings: vec![FinalizeFinding {
                    finding_key: block.group_key.clone(),
                    content: block.blob_data.clone(),
                    occurrences: vec![FinalizeOccurrence {
                        file_path: block.file_path.clone(),
                        line_start: 1,
                        line_end: 2,
                        change_type: Some("A".into()),
                        side: None,
                    }],
                    tags: HashMap::new(),
                    metrics: vec![FinalizeMetric {
                        metric_key: "custom_count".into(),
                        file_path: block.file_path.clone(),
                        value_before: 0.0,
                        value_after: 7.0,
                        change_type: Some("A".into()),
                        scope: None,
                        tags: HashMap::new(),
                    }],
                }],
            })
        }
    }

    #[tokio::test]
    async fn isolates_failures_and_cleans_only_successful_analyzers() {
        let db = codeprism_database::Db::new("sqlite::memory:")
            .await
            .unwrap();
        db.migrate().await.unwrap();
        sqlx::query("INSERT INTO projects (name) VALUES ('test')")
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO scans (project_id, commit_hash, scan_mode) VALUES (1, 'x', 'SNAPSHOT')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        for analyzer_id in ["good", "bad"] {
            sqlx::query("INSERT INTO intermediate_blocks (scan_id, analyzer_id, file_path, group_key, blob_data) VALUES (1, ?, 'a.rs', 'group', 'body')")
                .bind(analyzer_id).execute(db.pool()).await.unwrap();
        }
        let mut analyzers: HashMap<String, Box<dyn FileProcessor>> = HashMap::new();
        analyzers.insert(
            "good".into(),
            Box::new(FakeAnalyzer {
                id: "good",
                fail: false,
            }),
        );
        analyzers.insert(
            "bad".into(),
            Box::new(FakeAnalyzer {
                id: "bad",
                fail: true,
            }),
        );

        let report = finalize_all(1, db.pool(), &analyzers).await;
        assert_eq!(report.errors.len(), 1);
        assert!(report.errors.contains_key("bad"));
        let remaining: Vec<String> =
            sqlx::query_scalar("SELECT analyzer_id FROM intermediate_blocks ORDER BY analyzer_id")
                .fetch_all(db.pool())
                .await
                .unwrap();
        assert_eq!(remaining, vec!["bad"]);
        let value: f64 = sqlx::query_scalar(
            "SELECT value_after FROM metrics WHERE analyzer_id = 'good_aggregated'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(value, 7.0);
    }
}
