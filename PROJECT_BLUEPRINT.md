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

## 5. 实施阶段规划 (Implementation Phases)

### 品质前提：所有代码必须符合 Rust 语言规范，并遵循 Rust 的最佳实践。并且有相应测试确保代码质量。

### 阶段一：骨架与数据库 (Scaffold & DB)

* 初始化 Rust 项目。

* 实现 SQLite 数据库连接与 Schema 迁移。

* 实现基础 CLI (`init`, `scan`)。

* **实现 Git 读取模块**: 封装 `git2`，实现 `read_tree` (Snapshot) 和 `diff_tree` (Diff) 两种模式。

* 实现第一个基础分析器 (如 `LineCounter`) 并将数据存入 `metrics` 表。

### 阶段二：聚合引擎与 API (Aggregation & Server)

* 引入 `Axum` 启动 Web 服务。

* 实现**聚合引擎**：根据请求的 View 配置，动态生成 SQL 或在内存中处理数据。

* 提供 API: `GET /api/v1/scan/:id/view/:view_id`。

### 阶段三：前端集成 (Frontend Integration)

* [x] 搭建 React + Vite 环境。

* [x] 创建通用图表组件 `ChartRenderer` (基于 ECharts)。

* [x] 使用 `rust-embed` 将前端产物打包进 Rust 二进制文件。

* [x] 实现 `codeprism serve` 命令。

### 阶段四：高级功能 (Advanced Features)

* **可扩展性**: 完善 `Analyzer` Trait 接口，支持添加更多特定技术栈的分析器 (XML, JSON, AST-based)。

