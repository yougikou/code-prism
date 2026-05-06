use crate::config::{ViewConfig, ViewKind};
use anyhow::Result;
use codeprism_core::SortOrder;
use serde::Serialize;
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema, Clone)]
pub struct AggregationResult {
    pub label: String,
    pub value: f64,
    pub tech_stack: Option<String>,
    pub category: Option<String>,
    pub change_type: Option<String>,
    pub metric_key: Option<String>,
    pub analyzer_id: Option<String>,
    #[schema(value_type = Option<Vec<AggregationResult>>)]
    pub children: Option<Vec<AggregationResult>>,
    pub group_key: Option<String>,
}

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
pub struct ViewFilters {
    pub tech_stack: Option<String>,
    pub category: Option<String>,
    pub metric_key: Option<String>,
    /// Filter by change type: Add, Modify, Delete
    pub change_type: Option<String>,
    /// Comma separated list of fields to group by (e.g. "tech_stack,change_type")
    pub group_by: Option<String>,
}

pub struct TopNAggregator;

impl TopNAggregator {
    pub async fn execute(
        pool: &SqlitePool,
        scan_id: i64,
        view_config: &ViewConfig,
        filters: &ViewFilters,
    ) -> Result<Vec<AggregationResult>> {
        let (source, params) = match &view_config.kind {
            ViewKind::TopN { source, params } => (source, params),
            _ => return Ok(vec![]), // Should not reach here if routed correctly
        };

        let mut query = String::from(
            "SELECT file_path, value_after, tech_stack, category, change_type, metric_key, analyzer_id
             FROM metrics 
             WHERE scan_id = ?",
        );

        // Optional source filters (from view config)
        if source.metric_key.is_some() {
            query.push_str(" AND metric_key = ?");
        }
        if !source.analyzer_id.is_empty() {
            query.push_str(" AND analyzer_id = ?");
        }
        if source.category.is_some() {
            query.push_str(" AND category = ?");
        }

        // Apply dynamic filters (from request params)
        if filters.tech_stack.is_some() {
            query.push_str(" AND tech_stack = ?");
        }
        if filters.category.is_some() {
            query.push_str(" AND category = ?");
        }
        if filters.metric_key.is_some() {
            query.push_str(" AND metric_key = ?");
        }
        if filters.change_type.is_some() {
            query.push_str(" AND change_type = ?");
        }

        let order_dir = match params.order {
            SortOrder::Asc => "ASC",
            SortOrder::Desc => "DESC",
        };
        query.push_str(&format!(" ORDER BY value_after {} LIMIT ?", order_dir));

        let mut sql_query = sqlx::query(&query).bind(scan_id);

        // Bind source filters
        if let Some(mk) = &source.metric_key {
            sql_query = sql_query.bind(mk);
        }
        if !source.analyzer_id.is_empty() {
            sql_query = sql_query.bind(&source.analyzer_id);
        }
        if let Some(cat) = &source.category {
            sql_query = sql_query.bind(cat);
        }

        // Bind dynamic filters
        if let Some(ts) = &filters.tech_stack {
            sql_query = sql_query.bind(ts);
        }
        if let Some(cat) = &filters.category {
            sql_query = sql_query.bind(cat);
        }
        if let Some(mk) = &filters.metric_key {
            sql_query = sql_query.bind(mk);
        }
        if let Some(ct) = &filters.change_type {
            sql_query = sql_query.bind(ct);
        }

        sql_query = sql_query.bind(params.limit);

        let rows = sql_query.fetch_all(pool).await?;

        let mut results: Vec<AggregationResult> = rows
            .into_iter()
            .map(|row| AggregationResult {
                label: row.try_get::<String, _>("file_path").unwrap_or_default(),
                value: row.try_get::<f64, _>("value_after").unwrap_or_default(),
                tech_stack: row
                    .try_get::<Option<String>, _>("tech_stack")
                    .unwrap_or_default(),
                category: row
                    .try_get::<Option<String>, _>("category")
                    .unwrap_or_default(),
                change_type: row
                    .try_get::<Option<String>, _>("change_type")
                    .unwrap_or_default(),
                metric_key: row
                    .try_get::<Option<String>, _>("metric_key")
                    .unwrap_or_default(),
                analyzer_id: row
                    .try_get::<Option<String>, _>("analyzer_id")
                    .unwrap_or_default(),
                children: None,
                group_key: None,
            })
            .collect();

        // Perform grouping if requested
        let effective_group_by = if let Some(g) = &filters.group_by {
            Some(g.clone())
        } else if !view_config.group_by.is_empty() {
            Some(view_config.group_by.join(","))
        } else {
            None
        };

        if let Some(group_by_str) = effective_group_by {
            let keys: Vec<&str> = group_by_str
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            if !keys.is_empty() {
                results = Self::group_recursive(results, &keys, view_config.include_children);
            }
        }

        Ok(results)
    }

