# 跨文件聚合分析框架设计

> 以一个统一的「分片提取 → 中间存储 → 全局聚合」三阶段 pipeline，替代当前独立的 `DuplicationAnalyzer` 路径，使其能承载多种跨文件分析场景。

---

## 1. 现状分析

### 1.1 当前架构：两套并行的分析器

```
┌─────────────────────────────────────────────────────────┐
│                    Scanner 扫描管线                        │
│                                                          │
│  逐文件同步处理:                                           │
│  ┌──────────────────────┐   ┌──────────────────────────┐ │
│  │ Analyzer trait        │   │ DuplicationAnalyzer trait│ │
│  │  (ScriptAnalyzer,     │   │  (ScriptDuplication-     │ │
│  │   FileCountAnalyzer,  │   │   Analyzer)              │ │
│  │   RegexAnalyzer ...)  │   │                          │ │
│  │                       │   │                          │ │
│  │ 逐文件: analyze()     │   │ 逐文件: extract_blocks() │ │
│  │ 即时写 metrics 表     │   │ 暂存 intermediate_blocks │ │
│  └───────┬───────────────┘   └──────────┬───────────────┘ │
│          │                              │                 │
│          ▼                              ▼                 │
│   metrics 表                   intermediate_blocks 表          │
│  (即时写入)                    (所有文件扫描完才处理)       │
│                                                          │
│          ┌──────────────────────────────────────┐         │
│          │ FileProcessor::finalize()                │         │
│          │  → group by block_hash               │         │
│          │  → Python 脚本内部阈值过滤            │         │
│          │  → 写 metrics (tags: duplication)    │         │
│          │  → 写 matches (聚合记录)              │         │
│          │  → 清理 intermediate_blocks           │         │
│          └──────────────────────────────────────┘         │
└─────────────────────────────────────────────────────────┘
```

### 1.2 核心差异

| 维度 | `Analyzer` (逐文件) | `DuplicationAnalyzer` (跨文件) |
|------|-------------------|-------------------------------|
| 输出格式 | `MetricEntry { value, tags }` | `ContentBlock { hash, content, loc }` |
| 存储时机 | 即时写 `metrics` 表 | 先暂存 `intermediate_blocks` 中间表 |
| 聚合阶段 | 无（逐文件即可产出） | 全部文件扫完后的 `post_process` |
| 阈值过滤 | 无 | Python 脚本内部控制 |
| 子进程 I/O | 阻塞无超时 | 30s 超时 + 线程隔离 |
| 生命周期 | 持久进程 | 持久进程 + 明确 kill/wait |

### 1.3 已解决 / 未解决的局限

1. **[已解决]** **`post_process_duplicates()` → `FileProcessor::finalize()`**：已通过 `FileProcessor` trait 统一，每个分析器拥有自己的 `finalize` 方法。聚合逻辑移至 Python 脚本内部，框架只负责数据管道。
2. **`content_blocks` 表的 schema 固定**，字段绑定 duplication（block_hash, block_content），其他分析类型需要不同字段 — 已解决：统一 `intermediate_blocks` 表
3. **DuplicationAnalyzer trait 独立**，与 Analyzer trait 没有类型关联，无法渐进式扩展 — 已解决：引入 `FileProcessor` 继承 `Analyzer`
4. **DIFF 模式下跨文件检测不完整**：只扫描变更文件，无法关联未变更文件

---

## 2. 目标：统一跨文件聚合分析框架

### 2.1 核心抽象

将当前的三阶段模式（提取 → 存储 → 聚合）通用化，使同一条管线支持多种分析类型：

```
阶段1: 逐文件提取    →   阶段2: 中间存储    →  阶段3: 全局聚合
─────────────────────────────────────────────────────────────
每个文件调用一次         写入中间表              扫描全部完成后
```

### 2.2 可支持的分析场景

| 优先级 | 分析类型 | 提取的输出 | 聚合逻辑 | 图表价值 | 当前状态 |
|-------|---------|-----------|---------|---------|---------|
| ★★★ | **重复代码检测** | 函数/方法体代码 | group by hash → count distinct file ≥ N | 找出「复制粘贴」的代码块 | ✅ 已有 |
| ★★★ | **死代码检测** | 导出定义 + 调用点 | 定义集合 − 调用集合 = 未使用导出 | 清理无效代码，降低维护成本 | 🔲 待做 |
| ★★★ | **未定义引用检测** | 符号定义 + 引用点 | 引用集合 − 定义集合 = 未定义引用 | 发现缺失导入 / 拼写错误 | 🔲 待做 |
| ★★★ | **TODO/FIXME 聚合** | 注释中的 TODO/FIXME | group by 标签 + 文件分类统计 | 跟踪技术债务分布 | 🔲 待做 |
| ★★★ | **API 使用模式** | API 调用 + 参数 | 按 API 名 count + 参数聚类 | 了解框架使用密度与异常模式 | 🔲 待做 |
| ★★★ | **配置文件交叉引用** | 配置 key 定义 + 读取点 | 定义−读取 = 孤立配置；读取−定义 = 缺失配置 | 清理过期配置、发现拼写错误 | 🔲 待做 |
| ★★☆ | **跨文件引用分析** | import/require/use 语句 | group by 引用目标 → 依赖图谱 | 了解模块间依赖关系 | 🔲 待做 |
| ★☆☆ | **相似代码检测** | AST fingerprint | 向量相似度聚类 → 输出相似对 | 定位「长得很像但还没完全一样」的代码 | 🔲 待做 |

