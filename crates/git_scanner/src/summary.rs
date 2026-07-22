use super::Scanner;

impl Scanner {
    pub(super) async fn save_scan_summary(
        &self,
        scan_id: i64,
        total_files: u64,
    ) -> anyhow::Result<()> {
        let total_errors: u64 = self
            .analyzer_error_details
            .values()
            .map(|v| v.len() as u64)
            .sum::<u64>()
            + self
                .cross_file_error_details
                .values()
                .map(|v| v.len() as u64)
                .sum::<u64>();
        let load_errors: Vec<&String> = self
            .analyzer_load_errors
            .iter()
            .chain(self.cross_file_load_errors.iter())
            .collect();
        let load_errors_json = serde_json::to_string(&load_errors)?;

        let mut analyzer_stats: Vec<serde_json::Value> = self
            .analyzers
            .keys()
            .map(|id| {
                let files = self.analyzer_exec_count.get(id).copied().unwrap_or(0);
                let details = self
                    .analyzer_error_details
                    .get(id)
                    .cloned()
                    .unwrap_or_default();
                serde_json::json!({
                    "analyzer_id": id,
                    "files_analyzed": files,
                    "execution_errors": details.len(),
                    "error_details": details,
                })
            })
            .collect();
        for id in self.cross_file_analyzers.keys() {
            let files = self.cross_file_exec_count.get(id).copied().unwrap_or(0);
            let details = self
                .cross_file_error_details
                .get(id)
                .cloned()
                .unwrap_or_default();
            analyzer_stats.push(serde_json::json!({
                "analyzer_id": id,
                "files_analyzed": files,
                "execution_errors": details.len(),
                "error_details": details,
                "type": "cross_file",
            }));
        }

        let total_executions: u64 = self.analyzer_exec_count.values().sum::<u64>()
            + self.cross_file_exec_count.values().sum::<u64>();
        let executed_count = self
            .analyzer_exec_count
            .values()
            .filter(|&&count| count > 0)
            .count()
            + self
                .cross_file_exec_count
                .values()
                .filter(|&&count| count > 0)
                .count();
        let total_load_count = self.analyzers.len() + self.cross_file_analyzers.len();

        sqlx::query(
            "INSERT INTO scan_summaries (scan_id, total_files_scanned, total_analyzers_loaded, \
             total_analyzers_executed, total_analyzer_executions, total_errors, load_errors, analyzer_stats) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(scan_id).bind(total_files as i64).bind(total_load_count as i64)
        .bind(executed_count as i64).bind(total_executions as i64).bind(total_errors as i64)
        .bind(load_errors_json).bind(serde_json::to_string(&analyzer_stats)?)
        .execute(self.db.pool()).await?;
        Ok(())
    }
}