    pub fn group_recursive(
        items: Vec<AggregationResult>,
        keys: &[&str],
        include_children: bool,
    ) -> Vec<AggregationResult> {
        if keys.is_empty() || items.is_empty() {
            return items;
        }

        let current_key = keys[0];
        let remaining_keys = &keys[1..];
        let mut groups: HashMap<String, Vec<AggregationResult>> = HashMap::new();

        // 1. Partition items
        for item in items {
            let key_value = match current_key {
                "tech_stack" => item
                    .tech_stack
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string()),
                "category" => item
                    .category
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string()),
                "change_type" => item
                    .change_type
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string()),
                "metric_key" => item
                    .metric_key
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string()),
                "analyzer_id" => item
                    .analyzer_id
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string()),
                // Add more keys as needed, maybe reflection or map?
                _ => "Other".to_string(), // Fallback
            };
            groups.entry(key_value).or_default().push(item);
        }

        // 2. Build group nodes
        let mut group_nodes: Vec<AggregationResult> = Vec::new();

        for (group_val, mut children) in groups {
            // Apply recursion
            if !remaining_keys.is_empty() {
                children = Self::group_recursive(children, remaining_keys, include_children);
            }

            // Calculate aggregate value (SUM of immediate children values)
            let total_value: f64 = children.iter().map(|c| c.value).sum();

            // Filter children if requested
            // Only strictly apply exclusion if we are at the bottom (no more grouping keys).
            // If there ARE remaining keys, 'children' are the sub-groups, which we almost certainly want to keep
            // to show the hierarchy, otherwise grouping is useless.
            let children_val = if !remaining_keys.is_empty() || include_children {
                Some(children)
            } else {
                None
            };

            let mut node = AggregationResult {
                label: group_val.clone(),
                value: total_value,
                tech_stack: None,
                category: None,
                change_type: None,
                metric_key: None,
                analyzer_id: None,
                children: children_val,
                group_key: Some(current_key.to_string()),
            };

            // Populate the specific field for this group
            match current_key {
                "tech_stack" => node.tech_stack = Some(group_val.clone()),
                "category" => node.category = Some(group_val.clone()),
                "change_type" => node.change_type = Some(group_val.clone()),
                "metric_key" => node.metric_key = Some(group_val.clone()),
                "analyzer_id" => node.analyzer_id = Some(group_val.clone()),
                _ => {}
            }

            group_nodes.push(node);
        }

        // Sort groups by value desc
        group_nodes.sort_by(|a, b| {
            b.value
                .partial_cmp(&a.value)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        group_nodes
    }
}

pub struct SumAggregator;