---

## 3. 设计草案

### 3.1 新增 `FileProcessor` trait（统一 Analyzer + DuplicationAnalyzer）

```rust
/// 增强型分析器：在逐文件分析的基础上，支持「全部文件处理完成后的聚合回调」
pub trait FileProcessor: Analyzer {
    /// 提取该文件中的"中继块"（block），写入中间表
    /// 如果不需要跨文件聚合，返回空 vec
    fn extract_blocks(&self, file_path: &str, content: &str) -> Vec<IntermediateBlock>;

    /// 所有文件扫描完成后的聚合回调。
    ///
    /// 对于 Python 驱动的分析器（ScriptCrossFileAnalyzer），该方法：
    /// 1. 从 intermediate_blocks 读取所有 blocks
    /// 2. 发送 action="finalize" 给 Python 脚本（分组 + 阈值过滤由脚本完成）
    /// 3. 将脚本返回的 FinalizeMatchResult 写入 metrics + matches 表
    /// 4. 清理 intermediate_blocks
    ///
    /// 分析器的配置参数（阈值等）定义在 Python 脚本内部，不在 YAML 配置中。
    async fn finalize(&self, scan_id: i64, pool: &sqlx::Pool<sqlx::Sqlite>) -> anyhow::Result<()>;
}
```

**说明**：
- 继承自 `Analyzer` → 现有分析器不受影响（默认 `extract_blocks` 返回空，`finalize` 是 no-op）
- `ScriptAnalyzer` 可以实现 `extract_blocks` 而无 `finalize`（纯提取）
- `ScriptCrossFileAnalyzer` 的 `finalize` 将 blocks 数据发送给 Python 脚本（`action="finalize"` 协议），由脚本完成 group by / 阈值过滤，框架负责将结果写入 metrics + matches 表

### 3.2 通用中间表 `intermediate_blocks`

```sql
-- 统一中间表，替代当前的 content_blocks
CREATE TABLE intermediate_blocks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scan_id INTEGER NOT NULL,
    analyzer_id TEXT NOT NULL,        -- 谁提取的
    file_path TEXT NOT NULL,
    group_key TEXT NOT NULL,          -- 聚合 key（如 block_hash、import target、API name）
    blob_data TEXT,                   -- 文本 payload（如 block_content）
    int_data1 INTEGER,               -- 通用 int 字段（如 line_start、block_size）
    int_data2 INTEGER,               -- 通用 int 字段（如 line_end）
    int_data3 INTEGER,               -- 通用 int 字段
    str_data1 TEXT,                   -- 通用 string 字段（如 change_type）
    str_data2 TEXT,                   -- 通用 string 字段
    FOREIGN KEY(scan_id) REFERENCES scans(id)
);

CREATE INDEX idx_intermediate_scan_analyzer ON intermediate_blocks(scan_id, analyzer_id);
CREATE INDEX idx_intermediate_group ON intermediate_blocks(scan_id, analyzer_id, group_key);
```

**字段设计原则**：`group_key` 是聚合维度（≈ hash），`blob_data` 存大数据块，`int_data1-3` / `str_data1-2` 存辅助信息，避免每增加一种分析就改一次 schema。

### 3.3 扫描器中的通用管线

```rust
// 统一管线 — snapshot 和 diff 共享同一套三阶段流程。
// diff 阶段1 额外处理 old/new 两侧数据，但 extract_blocks + finalize 的接口不变。

// 阶段1: 逐文件提取
async fn scan_file_common(&mut self, scan_id: i64, path: &str, content: &str,
                          change_type: &str, analyzer_config: &...)
{
    let metrics = analyzer.analyze(path, content);
    self.save_metrics(scan_id, metrics).await?;

    if let Some(fp) = analyzer.downcast_ref::<dyn FileProcessor>() {
        let blocks = fp.extract_blocks(path, content);
        self.save_intermediate_blocks(scan_id, analyzer.id(), blocks).await?;
    }
}

// 阶段1 结束时 post_process = 阶段3
async fn finalize_all(&mut self, scan_id: i64) {
    for (id, analyzer) in &self.analyzers {
        if let Some(fp) = analyzer.downcast_ref::<dyn FileProcessor>() {
            fp.finalize(scan_id, &self.db, &config).await?;
        }
    }
    // 清理中间表
    sqlx::query("DELETE FROM intermediate_blocks WHERE scan_id = ?")
        .bind(scan_id)
        .execute(self.db.pool()).await?;
}

// scan_snapshot 和 scan_diff 各自调用 scan_file_common + finalize_all，
// scan_diff 额外对 old_content 也调用一次 scan_file_common（side=0）。
```

### 3.4 配置统一化

```yaml
# 跨文件分析器的配置 — 只定义框架级别的元信息和标签覆盖。
# 分析逻辑（分组、阈值过滤等）完全由 Python 脚本的 finalize 函数控制。
custom_cross_file_analyzers:
  duplicate_rust_fns:
    tags:
      metric: duplicate_block
      category: duplication
  unused_imports:
    mode: "cross_file"
    aggregation:
      type: "set_diff"
```

---

## 4. 分步实施路线图

### Phase 1（当前阶段）— 模块化现有 duplication 代码

**目标**：在不改变功能的前提下抽象可移植的模块

