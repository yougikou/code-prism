# CodePrism 模块化架构设计 (Modular Architecture)

为了优化 AI Agent 的上下文理解能力并减少 Token 消耗，本项目采用 **Rust Workspace** 模式进行物理分层。

每个模块都有明确的职责边界。在开发时，请指引 Agent 仅关注当前任务涉及的模块路径。

## 1. 根目录结构

```
codeprism/
├── Cargo.toml              # Workspace 定义
├── PROJECT_BLUEPRINT.md    # 全局蓝图 (Global Context)
├── web/                    # 前端项目 (React)
└── crates/                 # 后端模块 (Rust Crates)
    ├── core/               # 核心类型定义 (Shared Types)
    ├── database/           # 数据库操作 (SQLx)
    ├── git_scanner/        # Git 交互层 (Git2)
    ├── analyzer/           # 分析器逻辑 (Business Logic)
    └── server/             # Web 服务与聚合 (Axum)
└── src/                    # CLI 入口 (Main Binary)

```

## 2. 后端模块详解 (Rust Crates)

### 2.1 核心层: `crates/core`

* **职责**: 定义整个系统共用的数据结构、枚举、错误类型。

* **AI 提示**: "这是数据协议层，只包含 `struct` 和 `enum` 定义，不包含业务逻辑。"

* **关键内容**:

  * `MetricEntry` (结构体)

  * `ChangeType` (枚举: Add, Modify, Delete)

  * `AppError` (统一错误处理)

### 2.2 数据层: `crates/database`

* **职责**: 处理 SQLite 连接、Schema 迁移、CRUD 操作。

* **依赖**: `crates/core`, `sqlx`

* **AI 提示**: "只关注 SQL 语句编写和数据库交互，不要处理 HTTP 或 Git 逻辑。"

* **关键内容**:

  * `Migration` logic

  * `ProjectRepository`, `ScanRepository`, `MetricsRepository`

### 2.3 扫描层: `crates/git_scanner`

* **职责**: 与 Git 仓库交互，读取 Tree/Blob，计算 Diff。

* **依赖**: `crates/core`, `git2` (或 `gix`)

* **AI 提示**: "只关注如何从 Git ODB 中读取文件内容和差异，输出原始数据流。"

* **关键内容**:

  * `SnapshotWalker`: 遍历某个 Commit 的所有文件。

  * `DiffCalculator`: 计算两个 Commit 间的文件变更。

### 2.4 分析层: `crates/analyzer`

* **职责**: 纯函数的代码分析逻辑。输入文本，输出指标。

* **依赖**: `crates/core`

* **AI 提示**: "输入是字符串或字节流，输出是 `MetricEntry` 列表。不涉及 IO 操作。"

* **关键内容**:

  * `Analyzer` (Trait 定义)

  * `LineCounter` (实现)

  * `ComplexityAnalyzer` (实现)

### 2.5 服务层: `crates/server`

* **职责**: 启动 HTTP 服务，处理路由，执行**聚合统计**。

* **依赖**: `crates/core`, `crates/database`, `axum`

* **AI 提示**: "负责 API 路由定义和 `Aggregation Engine` 的实现（Group By/Sum 逻辑）。"

* **关键内容**:

  * `routes/`

  * `aggregation/` (统计引擎)

  * `config.rs` (View 配置文件解析)

### 2.6 CLI 入口: `src/main.rs` (Root Binary)

* **职责**: 胶水层。解析命令行参数，组装上述模块。

* **依赖**: 所有 crates, `clap`

* **AI 提示**: "负责解析 `clap` 命令，并调用各模块的公共接口。"

## 3. 前端模块详解 (web/)

前端建议按**功能特性 (Feature-based)** 而非技术类型分层，这样 AI 在修改一个功能时只需关注一个文件夹。

```
web/
├── src/
│   ├── api/                # API 客户端 (Fetch/Axios)
│   ├── components/         # 通用 UI 组件 (Button, Card)
│   ├── features/           # <--- 关键：按业务模块划分
│   │   ├── dashboard/      # 仪表盘相关组件和逻辑
│   │   ├── diff-view/      # 差异对比视图
│   │   └── project-list/   # 项目列表视图
│   ├── lib/                # 工具函数 (Utils)
│   └── main.tsx            # 入口

```