impl SumAggregator {
    pub async fn execute(
        pool: &SqlitePool,
        scan_id: i64,
        view_config: &ViewConfig,
        filters: &ViewFilters,
    ) -> Result<Vec<AggregationResult>> {
        let source = match &view_config.kind {
            ViewKind::Sum { source } => source,
            _ => return Ok(vec![]),
        };

        // Reuse TopNAggregator logic but without LIMIT and Sorting?
        // Actually, the main difference is the intention.
        // Sum view usually implies we want the total sum, OR if grouped, sum per group.
        // If we use the same query as TopN but w/o LIMIT, we get all rows.
        // Then we can group or just sum everything.

        let mut query = String::from(
            "SELECT file_path, value_after, tech_stack, category, change_type, metric_key, analyzer_id
             FROM metrics 
             WHERE scan_id = ?",
        );

        // Optional source filters (from view config)
        if source.metric_key.is_some() {
            query.push_str(" AND metric_key = ?");
        }
        if !source.analyzer_id.is_empty() {
            query.push_str(" AND analyzer_id = ?");
        }
        if source.category.is_some() {
            query.push_str(" AND category = ?");
        }

        // Apply dynamic filters (from request params)
        if filters.tech_stack.is_some() {
            query.push_str(" AND tech_stack = ?");
        }
        if filters.category.is_some() {
            query.push_str(" AND category = ?");
        }
        if filters.metric_key.is_some() {
            query.push_str(" AND metric_key = ?");
        }
        if filters.change_type.is_some() {
            query.push_str(" AND change_type = ?");
        }

        // NO ORDER BY or LIMIT for Sum

        let mut sql_query = sqlx::query(&query).bind(scan_id);

        // Bind source filters
        if let Some(mk) = &source.metric_key {
            sql_query = sql_query.bind(mk);
        }
        if !source.analyzer_id.is_empty() {
            sql_query = sql_query.bind(&source.analyzer_id);
        }
        if let Some(cat) = &source.category {
            sql_query = sql_query.bind(cat);
        }

        // Bind dynamic filters
        if let Some(ts) = &filters.tech_stack {
            sql_query = sql_query.bind(ts);
        }
        if let Some(cat) = &filters.category {
            sql_query = sql_query.bind(cat);
        }
        if let Some(mk) = &filters.metric_key {
            sql_query = sql_query.bind(mk);
        }
        if let Some(ct) = &filters.change_type {
            sql_query = sql_query.bind(ct);
        }

        let rows = sql_query.fetch_all(pool).await?;

        let mut results: Vec<AggregationResult> = rows
            .into_iter()
            .map(|row| AggregationResult {
                label: row.try_get::<String, _>("file_path").unwrap_or_default(),
                value: row.try_get::<f64, _>("value_after").unwrap_or_default(),
                tech_stack: row
                    .try_get::<Option<String>, _>("tech_stack")
                    .unwrap_or_default(),
                category: row
                    .try_get::<Option<String>, _>("category")
                    .unwrap_or_default(),
                change_type: row
                    .try_get::<Option<String>, _>("change_type")
                    .unwrap_or_default(),
                metric_key: row
                    .try_get::<Option<String>, _>("metric_key")
                    .unwrap_or_default(),
                analyzer_id: row
                    .try_get::<Option<String>, _>("analyzer_id")
                    .unwrap_or_default(),
                children: None,
                group_key: None,
            })
            .collect();

        // Perform grouping
        let effective_group_by = if let Some(g) = &filters.group_by {
            Some(g.clone())
        } else if !view_config.group_by.is_empty() {
            Some(view_config.group_by.join(","))
        } else {
            None
        };

        if let Some(group_by_str) = effective_group_by {
            let keys: Vec<&str> = group_by_str
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            if !keys.is_empty() {
                // If grouped, return the groups (which are sums by definition of group_recursive)
                results =
                    TopNAggregator::group_recursive(results, &keys, view_config.include_children);
            } else {
                // If no keys, maybe just one big sum?
                // But default behavior if group_by string is empty is to return list.
                // For "Sum" view, returning individual files might not be what is expected if NOT grouped?
                // But technically "Sum" view is just a Data Source mode.
                // If the user wants a single scalar sum, they can read the client side or we provide a special group_by="all"?
                // For now, let's keep consistency: returns items.
                // If we strictly want SUM, we should probably output 1 row.
                let total: f64 = results.iter().map(|r| r.value).sum();
                results = vec![AggregationResult {
                    label: "Total".to_string(),
                    value: total,
                    tech_stack: None,
                    category: None,
                    change_type: None,
                    metric_key: None,
                    analyzer_id: None,
                    children: None,
                    group_key: None,
                }];
            }
        } else {
            // Explicit default for Sum View: Single Total if no grouping is specified!
            let total: f64 = results.iter().map(|r| r.value).sum();
            results = vec![AggregationResult {
                label: "Total".to_string(),
                value: total,
                tech_stack: None,
                category: None,
                change_type: None,
                metric_key: None,
                analyzer_id: None,
                children: None,
                group_key: None,
            }];
        }

        Ok(results)
    }
}