| 子任务 | 影响文件 | 说明 |
|-------|---------|------|
| 1.1 提取 `post_process_duplicates()` 到专门的模块 | `git_scanner/src/` 新建 `cross_file.rs` | 把 200 行方法从 Scanner 移到独立文件 |
| 1.2 将 `content_blocks` 表 → `intermediate_blocks` 并加 analyzer_id | `init.sql`, `lib.rs` | 通用化 schema |
| 1.3 `ScriptCrossFileAnalyzer` 改为实现 Analyzer trait 扩展 | `analyzer/src/` | 用 `extract_blocks` 方法替代独立 trait |
| 1.4 前端 ConfigPage 完善 duplication 面板 | ✅ 已做完 | — |

**Duration**: 约 2-3 天

### Phase 2 — 通用 FileProcessor trait

**目标**：定义 trait + 扫描器管线改造

| 子任务 | 影响文件 | 说明 |
|-------|---------|------|
| 2.1 定义 `FileProcessor` trait | `analyzer/src/lib.rs` | 继承 Analyzer，新增 extract_blocks + finalize |
| 2.2 Scanner 管线适配 | `git_scanner/src/lib.rs` | 统一调用 extract_blocks + finalize |
| 2.3 `ScriptCrossFileAnalyzer` 迁移 | `analyzer/src/script_cross_file.rs` | 改为 impl FileProcessor |
| 2.4 删除 `DuplicationAnalyzer` trait | 多个文件 | 清理独立路径 |

**Duration**: 约 2-3 天

### Phase 3 — 新增跨文件分析类型

**目标**：在一个统一框架下添加至少 2-3 种新分析

| 分析类型 | 提取逻辑（Python 脚本） | 聚合逻辑（finalize） |
|---------|----------------------|-------------------|
| **跨文件引用统计** | 提取 import/require/use 语句 | group by 引用目标 → 写 metrics |
| **TODO/FIXME 聚合** | 提取注释中的 TODO/FIXME | group by 标签 → 写 metrics |
| **未定义引用检测** | 提取标识符 + 定义引用 | join 比较 |

**Duration**: 约 1-2 天每种

### Phase 4 — DIFF 模式增强

**目标**：DIFF 扫描也能正确执行跨文件分析

需要让 `finalize` 能够查询**历史扫描**的 intermediate_blocks 数据，将当前变更文件的提取结果与历史数据合并分析。

```sql
-- DIFF 模式下：联合当前扫描 + 上一快照的数据
SELECT block_hash FROM intermediate_blocks WHERE scan_id = ?
UNION ALL
SELECT json_extract(tags, '$.content_hash') FROM metrics 
WHERE scan_id = ? AND json_extract(tags, '$.metric') = 'duplicate_block'
```

---

## 5. 与现有系统关系

### 5.1 完全不需改变的组件

- **聚合视图**（`aggregation.rs`）：metrics 输出格式不变，tag_filters 机制不变
- **Web Dashboard**：数据展示不变
- **API 端点**：不需新增端点（现有 /duplications 端点可通用化）
- **ConfigPage**：只需在 analyzer 编辑器中增加 `mode` 选项

### 5.2 需保持兼容的接口

- `Analyzer` trait 的所有现有实现（不做任何修改）
- `MetricEntry` / `MatchDetail` 格式
- `save_metrics()` / `save_matches()` 方法签名
- 所有 Python 脚本（保持输出格式兼容）

---

## 6. DIFF 模式下的行为界定

### 6.1 设计原则

**每次扫描只分析自己扫到的文件集合**。这不是缺陷，是设计选择。

- **Snapshot**：全仓库 → 全量跨文件分析
- **DIFF**：变更文件集合 → **仅在这个集合内**做跨文件分析

如果重复代码跨越了一个变更文件和一个未变更文件，DIFF 不会命中。这是正确的——它只对本次变更负责。

### 6.2 用户如何控制覆盖度

由用户选择 diff base commit 来控制：

```
浅覆盖:  commit-A ← commit-B      (只扫 B-C 之间的变更)
深覆盖:  commit-A ← commit-C      (把 A-C 之间所有变更一次性 DIFF)

场景：a.py 和 b.py 都有重复的 foo()
  - 如果两次分别扫描（a.py 一次、b.py 一次）→ 各只有 1 条 → 不满足脚本内定义的阈值
  - 如果一次 DIFF base 覆盖两者 → 2 条 → 命中
```

应用层面可以提供一个「最近 N 次提交的累积 DIFF」快捷操作，方便用户一键选择合理的时间窗口。

### 6.3 管线无状态

`finalize()` 中只查询当前扫描 session 的 `intermediate_blocks`，不做历史 merge。保持系统简单可预测。

---

## 7. 开放问题

1. **[已解决]** `finalize` 方法的参数 / 配置传递：`min_file_count`、`min_block_count` 等阈值已移至 Python 脚本内部。框架仅通过 `action="finalize"` 协议传递 blocks 数据，脚本的配置逻辑完全自我包含。
2. DIFF 模式下历史数据引用策略：应当自动查找上一快照，还是由用户在 UI 中指定？
3. 新分析类型的 Python 脚本输出格式：`ScriptContentBlock`（extract 阶段）和 `FinalizeMatchResult`（finalize 阶段）的 JSON schema 是否足够泛化，还是需要类型感知的序列化？
4. 性能考量：当仓库很大时（100K+ 文件），`finalize` 中的 blocks 数据通过 stdin/stdout 传输可能成为瓶颈。是否需要用 streaming 或数据库直连的方式分批次处理？

