use crate::config::{ViewConfig, ViewKind};
use anyhow::Result;
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
    #[schema(value_type = Option<Vec<AggregationResult>>)]
    pub children: Option<Vec<AggregationResult>>,
    pub group_key: Option<String>,
}

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
pub struct ViewFilters {
    pub tech_stack: Option<String>,
    pub category: Option<String>,
    pub metric_key: Option<String>,
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
            ViewKind::Sum { .. } => return Ok(vec![]), // Should not reach here if routed correctly
        };

        let mut query = String::from(
            "SELECT file_path, value_after, tech_stack, category, change_type 
             FROM metrics 
             WHERE scan_id = ? 
             AND metric_key = ?",
        );

        if !source.analyzer_id.is_empty() {
            query.push_str(" AND analyzer_id = ?");
        }

        if !view_config.tech_stack.is_empty() {
            // Using view_config.tech_stack as category filter as per current adapter logic
            query.push_str(" AND category = ?");
        }

        // Apply dynamic filters
        if filters.tech_stack.is_some() {
            query.push_str(" AND tech_stack = ?");
        }
        if filters.category.is_some() {
            query.push_str(" AND category = ?");
        }
        if filters.metric_key.is_some() {
            query.push_str(" AND metric_key = ?");
        }

        query.push_str(" ORDER BY value_after DESC LIMIT ?");

        let mut sql_query = sqlx::query(&query).bind(scan_id).bind(&source.metric_key);

        if !source.analyzer_id.is_empty() {
            sql_query = sql_query.bind(&source.analyzer_id);
        }

        if !view_config.tech_stack.is_empty() {
            sql_query = sql_query.bind(&view_config.tech_stack);
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
                children: None,
                group_key: None,
            })
            .collect();

        // Perform grouping if requested
        if let Some(group_by_str) = &filters.group_by {
            let keys: Vec<&str> = group_by_str
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            if !keys.is_empty() {
                results = Self::group_recursive(results, &keys);
            }
        }

        Ok(results)
    }

    pub fn group_recursive(items: Vec<AggregationResult>, keys: &[&str]) -> Vec<AggregationResult> {
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
                children = Self::group_recursive(children, remaining_keys);
            }

            // Calculate aggregate value (SUM of immediate children values)
            let total_value: f64 = children.iter().map(|c| c.value).sum();

            group_nodes.push(AggregationResult {
                label: group_val.clone(),
                value: total_value,
                tech_stack: None, // Or could infer if homogeneous
                category: None,
                change_type: None,
                children: Some(children),
                group_key: Some(current_key.to_string()),
            });
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
            "SELECT file_path, value_after, tech_stack, category, change_type 
             FROM metrics 
             WHERE scan_id = ? 
             AND metric_key = ?",
        );

        if !source.analyzer_id.is_empty() {
            query.push_str(" AND analyzer_id = ?");
        }

        if !view_config.tech_stack.is_empty() {
            query.push_str(" AND category = ?");
        }

        if filters.tech_stack.is_some() {
            query.push_str(" AND tech_stack = ?");
        }
        if filters.category.is_some() {
            query.push_str(" AND category = ?");
        }
        if filters.metric_key.is_some() {
            query.push_str(" AND metric_key = ?");
        }

        // NO ORDER BY or LIMIT for Sum (unless we want to optimize?)

        let mut sql_query = sqlx::query(&query).bind(scan_id).bind(&source.metric_key);

        if !source.analyzer_id.is_empty() {
            sql_query = sql_query.bind(&source.analyzer_id);
        }

        if !view_config.tech_stack.is_empty() {
            sql_query = sql_query.bind(&view_config.tech_stack);
        }

        if let Some(ts) = &filters.tech_stack {
            sql_query = sql_query.bind(ts);
        }
        if let Some(cat) = &filters.category {
            sql_query = sql_query.bind(cat);
        }
        if let Some(mk) = &filters.metric_key {
            sql_query = sql_query.bind(mk);
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
                children: None,
                group_key: None,
            })
            .collect();

        // Perform grouping
        if let Some(group_by_str) = &filters.group_by {
            let keys: Vec<&str> = group_by_str
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            if !keys.is_empty() {
                // If grouped, return the groups (which are sums by definition of group_recursive)
                results = TopNAggregator::group_recursive(results, &keys);
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
                children: None,
                group_key: None,
            }];
        }

        Ok(results)
    }
}