// ============ Stat Aggregator (AVG, MIN, MAX) ============
pub struct StatAggregator;

#[derive(Debug, Clone, Copy)]
pub enum StatType {
    Avg,
    Min,
    Max,
}

impl StatAggregator {
    pub async fn execute(
        pool: &SqlitePool,
        scan_id: i64,
        view_config: &ViewConfig,
        filters: &ViewFilters,
        stat_type: StatType,
    ) -> Result<Vec<AggregationResult>> {
        let source = match &view_config.kind {
            ViewKind::Avg { source } | ViewKind::Min { source } | ViewKind::Max { source } => {
                source
            }
            _ => return Ok(vec![]),
        };

        let stat_fn = match stat_type {
            StatType::Avg => "AVG",
            StatType::Min => "MIN",
            StatType::Max => "MAX",
        };

        let mut query = format!(
            "SELECT {}(value_after) as stat_value, tech_stack, category, metric_key, analyzer_id, change_type
             FROM metrics
             WHERE scan_id = ?",
            stat_fn
        );

        // Optional source filters (from view config)
        if source.metric_key.is_some() {
            query.push_str(" AND metric_key = ?");
        }
        if !source.analyzer_id.is_empty() {
            query.push_str(" AND analyzer_id = ?");
        }
        if source.category.is_some() {
            query.push_str(" AND category = ?");
        }

        // Dynamic filters
        if filters.tech_stack.is_some() {
            query.push_str(" AND tech_stack = ?");
        }
        if filters.category.is_some() {
            query.push_str(" AND category = ?");
        }
        if filters.metric_key.is_some() {
            query.push_str(" AND metric_key = ?");
        }
        if filters.change_type.is_some() {
            query.push_str(" AND change_type = ?");
        }

        // Grouping
        let effective_group_by = if let Some(g) = &filters.group_by {
            Some(g.clone())
        } else if !view_config.group_by.is_empty() {
            Some(view_config.group_by.join(","))
        } else {
            None
        };

        if let Some(ref group_by_str) = effective_group_by {
            const ALLOWED_GROUP_KEYS: &[&str] = &[
                "tech_stack", "category", "change_type", "metric_key", "analyzer_id",
            ];
            let keys: Vec<&str> = group_by_str
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty() && ALLOWED_GROUP_KEYS.contains(s))
                .collect();
            if !keys.is_empty() {
                query.push_str(" GROUP BY ");
                query.push_str(&keys.join(", "));
            }
        }

        let mut sql_query = sqlx::query(&query).bind(scan_id);

        // Bind source filters
        if let Some(mk) = &source.metric_key {
            sql_query = sql_query.bind(mk);
        }
        if !source.analyzer_id.is_empty() {
            sql_query = sql_query.bind(&source.analyzer_id);
        }
        if let Some(cat) = &source.category {
            sql_query = sql_query.bind(cat);
        }

        // Bind dynamic filters
        if let Some(ts) = &filters.tech_stack {
            sql_query = sql_query.bind(ts);
        }
        if let Some(cat) = &filters.category {
            sql_query = sql_query.bind(cat);
        }
        if let Some(mk) = &filters.metric_key {
            sql_query = sql_query.bind(mk);
        }
        if let Some(ct) = &filters.change_type {
            sql_query = sql_query.bind(ct);
        }

        let rows = sql_query.fetch_all(pool).await?;