---

## 8. 前端 Dashboard 计划

### 8.1 现状：Duplications Panel

当前 duplication 的 UI 路径：

```
Dashboard "Duplications" Tab
  └─ DuplicationsPanel
       ├─ StatsRow (总块数、影响文件数、最大出现次数、分析器数)
       ├─ AnalyzerCard (按分析器分组)
       │    └─ 每个 block 显示: 首行代码、出现次数、文件数、行号范围
       │        点击 → DuplicationDetailModal
       └─ 分页 (Load More)
  └─ DuplicationDetailModal
       ├─ Block Content (代码预览)
       └─ File List (每个文件: 路径、行号范围、diff 前后值)
            └─ 文件点击 → 跳转到源文件
```

**后端 API**（`routes.rs`）：

```
GET /api/v1/projects/:name/scans/:id/duplications?page=&page_size=&min_occurrences=
  → DuplicationsResponse { duplications: DuplicationInfo[] }
```

其中 `DuplicationInfo` 的数据来源是 `metrics` 表，查询条件是 `json_extract(tags, '$.metric') = 'duplicate_block'`，按 `content_hash` 分组聚合。

**当前局限**：

1. **API 路径硬编码为 `/duplications`**，无法承载其他分析类型
2. **`DuplicationInfo` 数据结构固定**（block_content, content_hash, occurrence_count），其他分析类型需要不同字段
3. **UI 组件绑定 duplication 术语**（"Duplications"、"Duplicate Blocks"）
4. **前端 `data.ts` 中 `DuplicationAnalyzerDef` 只支持 duplication 配置字段**
5. **`FetchDuplications` 请求使用固定的 `metric=duplicate_block` 查询条件**

### 8.2 目标：通用化 Dashboard

将当前 duplication-only 的 UI 泛化为「跨文件分析面板」，每种分析类型自动渲染匹配的可视化组件。

#### 8.2.1 后端 API 设计

将 `/duplications` 端点扩展为通用跨文件分析端点：

```
# 通用端点 — 按 analysis_type 区分
GET /api/v1/projects/:name/scans/:id/cross-file/:analysis_type?page=&page_size=&min_occurrences=
  → CrossFileAnalysisResponse { items: AnalysisItem[], total: number, analysis_type: string }

# analysis_type 枚举 (由 analyzer_id 的命名约定决定)
#   "duplication"    — 重复代码检测 (当前 /duplications 逻辑)
#   "cross_ref"      — 跨文件引用统计 (新增)
#   "todo_aggregate" — TODO/FIXME 聚合 (新增)
#   "unused_imports" — 未使用导入检测 (新增)
```

或者保持单一端点，由 `tag_filters` 参数动态决定：

```
GET /api/v1/projects/:name/scans/:id/cross-file?tag_metric=duplicate_block&page=&page_size=
  → 返回匹配该 tag 类型的分析结果
```

**推荐方案**：保持现有 `/duplications` 端点不变（向后兼容），新增通用端点：

```
GET /api/v1/projects/:name/scans/:id/analysis-blocks
  → 返回此扫描所有跨文件分析类型的汇总
  { groups: { "duplication": [...], "cross_ref": [...], "todo_aggregate": [...] } }
```

每种分析类型通过 `analyzer_id` 前缀或 `tags.category` 值区分。前端根据类型自动选择渲染组件。

#### 8.2.2 数据结构抽象

```typescript
// 通用跨文件分析条目（替代固定的 DuplicationInfo）
export interface AnalysisBlockItem {
  id: string;                     // 唯一标识 (如 content_hash)
  analysis_type: string;          // "duplication" | "cross_ref" | "todo_aggregate"
  analyzer_id: string;            // 原始 analyzer_id

  // 分类/聚合字段
  group_key: string;              // 聚合 key（duplication→content_hash, cross_ref→import target, TODO→tag name）
  blob_data: string;              // 文本 payload（duplication→block_content, cross_ref→import statement, TODO→comment text）
  occurrence_count: number;       // 出现次数
  affected_files: number;         // 涉及的不同文件数

  // 分析类型特定的元数据 (按 type 不同)
  metadata: Record<string, any>;  // 灵活字段：行号、大小、语言标签等

  // 文件明细
  files: AnalysisFileInfo[];

  // 可选指标
  value_before?: number;
  value_after?: number;
}

export interface AnalysisFileInfo {
  file_path: string;
  line_start?: number;
  line_end?: number;
  value_before?: number;
  value_after?: number;
  // 不同分析类型可能有额外字段
  extra?: Record<string, any>;
}
```

#### 8.2.3 组件结构

```
Dashboard (现有 tab 结构保持不变 — Summary + Tech Stacks)
  └─ 各 tab 内的 Grid Layout
       └─ aggregation_views 中的 chart (所有 chart_type 均返回 ECharts)
            ├─ detail_view: false → 标准 ECharts (现有逻辑，无变化)
            └─ detail_view: true  → 标准 ECharts + 点击 item 打开 AnalysisDetailModal

点击 chart item → AnalysisDetailModal
  ├─ 通过现有 endpoints 获取明细（代码内容、文件列表、行号）
  └─ 用户点击文件 → 跳转到源文件 (现有 handleFileClick)
```

