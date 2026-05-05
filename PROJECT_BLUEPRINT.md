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

### 5.4 技术负债改进清单（供 AI 代理持续跟进）

以下条目基于对当前代码实现的架构审查，定义为**明确的技术负债**，后续 AI 代理应优先围绕这些条目拆解 issue、生成计划并逐项收敛。

#### C. 配置读写服务化，削减胖路由

**现状问题**：
- YAML 读取、解析、修改、写回、内存配置重建逻辑散落在多个 route handler 中。
- 这使 HTTP 层同时承担协议、持久化、领域转换与状态重建职责。

**目标原则**：
- 路由层只负责请求/响应边界；配置更新流程必须下沉到集中服务。

**负债改进要求**：
- 抽出 `ConfigRepository` / `ConfigService`，统一处理配置读写与 reload。
- 抽出 UI 配置投影（如 `AppConfig`）的构建逻辑，避免在多个 handler 中重复重建。
- 为配置写入增加并发保护或版本控制，避免并发请求导致覆盖写。

#### D. 多项目 analyzer 注册隔离

**现状问题**：
- 当前 analyzer 定义会从 root 和所有项目配置中合并到全局注册表，同名 analyzer 存在“后者覆盖前者”的风险。
- 这会在多项目模式下造成配置串扰。

**目标原则**：
- 项目级 analyzer 定义必须具备清晰边界，禁止不同项目互相污染运行时注册表。

**负债改进要求**：
- analyzer registry 至少按 `project_name` 隔离。
- 将“analyzer 编译/加载”与“项目运行时选择”拆分为不同层次。
- 后续所有多项目能力都必须以“配置隔离先成立”为前提。

#### E. 写库链路批处理与性能治理

**现状问题**：
- 当前扫描链路存在逐文件、逐指标写库倾向，缺乏稳定的事务批处理和批量插入策略。
- 该实现对大仓库和高指标密度场景扩展性不足。

**目标原则**：
- 写库路径必须面向大规模数据设计，优先保证吞吐、事务边界清晰和资源占用可控。

**负债改进要求**：
- 引入批量写入与事务封装，减少单条 INSERT 放大效应。
- 评估独立 metrics writer / buffered flush 机制。
- 为扫描、分析、写库分别建立耗时与吞吐监控指标，避免性能问题只能靠体感判断。

#### F. 聚合逻辑下推与查询边界重构

**现状问题**：
- 当前部分聚合逻辑仍偏向“先取原始数据，再在 Rust 内存中做分组/排序/聚合”。
- 数据规模上升后，API 延迟与内存占用都会恶化。

**目标原则**：
- 能在 SQL 层完成的聚合，原则上不要回收到应用层完成。

**负债改进要求**：
- 将 `GROUP BY / SUM / AVG / MIN / MAX / ORDER BY / LIMIT` 等通用聚合尽量下推数据库。
- Rust 层仅保留数据库不擅长的视图结构重组逻辑。
- 为关键聚合路径增加基准测试，避免重构后只换结构不提升性能。

#### G. 前后端契约收紧与类型生成

**现状问题**：
- 前端当前对后端响应结构有较多手写推断与宽松兜底，契约变更不易第一时间暴露。
- 错误请求在前端部分路径中会被“空数据”吞掉，影响真实诊断。

**目标原则**：
- 契约必须单源定义、强类型生成、错误显式暴露。

**负债改进要求**：
- 以后端 OpenAPI 为基础生成前端类型与客户端。
- 前端必须明确区分 `loading / empty / failed / not_ready`，禁止统一回退为空列表。
- 配置模型和视图模型不允许长期维持“后端一套、前端手写一套”的漂移状态。

#### H. 前端状态管理拆分与 Dashboard 解巨石化

**现状问题**：
- `Dashboard` 组件同时承载配置加载、项目切换、run 获取、视图筛选、图表数据请求、图表 option 生成与页面渲染。
- `AppContext` 同时管理导航、主题、业务筛选、配置刷新信号，边界混杂。

**目标原则**：
- 页面编排、远程数据、图表生成、本地 UI 状态必须分层管理，避免核心页面持续膨胀。

**负债改进要求**：
- 拆分 `Dashboard` 为查询 hooks、视图卡片组件、图表 option 纯函数模块。
- `AppContext` 至少拆分为 shell/UI 状态与 dashboard 查询状态。
- 推荐引入查询缓存层（如 TanStack Query），替代手写 `useEffect + fetch + isActive` 状态机。

#### I. 测试基座修复与质量门禁

**现状问题**：
- 当前前端测试存在 provider / i18n 环境不完整问题，导致测试文件存在但不能形成有效约束。
- lint 也已暴露 `any`、hook 依赖和 refresh 结构问题。

**目标原则**：
- 测试必须先可运行，再谈覆盖率；CI 必须能对关键层做真实约束。

**负债改进要求**：
- 建立统一的前端测试 providers 封装。
- 为 dashboard、execute、config 三条主流程建立最小可用行为测试。
- 将 `cargo fmt --check`、`cargo clippy`、前端 lint、前端测试纳入标准质量门禁。
- 对关键后端聚合、扫描状态流转、配置兼容逻辑补足集成测试。

### 5.5 技术负债治理优先级

- **P0（立即治理）**：扫描任务 job/status 化、扫描错误传播、前端错误状态显式化、测试基座修复。
- **P1（近期治理）**：配置服务化、Dashboard 拆分、前后端契约生成、多项目 analyzer 隔离、批量写库优化。
- **P2（中期治理）**：聚合 SQL 下推、查询缓存体系、性能基准体系、构建链路优化。

### 5.6 AI 代理执行约束

- 任何 AI 代理在处理上述负债时，必须优先提交**窄范围、可验证、可回滚**的 PR，不允许把多项高风险重构混在单个 PR 中。
- 所有负债治理 PR 必须在描述中明确：
  - 解决的是哪一类负债
  - 当前实现风险是什么
  - 验证方式是什么
  - 是否引入迁移、兼容层或行为变更
- 若涉及扫描结果语义、配置格式、API 契约、数据库结构，必须先更新本蓝图再实施代码修改。

