# CodePrism 项目蓝图 (Project Blueprint)

本文档是 **CodePrism** 项目的单一事实来源 (Single Source of Truth)。所有的架构决策、数据库变更和开发路线图必须以此为准。

## 1. 项目愿景与核心定义

CodePrism 是一个基于 Git 的高性能、可扩展的代码资产分析工具。

* **核心理念**: "Everything is Metric" (一切皆指标)。

* **部署形态**: Single Binary (Rust 单体应用) + Embedded UI (内嵌 React 前端)。

* **核心架构模式**: **Server-Driven UI (服务端驱动 UI)**。

  * 后端负责复杂的聚合统计逻辑，通过配置文件驱动。

  * 前端只负责渲染通用的图表组件，不包含复杂的业务统计逻辑。

## 2. 技术栈约束 (Tech Stack)

### 后端 (Core)

* **语言**: Rust (Edition 2021)

* **Web 框架**: `axum` (高性能异步框架)

* **CLI 交互**: `clap` (命令行参数解析)

* **数据库**: `sqlx` + `sqlite` (嵌入式数据库，强类型 SQL 检查)

* **Git 操作**: `git2` (libgit2 bindings) 或 `gix` (Pure Rust)。**关键策略**: 尽量直接读取 Git Object Database (ODB) 中的 Tree 和 Blob，避免物理 `git checkout` 破坏用户工作区。

* **静态资源嵌入**: `rust-embed`

* **错误处理**: `anyhow`, `thiserror`

### 前端 (Presentation)

* **框架**: React + TypeScript + Vite

* **图表库**: Apache ECharts (处理大数据量渲染能力强)

* **组件库**: Shadcn/ui (基于 Radix UI，轻量可定制)

* **数据通信**: RESTful API

### 2.1 扫描策略与 Git 交互 (Scanning Strategy)

系统支持两种核心模式，对应三种用户场景。所有的文件变更类型简化为：**A (Add/新增)**, **M (Modify/变更)**, **D (Delete/删除)**。

**模式 A: 快照模式 (Snapshot Mode)**

* **场景 1: 单点全量扫描**

  * **输入**: 某个 Commit Hash (或 Branch/Tag)。

  * **逻辑**: 解析该 Commit 对应的 Tree Object，递归遍历所有文件。

  * **状态**: 所有存在的文件标记为 **A (视为当前快照的新增)** 或 **N (Normal/无变更状态)**，取决于是否与前一次扫描关联。默认作为基准快照时，可视为无变更记录，仅记录 Metric。

**模式 B: 差异模式 (Diff Mode)**

* **场景 2: 两个 Commits 对比**

  * **输入**: `Commit Old` vs `Commit New`。

  * **逻辑**: 使用 `git diff tree_old tree_new` 计算差异。

  * **处理**:

    * **A (新增)**: 读取 `New` 中的 Blob 进行分析，记录 Metrics。

    * **M (变更)**: 读取 `New` 中的 Blob 进行分析，记录 Metrics。

    * **D (删除)**: 记录变更状态，不产生内容 Metrics (LOC=0)。

* **场景 3: 两个 Branches 对比**

  * **输入**: `Branch A` vs `Branch B`。

  * **逻辑**: 解析两个 Branch Tip 指向的 Commit Hash，执行同上的“Commit 对比”逻辑。

## 3. 核心数据模型 (Database Schema)

数据库 Schema 是系统的基石。所有分析数据必须扁平化为原子指标。

```sql
-- 1. 项目元数据表
CREATE TABLE projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,           -- 项目名称
    repo_path TEXT NOT NULL,      -- 本地仓库路径或远程 URL
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 2. 扫描记录表 (每次执行是一次 Scan)
CREATE TABLE scans (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL,
    commit_hash TEXT NOT NULL,    -- 当前扫描的目标 Commit Hash
    branch_name TEXT,             -- 分支名 (可选)
    scan_mode TEXT NOT NULL,      -- 'SNAPSHOT' 或 'DIFF'
    base_commit_hash TEXT,        -- 如果是 DIFF 模式，记录对比的基准 Commit
    scan_time DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(project_id) REFERENCES projects(id)
);

-- 3. 通用指标表 (Unified Metrics & File Changes)
-- 合并了文件变更状态和代码分析指标
CREATE TABLE metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scan_id INTEGER NOT NULL,
    file_path TEXT NOT NULL,      -- 文件路径
    
    -- Git 变更元数据
    change_type TEXT,             -- 'A', 'M', 'D'. Snapshot 模式下可为 NULL
    old_file_path TEXT,           -- 重命名场景的前身路径
    tech_stack TEXT,              -- 技术栈归类
    
    -- 指标数据
    analyzer_id TEXT NOT NULL,
    metric_key TEXT NOT NULL,
    category TEXT,
    value_before REAL,            -- 变更前的值 (Diff Only)
    value_after REAL,             -- 变更后的值 (Snapshot or Diff "new")
    scope TEXT,
    
    FOREIGN KEY(scan_id) REFERENCES scans(id)
);


-- 索引优化
CREATE INDEX idx_metrics_scan ON metrics(scan_id);
CREATE INDEX idx_metrics_lookup ON metrics(analyzer_id, metric_key);
CREATE INDEX idx_metrics_file_prop ON metrics(tech_stack, change_type);
```

### 3.1 技术栈分类与选择性分析 (Tech Stack Classification & Selective Analysis)