**核心原则**：不新增任何 chart_type。跨文件分析使用完全相同的 `top_n` / `sum` / `distribution` / `avg` / `min` / `max` 聚合函数。`analyzer_id` 中可以直接引用跨文件分析器（如 `duplicate_rust_fns`）。唯一新增的是 `detail_view: bool` 开关，控制图表 item 是否可点击展开明细。

配置示例：

```yaml
aggregation_views:
  # ── 跨文件分析视图（与普通视图无区别，只是 analyzer_id 引用跨文件分析器）

  top_duplicate_funcs:
    title: "Top 10 Duplicate Functions"
    tech_stacks: ["All"]
    chart_type: bar_horizontal
    change_type_mode: switchable
    detail_view: true                     # ← 唯一新增字段：启用明细查看
    func:
      type: top_n
      analyzer_id: ["duplicate_rust_fns_aggregated", "duplicate_python_defs_aggregated"]
      tag_filters:
        category: duplication
    group_by: ["analyzer_id", "file_path"]

  dup_by_analyzer:
    title: "Duplicates by Analyzer"
    chart_type: pie
    detail_view: true
    func:
      type: top_n
      tag_filters:
        category: duplication
    group_by: ["analyzer_id"]

  dup_block_size_distribution:
    title: "Duplication Block Size Distribution"
    chart_type: bar_col
    func:
      type: distribution
      tag_filters:
        category: duplication
      buckets: [50, 200, 500, 1000, 5000]
    # detail_view not needed — distribution 用于宏观统计，不 drill-down

  # ── 未来新增分析类型：配置方式完全一致，仅 tag_filters 和 analyzer_id 不同

  top_imports:
    title: "Top Imported Dependencies"
    tech_stacks: ["All"]
    chart_type: bar_horizontal
    detail_view: true
    func:
      type: top_n
      tag_filters:
        category: cross_reference
    group_by: ["file_path"]
```

#### 8.2.4 各分析场景的 metrics 模型与图表映射

所有跨文件分析器的 `finalize()` 向 metrics 表写入的条目格式相同，仅 `tags.category` 和 tags 中的标识字段不同。下面逐一分析每个场景的 metrics 模型和推荐的图表配置。

---
**（1）重复代码检测** (category: `duplication`)

```
metrics 行模型:
  analyzer_id: "xxx_aggregated"
  tags: { metric: "duplicate_block", category: "duplication", content_hash: "...", block_size: "42", occurrence_count: "5" }
  value: 1.0
  file_path: "src/main.rs"
  scope: "xxx:15-42"

analyzer_id 建议: duplicate_rust_fns_aggregated, duplicate_python_defs_aggregated
```

| 配置目标 | aggregation | group_by | chart_type | detail_view |
|---------|-------------|---------|-----------|-------------|
| Top 重复代码块 | top_n | `analyzer_id, file_path` | bar_horizontal | ✅ |
| 各分析器分布 | top_n / sum | `analyzer_id` | pie | ✅ |
| 块大小分布 | distribution | — | bar_col | ❌ |

聚合结果是 `content_hash`（相同的代码内容）。detail_view 通过 `content_hash` 查询文件列表和代码内容。

---
**（2）死代码检测** (category: `dead_code`)

```
metrics 行模型:
  analyzer_id: "dead_code_aggregated"
  tags: { metric: "dead_symbol", category: "dead_code", symbol_name: "unused_function", symbol_type: "function" }
  value: 1.0                     # 一个死符号 = value=1
  file_path: "src/old.rs"
  scope: "dead_code:42-56"

analyzer_id 建议: dead_code_python, dead_code_rust（按语言实现不同的 Python 脚本）
```

`finalize` 逻辑：从所有文件中提取「定义集合」和「引用集合」，做集合差集。

| 配置目标 | aggregation | group_by | chart_type | detail_view |
|---------|-------------|---------|-----------|-------------|
| 死代码最多的文件 | top_n | `file_path` | bar_horizontal | ✅ 列出该文件所有死符号 + 位置 |
| 按符号类型分布 | sum | `symbol_type` | pie | ✅ 每种类型下死符号列表 |
| 死代码总量趋势 | sum | — | gauge / card | ❌ |

默认聚合单位是 `file_path`（每个文件有多少死符号）。detail_view 查询该文件下 category=dead_code 的所有条目。

---
**（3）未定义引用检测** (category: `undefined_ref`)

```
metrics 行模型:
  analyzer_id: "undefined_ref_aggregated"
  tags: { metric: "undefined_ref", category: "undefined_ref", symbol_name: "missingFunc", symbol_type: "function" }
  value: 1.0                     # 一次引用 = value=1
  file_path: "src/broken.ts"
  scope: "undefined_ref:42"

analyzer_id 建议: undefined_ref_py, undefined_ref_ts（按语言实现）
```

`finalize` 逻辑：收集所有被引用的符号 → 减去所有定义的符号 → 剩余 = 未定义引用。

| 配置目标 | aggregation | group_by | chart_type | detail_view |
|---------|-------------|---------|-----------|-------------|
| 最常缺失的符号 | top_n | `symbol_name` | bar_horizontal | ✅ 列出引用该符号的所有文件位置 |
| 未定义引用最多的文件 | top_n | `file_path` | bar_horizontal | ✅ 列出该文件所有未定义引用 |
| 按符号类型分布 | sum | `symbol_type` | pie | ✅ |