        let results: Vec<AggregationResult> = rows
            .into_iter()
            .map(|row| {
                let label = if effective_group_by.is_some() {
                    row.try_get::<Option<String>, _>("tech_stack")
                        .unwrap_or_default()
                        .unwrap_or_else(|| "Unknown".to_string())
                } else {
                    format!("{} Value", stat_fn)
                };
                AggregationResult {
                    label,
                    value: row.try_get::<f64, _>("stat_value").unwrap_or_default(),
                    tech_stack: row
                        .try_get::<Option<String>, _>("tech_stack")
                        .unwrap_or_default(),
                    category: row
                        .try_get::<Option<String>, _>("category")
                        .unwrap_or_default(),
                    change_type: row
                        .try_get::<Option<String>, _>("change_type")
                        .unwrap_or_default(),
                    metric_key: row
                        .try_get::<Option<String>, _>("metric_key")
                        .unwrap_or_default(),
                    analyzer_id: row
                        .try_get::<Option<String>, _>("analyzer_id")
                        .unwrap_or_default(),
                    children: None,
                    group_key: None,
                }
            })
            .collect();

        Ok(results)
    }
}

// ============ Distribution Aggregator ============
pub struct DistributionAggregator;

impl DistributionAggregator {
    pub async fn execute(
        pool: &SqlitePool,
        scan_id: i64,
        view_config: &ViewConfig,
        filters: &ViewFilters,
    ) -> Result<Vec<AggregationResult>> {
        let (source, params) = match &view_config.kind {
            ViewKind::Distribution { source, params } => (source, params),
            _ => return Ok(vec![]),
        };

        // Fetch all values
        let mut query = String::from(
            "SELECT value_after, tech_stack, category, metric_key, analyzer_id
             FROM metrics
             WHERE scan_id = ?",
        );

        // Optional source filters (from view config)
        if source.metric_key.is_some() {
            query.push_str(" AND metric_key = ?");
        }
        if !source.analyzer_id.is_empty() {
            query.push_str(" AND analyzer_id = ?");
        }
        if source.category.is_some() {
            query.push_str(" AND category = ?");
        }

        // Dynamic filters
        if filters.tech_stack.is_some() {
            query.push_str(" AND tech_stack = ?");
        }
        if filters.category.is_some() {
            query.push_str(" AND category = ?");
        }
        if filters.metric_key.is_some() {
            query.push_str(" AND metric_key = ?");
        }
        if filters.change_type.is_some() {
            query.push_str(" AND change_type = ?");
        }

        let mut sql_query = sqlx::query(&query).bind(scan_id);

        // Bind source filters
        if let Some(mk) = &source.metric_key {
            sql_query = sql_query.bind(mk);
        }
        if !source.analyzer_id.is_empty() {
            sql_query = sql_query.bind(&source.analyzer_id);
        }
        if let Some(cat) = &source.category {
            sql_query = sql_query.bind(cat);
        }

        // Bind dynamic filters
        if let Some(ts) = &filters.tech_stack {
            sql_query = sql_query.bind(ts);
        }
        if let Some(cat) = &filters.category {
            sql_query = sql_query.bind(cat);
        }
        if let Some(mk) = &filters.metric_key {
            sql_query = sql_query.bind(mk);
        }
        if let Some(ct) = &filters.change_type {
            sql_query = sql_query.bind(ct);
        }

        let rows = sql_query.fetch_all(pool).await?;

        // Create bucket counts
        let buckets = &params.buckets;
        let mut bucket_counts: Vec<i64> = vec![0; buckets.len() + 1];

        for row in rows {
            let value: f64 = row.try_get("value_after").unwrap_or_default();
            let mut placed = false;
            for (i, &boundary) in buckets.iter().enumerate() {
                if value < boundary {
                    bucket_counts[i] += 1;
                    placed = true;
                    break;
                }
            }
            if !placed {
                bucket_counts[buckets.len()] += 1;
            }
        }

        // Build results with bucket labels
        let mut results: Vec<AggregationResult> = Vec::new();
        for (i, count) in bucket_counts.iter().enumerate() {
            let label = if i == 0 {
                format!("< {}", buckets.first().unwrap_or(&0.0))
            } else if i == buckets.len() {
                format!(">= {}", buckets.last().unwrap_or(&0.0))
            } else {
                format!("{} - {}", buckets[i - 1], buckets[i])
            };

            results.push(AggregationResult {
                label,
                value: *count as f64,
                tech_stack: None,
                category: None,
                change_type: None,
                metric_key: None,
                analyzer_id: None,
                children: None,
                group_key: Some(format!("bucket_{}", i)),
            });
        }

        Ok(results)
    }
}