为了优化性能并提供更精准的洞察，系统需实现基于文件后缀的**技术栈分类** (Tech Stack Classification) 和配置驱动的**选择性分析** (Selective Analysis)。

**核心逻辑：**

1.  **配置驱动**: 在 `codeprism.yaml` 中定义技术栈规则。
2.  **分类 (Classification)**: 扫描时，根据文件后缀 (Extension) 将文件归类到一个或多个 `TechStack`。
3.  **选择性执行 (Selective Execution)**: 每个 `TechStack` 明确列出需要执行的 `Analyzer` 列表。扫描器仅对该文件执行匹配的分析器。

**配置示例 (`codeprism.yaml`)**:

```yaml
tech_stacks:
  - name: "Rust"
    extensions: ["rs", "toml"]
    analyzers: ["loc_counter", "rust_ast_analyzer"]

  - name: "Java"
    extensions: ["java", "jsp"]
    analyzers: ["loc_counter", "java_bytecode_analyzer"]
    
  - name: "Web"
    extensions: ["js", "ts", "html", "css"]
    analyzers: ["loc_counter", "web_best_practices"]

default_analyzers: ["loc_counter"]  # 默认对所有文件执行的分析器
```

**数据流变更：**
*   **扫描前**: 加载 `codeprism.yaml` 构建 `Extension -> Vec<Analyzer>` 的映射表。
*   **扫描中**: 遇到文件 -> 获取后缀 -> 查表 -> 执行指定 `Analyzer` -> 存储 Metrics。


## 4. 后端架构：聚合管道 (Aggregation Pipeline)

后端**不应**直接返回数据库原始行，而是返回“视图 (Views)”。

1. **配置驱动**: 通过 `codeprism.yaml` (或内置默认配置) 定义 `Views`。

2. **聚合策略 (Aggregation Strategies)**:
   后端必须实现以下核心聚合逻辑，供 API 调用：

   * **`sum_by_category`**: 按 category 分组求和。

     * *场景*: 语言分布饼图 (Java: 5000行, Rust: 2000行)。

   * **`bucket_distribution`**: 按数值区间分桶 (Histogram)。

     * *场景*: 文件大小分布直方图 (0-100行: 50个, 100-500行: 20个)。

   * **`top_n`**: 排序取前 N 个对象。

     * *场景*: 复杂度最高的 10 个文件列表。

3. **API 响应**: 返回专门为图表设计的 JSON 结构 (包含 `labels`, `datasets`, `chart_type`)，前端无需进行二次计算。

## 5. 架构审查与改进计划

基于项目实施后的实际运行和性能评估，以下是针对数据模型和分析数据收集方式的改进建议。这些改进旨在优化性能、可扩展性和维护性，同时保持与蓝图核心原则的兼容性。

### 5.1 数据模型改进

**当前评估**：
- metrics 表扁平化设计符合“一切皆指标”理念，但字段过多导致冗余和查询复杂性。
- 缺乏时间序列支持，影响历史趋势分析。
- 索引覆盖基本查询，但跨项目/时间聚合不足。

**改进计划**：
- **表结构分离**：将 metrics 表拆分为 file_changes（变更元数据：scan_id, file_path, change_type, tech_stack）和 metric_values（指标数据：file_change_id, analyzer_id, metric_key, value）。减少冗余，提高查询性能。
- **时间序列支持**：在 scans 和 metrics 表中添加 timestamps 字段，支持历史趋势视图（如 ECharts 时间轴图）。评估引入 time_series 表存储聚合快照。
- **索引增强**：添加复合索引，如 (project_id, scan_time, tech_stack) 和 (metric_key, value) 用于范围查询。考虑按项目或时间分区表以优化大型数据集。
- **数据验证**：在 core crate 中加强 schema 验证，确保 value_before/value_after 的逻辑一致性。
- **长期扩展**：评估迁移到 PostgreSQL 以支持多租户和复杂并发。

### 5.2 分析数据收集方式改进

**当前评估**：
- 异步解耦和配置驱动的选择性分析性能良好，但串行处理和缺乏批处理导致效率低下。
- 无缓存机制，重复扫描效率低；错误恢复不足。

**改进计划**：
- **并行化分析**：引入 tokio::spawn 或 rayon 并行处理文件分析，使用工作池限制并发。评估 futures::stream::buffered 批处理通道。
- **批写入优化**：在 git_scanner 中收集一批 metrics，使用 sqlx 事务批量插入。添加缓冲区平滑写入峰值。
- **增量扫描与缓存**：添加 commit_hash 缓存表，记录已分析文件哈希。差异模式仅分析变更文件；快照模式跳过未变更文件。
- **错误处理增强**：在 analyzer crate 中添加 Result 链式处理，支持部分失败和重试机制。
- **性能监控**：集成指标收集（如扫描时间、文件数），存储到 metrics 表。使用 profiling 工具识别瓶颈。
- **扩展性**：标准化分析器接口，支持热加载。评估消息队列处理分布式扫描。

### 5.3 整体架构改进时间表

- **短期（1-2 个月）**：优先数据模型分离和批写入优化。目标：大型仓库扫描时间减少 30%。
- **中期（3-6 个月）**：引入并行分析、缓存和 GraphQL API 支持复杂查询。
- **长期（6+ 个月）**：支持云部署、远程 Git URL、多项目并发和用户认证。
- **风险与验证**：变更需迁移脚本；并行化需测试竞态条件。使用现有测试和基准测试验证改进。