---
**（4）TODO/FIXME 聚合** (category: `todo_aggregation`)

```
metrics 行模型:
  analyzer_id: "todo_aggregate_aggregated"
  tags: { metric: "todo_count", category: "todo_aggregation", todo_tag: "TODO" }
  value: 3.0                     # 该文件中有 3 个 TODO
  file_path: "src/legacy.rs"
  scope: "todo:15,42,73"         # 行号列表

analyzer_id 建议: todo_aggregate（一个通用脚本即可）
```

`finalize` 逻辑：提取所有 TODO/FIXME/HACK/XXX/BUG 注释 → 按文件 + 标签类型分组计数 → 每行写入一条。

| 配置目标 | aggregation | group_by | chart_type | detail_view |
|---------|-------------|---------|-----------|-------------|
| TODO 最多的文件 | top_n | `file_path` | bar_horizontal | ✅ 列出该文件所有 TODO 内容 + 行号 |
| TODO 标签分布 | sum | `todo_tag` | pie | ✅ 每个标签下文件列表 |
| 按技术栈分布 | sum | `tech_stack` | pie | ✅ 各栈 TODO 明细 |

detail_view 通过 `file_path` + `category=todo_aggregation` 拿到该文件下所有 TODO 条目的 scope 和内容。

---
**（5）API 使用模式** (category: `api_usage`)

```
metrics 行模型:
  analyzer_id: "api_usage_aggregated"
  tags: { metric: "api_call_count", category: "api_usage", api_name: "client.query", api_module: "database" }
  value: 5.0                     # 该文件中调用 5 次
  file_path: "src/service.rs"
  scope: "api_usage:15,32,47,51,68"

analyzer_id 建议: api_usage（通用脚本，配置 regex 或 AST 识别 API 调用）
```

`finalize` 逻辑：提取 API 调用表达式 → 按 api_name + file_path 分组计数。

| 配置目标 | aggregation | group_by | chart_type | detail_view |
|---------|-------------|---------|-----------|-------------|
| 最常用的 API | top_n | `api_name` | bar_row | ✅ 调用位置列表 + 参数 |
| 各模块 API 使用量 | sum | `api_module` | pie | ✅ 该模块下所有调用 |
| API 调用最多的文件 | top_n | `file_path` | bar_horizontal | ✅ 列出该文件所有 API 调用 |

---
**（6）配置文件交叉引用** (category: `config_ref`)

```
metrics 行模型（3 种状态）:
  ─ used:        tags: { ..., config_key: "database.host", ref_status: "used" },    value: ref_count, file_path: "config/default.yaml"
  ─ orphaned:    tags: { ..., config_key: "database.old",  ref_status: "orphaned" }, value: 0,         file_path: "config/default.yaml"
  ─ missing:     tags: { ..., config_key: "database.x",    ref_status: "missing" },  value: ref_count, file_path: "src/db.ts"

analyzer_id 建议: config_ref（通用脚本）
```

`finalize` 逻辑：
1. 收集所有配置 key 定义 → 集合 D（来自配置文件的 `extract_blocks`）
2. 收集所有配置 key 读取 → 集合 R（来自代码文件的 `extract_blocks`）
3. D ∩ R = used；D − R = orphaned；R − D = missing

三种聚合价值极高——同一张图就能看到「定义的配置」哪些在用、哪些废弃、哪些代码引用缺失了定义：

| 配置目标 | aggregation | group_by | chart_type | detail_view |
|---------|-------------|---------|-----------|-------------|
| 被引用最多的配置 | top_n | `config_key` (ref_status=used) | bar_horizontal | ✅ 引用文件列表 + 行号 |
| 孤立配置（定义但未读） | top_n | `config_key` (ref_status=orphaned) | table | ✅ 定义位置 + 建议删除 |
| 缺失配置（引用但未定义） | top_n | `config_key` (ref_status=missing) | bar_horizontal | ✅ 引用来源文件 + 行号 |
| 三种状态汇总 | sum | `ref_status` | pie | ❌ 宏观，无需 drill-down |

---
**（7）跨文件引用分析** (category: `cross_reference`)

```
metrics 行模型:
  analyzer_id: "cross_ref_aggregated"
  tags: { metric: "cross_ref_count", category: "cross_reference", import_target: "lodash/debounce" }
  value: 1.0                     # 该文件引用 1 次
  file_path: "src/util/helpers.ts"
  scope: "cross_ref:15"
```

| 配置目标 | aggregation | group_by | chart_type | detail_view |
|---------|-------------|---------|-----------|-------------|
| 最常被引用的外部模块 | top_n | `import_target` | bar_horizontal | ✅ 引用来源文件列表 |
| 各文件引用外部模块数 | top_n | `file_path` | bar_horizontal | ✅ 每个模块位置 |
| 按技术栈分布 | sum | `tech_stack` | pie | ✅ |

---
**汇总**：全部 7 类分析场景（以及未来可能新增的场景）共用同一套 aggregation func 集合——`top_n`、`sum`、`distribution`。不需要新增任何 aggregation func。唯一的前端新增概念是 `detail_view: bool`。

#### 8.2.5 `detail_view` 设计

```yaml
# AggregationView 新增字段
detail_view: bool   # 默认 false。当为 true 时，图表中的 item 可点击弹出明细
```

**聚合结果到明细的映射**：

当用户点击一个聚合条目时，前端已有以下信息：

```
AggregationResult {
  analyzer_id: "duplicate_rust_fns_aggregated",  // 写入 metrics 时的 analyzer_id
  value: 5,                                        // 出现次数
  tags: { content_hash: "abc123", ... },           // 聚合维度的 tag 信息
  file_path: "src/main.rs",                        // 文件路径（group_by 的维度）
  group: [...]                                     // 子条目（递归分组时）
}
```

通过这些信息，可以构造明细查询：

| 信息源 | 查询端点 | 参数 |
|-------|---------|------|
| `analyzer_id` | `GET /matches` | `analyzer_id` + `scan_id` |
| `tags.content_hash` | `GET /duplications?content_hash=` | 重复代码的 block 明细 |
| `tags.import_target` | `GET /matches` | 引用统计的引用来源明细 |
| `file_path` + `scope` | `GET /matches` | 具体文件内匹配位置 |

明细弹窗 `AnalysisDetailModal` 从 `DuplicationDetailModal` 泛化而来，根据 `analyzer_id` 前缀或 `tags.category` 自动判断展示内容类型：

```
AnalysisDetailModal
  ├─ category=duplication  → 展示代码内容 + 出现文件列表（复用现有 DuplicationDetailModal UI）
  ├─ category=cross_reference → 展示引用目标 + 来源文件表格
  ├─ category=todo_aggregation → 展示 TODO 注释 + 文件位置列表
  └─ category=undefined_ref → 展示缺失符号 + 引用位置列表
```

组件职责分解：

| 组件 | 职责 | 对应现有组件 |
|------|------|-------------|
| `ChartRenderer` | ECharts wrapper | ✅ 已有 |
| `(Dashboard.tsx)` | 所有 chart_type 走同一渲染管线；检测 `detail_view` 后在 item 上绑定 click handler | 扩展现有 chart 渲染路由 |
| `AnalysisDetailModal` | 通用明细弹窗，根据 `tags.category` 分发子组件 | `DuplicationDetailModal` (泛化) |
| `DuplicationsPanel` | **移除** — 功能由 aggregation_views + detail_view 替代 | 待删除 |
| `DuplicationDetailModal` | **重命名** 为 `AnalysisDetailModal` | 待重命名 |

### 8.3 数据流

```text
后端 finalize() 写入 metrics 表
  │  tags.metric = "duplicate_block" / "cross_ref_count" / "todo_count"
  │  tags.category = "duplication" / "cross_reference" / "todo_aggregation"
  │  （与普通 analyzer 写入 metrics 的流程完全一致，无特殊路径）
  ▼
前端 GET /api/v1/projects/:name/scans/:id/views/:view_id
  │  （复用现有 aggregation 端点，完全相同的处理逻辑）
  │  view.detail_view → 判断是否绑定 click handler
  ▼
Dashboard.tsx 渲染
  ├─ detail_view: false → 标准 ChartRenderer (ECharts)，无点击交互
  └─ detail_view: true  → 标准 ChartRenderer (ECharts) + item click handler
      └─ 用户点击 → AnalysisDetailModal
          └─ 通过已有端点查询明细数据
```

关键原则：**数据路径无差异，仅交互层有区分**。

### 8.4 Diff 模式的兼容性

diff 模式对跨文件分析无特殊影响。已有 `value_before`/`value_after` 和 `change_type_mode` 完全覆盖：

```yaml
aggregation_views:
  top_duplicate_diff:
    title: "Duplicate Blocks (A/M/D)"
    chart_type: bar_horizontal
    detail_view: true
    change_type_mode: switchable          # A/M/D 切换，已有功能
    func:
      type: top_n
      tag_filters:
        category: duplication
    group_by: ["analyzer_id", "file_path"]
```

聚合引擎已经正确处理 `value_before`/`value_after`。跨文件分析器在 `finalize()` 中写入的 value_before/value_after 与普通 analyzer 完全一致。`detail_view` 不影响聚合语义。

### 8.5 后端无变更

| 组件 | 变更 | 原因 |
|------|------|------|
| API 端点 | 无变更 | views、matches、duplications 端点已覆盖全部需求 |
| 聚合引擎 | 无变更 | top_n / sum / distribution 已覆盖全部场景 |
| 配置结构 | 新增 `detail_view: bool` 字段 | 前端交互开关，后端仅透传 |
| ConfigPage | analyzer_id 下拉中显示跨文件分析器 | 已有逻辑已列出所有 analyzer_id，需确保 `*_aggregated` 后缀的在列表中 |

### 8.6 实现路线图（Dashboard）

**Phase A — 新增 `detail_view` 字段 + 通用 AnalysisDetailModal**（1-2 天）

| 子任务 | 影响文件 | 说明 |
|-------|---------|------|
| A.1 core 配置类型新增 `detail_view: bool` 字段 | `crates/core/src/lib.rs` AggregationView | 新增可选字段，默认 false |
| A.2 server config 新增对应字段透传 | `crates/server/src/config.rs` | `ServerAggregationView` 新增 `detail_view` |
| A.3 前端 data.ts 类型更新 | `data.ts` AggregationView | 新增 `detail_view?: boolean` |
| A.4 重命名 `DuplicationDetailModal` → `AnalysisDetailModal` | `dashboard/AnalysisDetailModal.tsx` | 组件重命名，props 泛化，根据 tags.category 分发内容 |
| A.5 ConfigPage 中 detail_view 开关 | `ConfigPage.tsx` | 聚合视图编辑器增加 checkbox |
| A.6 ConfigPage analyzer_id 下拉包含 `*_aggregated` 分析器 | `ConfigPage.tsx` | 确保用户可选跨文件分析器 |

**Phase B — Dashboard 集成**（1 天）

| 子任务 | 影响文件 | 说明 |
|-------|---------|------|
| B.1 Dashboard.tsx 检测 `detail_view` 并绑定 item click | `Dashboard.tsx` | chart item 点击 → 收集 tags + analyzer_id → 打开 AnalysisDetailModal |
| B.2 移除独立的 Duplications tab 和 DuplicationsPanel | `Dashboard.tsx`, `DuplicationsPanel.tsx` | 功能完全由 aggregation_views 替代 |
| B.3 默认 config 模板中将 duplication 视图改为 `detail_view: true` | `crates/core/src/lib.rs` (generate_template) | 确保现有用户升级后可见明细 |

**Phase C — 新增分析类型的 Python 脚本 + AnalysisDetailModal 明细子组件**（与后端 Phase 3 并行，1-2 天每种）

| 分析类型 | category 常量 | Python 脚本 | finalize 核心逻辑 | AnalysisDetailModal 新增子组件 |
|---------|-------------|------------|-----------------|----------------------------|
| 死代码检测 | `dead_code` | `dead_code_rust.py`, `dead_code_python.py` | 定义集合 − 引用集合 = 未使用导出 | 死符号列表 + 位置 + symbol_type 标签 |
| 未定义引用检测 | `undefined_ref` | `undefined_ref_ts.py` 等 | 引用集合 − 定义集合 = 未定义引用 | 缺失符号列表 + 引用位置 + 建议修复 |
| TODO/FIXME 聚合 | `todo_aggregation` | `todo_aggregate.py` | 提取注释 → 按标签 + 文件分组计数 | TODO 注释全文 + 行号 + 标签分类 |
| API 使用模式 | `api_usage` | `api_usage.py` | 提取 API 调用 → 按 API 名 + 文件分组 | 调用位置 + 参数列表 + 文件路径 |
| 配置文件交叉引用 | `config_ref` | `config_ref.py` | 定义−读取=孤立; 读取−定义=缺失 | 引用文件列表 / 定义位置 / 缺失标记 |
| 跨文件引用统计 | `cross_reference` | `cross_ref.py` | 提取 import → group by 目标 | 引用来源文件列表 + 行号 |

### 8.7 向后兼容策略

1. **配置兼容**：`detail_view` 默认为 false → 现有 config 不受影响
2. **API 兼容**：零变更，视图端点原样返回 `detail_view` 字段
3. **UI 兼容**：`DuplicationsPanel` tab 在过渡期可保留（Phase B 中移除）；新视图与旧面板并行
4. **迁移路径**：用户只需在 ConfigPage 中给已有 duplication 视图勾选 `detail_view: true`，或添加新的跨文件分析视图并选择 analyzer_id

### 8.8 国际化

`AnalysisDetailModal` 从 `DuplicationDetailModal` 重命名而来，已有 i18n key 保持：

```typescript
// 复用现有 key（duplications namespace）
t('duplications.blockContent', 'Block Content')
t('duplications.files', 'Files')
t('duplications.count', '{{count}} duplicate blocks')

// 新增 category 特定的 key
t('duplications.crossRefDetail', 'Import References')
t('duplications.todoDetail', 'TODO/FIXME Items')
t('duplications.undefinedRefDetail', 'Undefined References')
```

新增的 `detail_view` 标签在现有视图编辑器中作为 checkbox 展示，不需要新增 i18n namespace。

### 8.9 实现优先级

根据实用价值与实现难度，建议按以下顺序实现：

| 优先级 | 分析类型 | 理由 | 依赖条件 |
|-------|---------|------|---------|
| P0 | **重复代码检测** | ✅ 已有 | — |
| P1 | **TODO/FIXME 聚合** | 脚本逻辑最简单（纯 regex 提取注释），立即看到价值 | `detail_view` 基础框架就绪 |
| P1 | **死代码检测** | 实用价值高，finalize 逻辑为简单集合差集 | Python 脚本需区分定义 vs 引用 |
| P1 | **未定义引用检测** | 与死代码检测共用「定义/引用提取」基础设施，可并行开发 | 同上 |
| P2 | **API 使用模式** | 需要领域知识来定义「哪些调用算 API」，需更多的配置灵活性 | 可能需要支持用户配置 API 识别规则 |
| P2 | **配置文件交叉引用** | 需要区分「配置定义」和「配置读取」两种文件模式，finalize 逻辑略复杂 | Python 脚本需同时处理 config 文件和代码文件 |
| P3 | **跨文件引用分析** | 与已有 duplication 的跨文件差异检测重叠，价值密度相对较低 | — |

**实施建议**：

- P1 的三个分析类型（TODO、死代码、未定义引用）可以**并行开发**：各自独立的 Python 脚本 + 独立的 AnalysisDetailModal 子组件
- 每个新分析器的首个 config 模板示例应由开发者在 `crates/core/src/lib.rs` 的 `generate_template()` 中加入，确保用户升级后立即可见
- P2 的分析器可能需要更复杂的 Python 脚本框架（如配置文件解析、AST 分析），建议在 P1 全部完成后评估

---
