# CodePrism

<p align="center">
  <strong>🔬 高性能 Git 仓库代码分析工具</strong>
</p>

<p align="center">
  <a href="#-快速开始">快速开始</a> •
  <a href="#-安装">安装</a> •
  <a href="#-命令行参考">命令行参考</a> •
  <a href="#-配置">配置</a>
</p>

<p align="center">
  <a href="./README.md">English</a> |
  <a href="./README.zh-CN.md">简体中文</a> |
  <a href="./README.ja.md">日本語</a>
</p>

<p align="center">
  <a href="https://github.com/yougikou/code-prism/releases"><img src="https://img.shields.io/github/v/release/yougikou/code-prism?style=flat-square" alt="Release"></a>
  <a href="https://github.com/yougikou/code-prism/actions"><img src="https://img.shields.io/github/actions/workflow/status/yougikou/code-prism/release.yml?style=flat-square" alt="Build"></a>
  <a href="./LICENSE"><img src="https://img.shields.io/github/license/yougikou/code-prism?style=flat-square" alt="License"></a>
</p>

---

CodePrism 是一个使用 Rust 构建的**高性能代码分析工具**。它可以扫描 Git 仓库、提取代码指标，并通过直观的 Web 仪表板提供可操作的洞察。采用**服务端驱动 UI** 架构——仪表板的视图、图表和聚合逻辑通过 YAML 配置文件定义，无需修改前端代码即可自定义分析。

![CodePrism Dashboard](screenshot.png)

## ✨ 功能特性

- 🚀 **高性能** - 使用 Rust 构建，速度极快
- 📊 **丰富的分析** - 多种聚合类型（Sum、Avg、TopN、Min、Max、Distribution）和图表可视化（柱状图、折线图、饼图、雷达图、热力图、仪表盘、表格）
- 🔍 **匹配级别详情** - 从聚合指标下钻到单个正则/Python/WASM 匹配位置，包含行号、代码上下文和差异侧过滤
- 🔄 **Git 集成** - 支持快照和差异扫描模式，后台任务追踪
- 🗂️ **Git 仓库管理** - 直接从仪表板克隆远程仓库、拉取最新变更、切换分支和浏览提交
- 🎨 **服务端驱动 UI** - 通过 YAML 配置仪表板，灵活网格布局，支持按视图宽度和变更类型显示模式
- 📦 **多项目支持** - 在一个配置文件中管理多个项目，支持可复用模板
- 🔌 **可扩展分析器** - 内置、正则、Python 和 WASM 分析器，支持按文件上下文、变更类型和扫描模式过滤
- 🌐 **国际化 (i18n)** - 内置多语言 UI（英文、中文、日文），支持运行时切换
- 📋 **扫描任务追踪** - 后台扫描执行，实时状态监控和进度报告
- ⚡ **执行页面** - 统一的仓库管理、分支/提交选择和扫描执行 UI，带进度反馈

### 架构

- **后端**: 基于 Rust 的 CLI 和 Web 服务器，使用 Axum 框架
- **数据库**: 嵌入式 SQLite，支持自动迁移
- **前端**: React + TypeScript + Vite，使用 rust-embed 嵌入到二进制文件中
- **图表**: Apache ECharts 用于高性能数据可视化
- **Git 操作**: 通过 libgit2 直接访问 Git ODB，无需 checkout

运行期职责按边界拆分：`state.rs` 管理服务端共享状态，`scan_routes.rs` 管理扫描任务与摘要 HTTP 契约，`api_error.rs` 统一 JSON API 错误，Scanner 的摘要持久化位于 `summary.rs`。前端的扫描任务请求位于 `services/scan.ts`，轮询与 ETA 状态位于 `hooks/useScanJob.ts`，页面组件只负责展示和工作流协调。

如果 npm 不可用或前端构建失败，Cargo 构建会直接失败，避免静默嵌入过期的 `web/dist` 资源。已经显式构建并验证前端的 CI 和发布任务可设置 `CODEPRISM_SKIP_WEB_BUILD=1`，但预构建的 `web/dist` 仍必须存在。

### CLI 命令

- `init` - 初始化数据库并创建默认配置
- `scan <repo>` - 以快照模式扫描仓库
- `scan <repo> --diff <old> <new>` - 以差异模式扫描仓库
- `serve` - 启动带有仪表板的 Web 服务器
- `init-config` - 生成默认配置文件
- `check-config` - 验证配置文件
- `test-analyzers` - 运行 `custom_analyzers/` 中所有 Python 分析器的自测试

### CLI 辅助脚本

`scripts/` 目录包含仓库管理和批量扫描的辅助脚本：

| 脚本 | 说明 |
|--------|------|
| `codeprism-repo-list` / `.ps1` | 列出已注册的项目及其扫描历史 |
| `codeprism-repo-clone` / `.ps1` | 克隆远程仓库并注册为项目 |
| `codeprism-scan` / `.ps1` | 扫描一个或所有已注册项目（支持批量模式） |
| `codeprism-scan-list` / `.ps1` | 列出最近的扫描状态和时间 |

这些脚本通过 REST API 与正在运行的 CodePrism 服务器通信，为服务器启动模式提供自动化支持——适用于定期扫描、CI/CD 集成和定时分析工作流，无需直接使用 CLI scan 命令。

### 分析器

- **内置**: 文件计数、字符计数
- **正则**: 通过 YAML 配置的模式匹配
- **Python 脚本**: `custom_analyzers/` 目录中的持久进程分析器
- **WASM**: 通过 wasmtime 运行时执行 WebAssembly 模块

#### Python 脚本分析器

Python 分析器采用**持久循环模式**运行，通过 stdin/stdout 高效通信：

- **输入**: 每个脚本通过 stdin 接收每行一个 JSON 对象：
  ```json
  {"file_path": "src/main.rs", "content": "fn main() { ... }"}
  ```
- **输出**: 脚本通过 stdout 输出 JSON 数组：
  ```json
  [{"value": 5.0, "tags": {"metric": "complexity", "category": "complexity"}}]
  ```
- **匹配详情（可选）**: 分析器可以通过可选的 `matches` 字段返回每个匹配的位置信息：
  ```json
  [{
    "value": 3.0,
    "tags": {"metric": "todo_count", "category": "quality"},
    "matches": [
      {"file_path": "src/main.rs", "line_number": 42, "column_start": 9, "column_end": 21, "matched_text": "TODO: refactor", "context_before": "// FIXME: optimize", "context_after": "fn main() {"}
    ]
  }]
  ```
  匹配详情按扫描存储，可通过 Web 仪表板的子列表弹窗点击文件路径查看。
- **生命周期**: 脚本启动后常驻内存，在多次分析请求间复用，避免解释器启动开销。

每个 Python 分析器可以包含 `test()` 函数，通过以下方式调用：
```bash
python custom_analyzers/my_analyzer.py test
```

一键运行所有分析器的自测试：
```bash
codeprism test-analyzers
```
该命令自动发现 `custom_analyzers/` 中的所有 `.py` 文件并执行它们的测试入口点。

**示例分析器**（`custom_analyzers/`）：
- [`gosu_complexity.py`](custom_analyzers/gosu_complexity.py) — Gosu 语言的圈复杂度计算
- [`java_complexity.py`](custom_analyzers/java_complexity.py) — Java 的圈复杂度计算

### 匹配详情查看

当正则、Python 或 WASM 分析器产生匹配级别的数据时，您可以从聚合图表数值下钻到单个匹配位置：

1. **文件列表弹窗**：点击图表卡片上的 **FileText** 图标，查看所有文件及其指标值
2. **匹配详情弹窗**：点击文件路径，查看该文件内的每个匹配位置，包括：
   - **行号和列** — 每个匹配的精确位置
   - **匹配文本** — 在 UI 中以代码格式高亮显示
   - **上下文行** — 匹配行前后各一行，便于阅读

这提供了从聚合指标到原始分析结果的完整可追溯性。

**API 端点：**

```
GET /api/v1/projects/:project_name/scans/:scan_id/matches?file_path=<路径>[&analyzer_id=<ID>&page=1&page_size=100]
```

### 扫描模式

- **快照模式**: 在特定提交时分析整个仓库
- **差异模式**: 分析两个提交或分支之间的变更（追踪新增/修改/删除类型）

扫描以后台任务方式运行，可通过 API 和 Web 仪表板查看执行状态。

## 📥 安装

### 下载预编译版本（推荐）

从 [GitHub Releases](https://github.com/yougikou/code-prism/releases) 下载适合您平台的最新版本：

| 平台 | 下载文件 |
|------|----------|
| **Linux x86_64** | `codeprism-x86_64-unknown-linux-gnu.tar.gz` |
| **macOS (Apple Silicon)** | `codeprism-aarch64-apple-darwin.tar.gz` |
| **Windows x86_64** | `codeprism-x86_64-pc-windows-msvc.zip` |

```bash
# Linux / macOS
tar xzf codeprism-*.tar.gz
chmod +x codeprism
sudo mv codeprism /usr/local/bin/

# 验证安装
codeprism --version
```

### 从源码构建

```bash
git clone https://github.com/yougikou/code-prism.git
cd code-prism
cargo build --release
# 可执行文件位于 target/release/codeprism
```

### 构建前端 Web

构建过程（`crates/server/build.rs`）将在 `npm` 可用时自动尝试构建前端资源。

如果您想手动构建前端，或者自动构建失败：

```bash
cd web
npm install
npm run build
# 资源文件将生成在 web/dist 目录
```



## 🚀 快速开始

```bash
# 1. 初始化数据库
codeprism init

# 2. 扫描你的仓库
codeprism scan /path/to/your/repo

# 3. 启动 Web 仪表板
codeprism serve
```

在浏览器中打开 **http://localhost:3000**。

## 📖 命令行参考

### 全局选项

```
codeprism [选项] <命令>

选项:
  --config <路径>    配置文件路径（默认：codeprism.yaml）
  --help             显示帮助信息
  --version          显示版本信息
```

### 命令

#### `init` - 初始化数据库

```bash
codeprism init
```

创建 SQLite 数据库（`codeprism.db`）并应用所需的表结构。

#### `scan` - 扫描仓库

```bash
codeprism scan <路径> [选项]

参数:
  <路径>  Git 仓库路径（默认：.）

选项:
  -p, --project <名称>     项目名称（默认：目录名）
  --mode <模式>            扫描模式：snapshot 或 diff（默认：snapshot）
  --commit <哈希>          要扫描的特定提交（快照模式）
  --base <哈希>            比较的基准提交（差异模式，必需）
  --target <哈希>          比较的目标提交（差异模式，默认：HEAD）
```

**示例：**

```bash
# 快照扫描当前目录
codeprism scan .

# 扫描特定提交
codeprism scan . --commit abc123

# 两个提交之间的差异扫描
codeprism scan . --mode diff --base abc123 --target def456

# 使用自定义项目名称扫描
codeprism scan ../my-project --project "MyApp"
```

#### `serve` - 启动 Web 仪表板

```bash
codeprism serve [选项]

选项:
  --port <端口>    服务器端口（默认：3000）
```

**示例：**

```bash
# 在默认端口启动
codeprism serve

# 在自定义端口启动
codeprism serve --port 8080

# 使用自定义配置
codeprism serve --config production.yaml
```

#### `init-config` - 生成配置文件

```bash
codeprism init-config [路径]

参数:
  [路径]  输出文件路径（默认：codeprism.yaml）
```

#### `check-config` - 验证配置文件

```bash
codeprism check-config
```

### 退出码

| 代码 | 描述 |
|------|------|
| `0` | 成功 |
| `1` | 一般错误 |
| `2` | 配置错误 |
| `3` | 数据库错误 |
| `4` | Git 错误 |

## 🖥️ Web 仪表板

CodePrism 包含一个功能齐全的 Web 仪表板，使用 React + TypeScript + Vite 构建，嵌入在二进制文件中。

### 页面

| 页面 | 路由 | 说明 |
|------|------|------|
| **仪表板 (Dashboard)** | `/` | 主分析视图，每个技术栈标签页有可配置的图表 |
| **执行 (Execute)** | `/execute` | 仓库管理和扫描执行 UI |
| **配置 (Config)** | `/config` | 视图和项目的可视化配置编辑器 |

### 仪表板功能

- **技术栈标签页** — 每个技术栈拥有独立标签页和对应的聚合视图
- **汇总标签页** — 未指定技术栈或标记为 "All" 的视图显示在此
- **趋势图** — 时序折线图跟踪多个扫描周期的指标变化
- **下钻分析** — 点击图表项查看文件列表，点击文件查看匹配位置（含行号和代码上下文）
- **变更类型过滤** — 每个视图支持堆叠 A/M/D 柱状图或可切换按钮
- **骨架加载** — 数据获取时的平滑加载状态
- **通知提示** — 后台操作的非侵入式反馈

### 执行页面功能

- **仓库管理** — 克隆远程 Git 仓库、拉取最新变更、切换分支
- **本地项目注册** — 将本地目录注册为扫描项目
- **扫描执行** — 使用分支/提交选择运行快照或差异扫描
- **进度跟踪** — 扫描任务实时状态和进度条
- **提交浏览器** — 浏览和搜索提交以供差异扫描参考

### 配置页面功能

- **视图编辑器** — 添加、编辑和删除聚合视图，支持所有 func 类型
- **项目管理器** — 通过 UI 弹窗创建、重命名和删除项目
- **模板管理** — 保存和应用可复用的项目模板
- **实时预览** — 配置变更反映在 YAML 输出中

### 布局

- **侧边栏** — 提供仪表板、执行和配置页面的持久导航
- **顶部栏** — 当前项目信息和语言切换器
- **语言切换器** — 在运行时可切换英文、中文和日文

## ⚙️ 配置

CodePrism 使用 YAML 配置文件。详情请参见[配置指南](#配置文件格式)。

```bash
# 生成默认配置
codeprism init-config

# 使用自定义配置
codeprism --config my-config.yaml scan .
```

### 配置文件格式

```yaml
database_url: "sqlite:codeprism.db"

global_excludes:
  - "**/.git/**"
  - "**/node_modules/**"

tech_stacks:
  - name: "Rust"
    extensions: ["rs", "toml"]
    analyzers: ["char_count"]

aggregation_views:
  top_files:
    title: "Top 10 最大文件"
    tech_stacks: ["Rust"]
    func:
      type: "top_n"
      metric_key: "char_count"
      limit: 10
    chart_type: "bar_row"
```

**视图显示规则：**
- `tech_stacks` **未定义**或**为空**的视图 → 显示在 **Summary** 标签页
- `tech_stacks` 包含 `"All"` 的视图 → 显示在 **Summary** 标签页
- `tech_stacks` 包含特定技术栈名称的视图 → 显示在对应的技术栈标签页

### 聚合视图 func 配置

聚合视图中的 `func` 对象支持以下基于标签过滤的字段：

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `type` | string | **是** | 聚合类型：`sum`, `avg`, `top_n`, `min`, `max`, `distribution` |
| `tag_filters` | object | 否 | 键值过滤对（如 `metric: char_count`, `category: size`） |
| `analyzer_id` | string 或 string[] | 否 | 按分析器 ID 筛选 |
| `limit` | integer | `top_n` 需要 | 返回的结果数量 |
| `order` | string | `top_n` 使用 | 排序方式：`"desc"`（默认）或 `"asc"` |
| `buckets` | float[] | `distribution` 需要 | 分布统计的桶边界 |

**附加视图字段：**

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `width` | integer | `1` | 网格宽度：`1`（半宽）或 `2`（全宽） |
| `include_children` | boolean | `true` | 是否在聚合结果中包含子条目 |
| `change_type_mode` | string | — | 变更类型显示模式：`"all"`（堆叠）、`"switchable"`（A/M/D 切换按钮）或不设置（无过滤） |
| `group_by` | string[] | `[]` | 分组键：`tech_stack`, `category`, `change_type`, `metric_key`, `analyzer_id` |
| `trend` | boolean | `false` | 启用趋势/时序图模式 |
| `trend_limit` | integer | `30` | 趋势中包含的最近扫描次数 |

**支持的分组键：**

`group_by` 字段支持以下键：`tech_stack`, `category`, `change_type`, `metric_key`, `analyzer_id`。

**示例：**

```yaml
# 仅按 metric_key 筛选
func:
  type: "sum"
  tag_filters:
    metric: "char_count"

# 仅按 category 筛选（不指定 metric_key）
func:
  type: "sum"
  tag_filters:
    category: "logging"
group_by: ["metric_key"]

# 无筛选条件（统计所有数据）
func:
  type: "sum"
```

**TopN 排序：**

```yaml
# 最大的 10 个文件（降序，默认）
func:
  type: "top_n"
  tag_filters:
    metric: "char_count"
  limit: 10
  order: "desc"

# 最小的 10 个文件（升序）
func:
  type: "top_n"
  tag_filters:
    metric: "char_count"
  limit: 10
  order: "asc"
```

**视图宽度和变更类型模式：**

```yaml
# 全宽视图，堆叠 A/M/D 显示
aggregation_views:
  code_churn:
    title: "按技术栈的代码变动"
    chart_type: bar_col
    width: 2
    change_type_mode: all
    func:
      type: sum
      tag_filters:
        metric: char_count

# A/M/D 切换按钮
  changes:
    title: "按类别的变更"
    change_type_mode: switchable
    func:
      type: sum
```

### 趋势图 / 时序图

趋势图表允许您通过对同一仓库在不同提交时间点进行多次扫描，跟踪指标随时间的变化。通过在现有 `aggregation_view` 中添加趋势字段即可启用。

**趋势专用字段：**

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `trend` | boolean | `false` | 启用趋势图模式 |
| `trend_limit` | integer | `30` | 趋势中包含的最近扫描次数 |

**工作原理：**

1. 每次扫描会将 Git 提交时间戳（`commit_timestamp`）存入数据库
2. 当视图设置了 `trend: true`，仪表板将其渲染为多系列折线图，独立于单次扫描选择
3. 后端按提交时间排序查询最近的扫描，将每次扫描的聚合数据组装为时序数据点
4. 趋势端点按 `(label, metric_key, category, analyzer_id)` 四元组分组建系列

**配置示例：**

```yaml
# 单一分析器单一指标 — 跟踪文件数变化
aggregation_views:
  file_trend:
    title: "文件数变化趋势"
    chart_type: line
    width: 2
    trend: true
    trend_limit: 30
    func:
      type: sum
      analyzer_id: [file_count]
      tag_filters:
        category: size

# 单一分析器产生多个 metric_key（如复杂度和代码行数）
  python_trend:
    title: "Python 指标趋势"
    group_by: [metric_key]
    chart_type: line
    width: 2
    trend: true
    trend_limit: 30
    func:
      type: sum
      analyzer_id: [my_python_analyzer]

# 多个分析器 × 多个指标
  all_metrics_trend:
    title: "全指标趋势"
    group_by: [analyzer_id, metric_key]
    chart_type: line
    width: 2
    trend: true
    trend_limit: 50
    func:
      type: sum
      analyzer_id: [my_python_analyzer, file_count, char_count]
```

设置了 `trend: true` 的视图仍然可以作为普通单次扫描视图使用 —— 同一份配置同时驱动单次扫描图表和趋势折线图。

**API 端点：**

```
GET /api/v1/projects/:project_name/trends/:view_id?mode=snapshot&limit=20
```

查询参数：
- `mode` — `"snapshot"`（默认）或 `"diff"`
- `limit` — 包含的扫描次数上限（最大 100）
- `base_commit` — 差异模式下按 base commit 过滤扫描

**趋势图功能：**
- X 轴：时间（提交时间戳）
- Y 轴：指标值
- 支持多系列，带可滚动图例
- 交互式数据缩放（滚轮缩放和滑块）
- 悬停提示显示具体日期和数值

**注意事项：**
- `Sum` 和 `Avg` 聚合类型最适合趋势图（产生稳定、可解释的系列）
- `TopN` 可能产生不连续的系列，因为 TOP 项在不同扫描间可能变化
- 趋势视图在仪表板上带有 **Trend** 徽章标识
- 趋势数据独立于所选扫描加载 —— 无需选择特定扫描即可查看趋势

### 保留的 metric_key

以下 `metric_key` 为系统保留，自定义分析器应避免使用：

| metric_key | 说明 |
|------------|------|
| `file_count` | 内置分析器，与扫描文件记录对应 |
| `char_count` | 内置分析器，文件字符数 |

### 自定义分析器指南

#### 跨文件自定义分析器

跨文件脚本采用 `extract` / `finalize` 两阶段协议。`extract` 应返回由分析器决定的 `group_key`；`finalize` 返回 `{"findings": [...]}`，每个 finding 显式包含 `finding_key`、`content`、`occurrences`、`tags` 和 `metrics`。是否形成 finding、最少文件数、最少代码行、分组方法及指标计算都属于分析器内部逻辑，不放入 YAML。YAML 只保留 `tags`、`scan_mode`、`change_type` 等框架级元信息。

框架负责协议校验与事务落库。单个分析器失败会重试一次并保留其中间数据，不影响其他分析器；作业状态为 `completed_with_errors`。成功分析器的中间数据在同一事务中清理，失败数据启动时保留七天。Diff 模式只分析发生变更的文件。

开发自定义分析器时，需理解 `analyzer_id` 和 `metric_key` 的区别：

| 字段 | 用途 | 作用域 |
|------|------|--------|
| `analyzer_id` | 标识**哪个分析器**产生了指标 | 每个分析器全局唯一 |
| `metric_key` | 标识**什么类型的测量值** | 可跨分析器共享 |
| `category` | 指标分组 | 用于过滤/组织 |

**自定义正则分析器字段：**

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `pattern` | string | **必填** | 要匹配的正则表达式 |
| `metric_key` | string | `"custom_match"` | 结果的指标键 |
| `category` | string | — | 用于过滤的类别 |
| `description` | string | — | 可读描述 |
| `tags` | object | `{}` | 附加到结果的任意键值标签 |
| `scan_mode` | string | `"all"` | 应用模式：`"all"`, `"snapshot"`, `"diff"` |
| `change_type` | string | `"all"` | 变更类型过滤：`"all"`, `"A"`（新增）, `"M"`（修改）, `"D"`（删除） |

**自定义实现/脚本分析器字段：**

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `metric_key` | string | — | 结果的指标键 |
| `category` | string | — | 用于过滤的类别 |
| `description` | string | — | 可读描述 |
| `tags` | object | `{}` | 覆盖或补充脚本输出的标签 |
| `scan_mode` | string | `"all"` | 应用模式：`"all"`, `"snapshot"`, `"diff"` |
| `change_type` | string | `"all"` | 变更类型过滤：`"all"`, `"A"`, `"M"`, `"D"` |

**设计模式：**

1. **多个分析器，相同 metric_key** - 不同语言的分析器可以输出相同的 `metric_key`：
   ```yaml
   # Python 复杂度分析器
   analyzer_id: "python_complexity"
   metric_key: "complexity"
   
   # Java 复杂度分析器
   analyzer_id: "java_complexity"
   metric_key: "complexity"  # 相同的 metric_key，便于统一查询
   ```

2. **一个分析器，多个 metric_keys** - 单个分析器可以输出多个指标：
   ```yaml
   analyzer_id: "code_quality"
   # 输出:
   #   metric_key: "todo_count"
   #   metric_key: "fixme_count"
   ```

### 多项目配置

```yaml
projects:
  - name: "frontend"
    tech_stacks:
      - name: "React"
        extensions: ["tsx", "ts"]
        analyzers: ["char_count"]
    aggregation_views: {}

  - name: "backend"
    tech_stacks:
      - name: "Rust"
        extensions: ["rs"]
        analyzers: ["char_count"]
    aggregation_views: {}
```

项目可以通过 **Web 仪表板 UI** 进行管理 — 直接添加、重命名和删除项目，无需手动编辑 YAML 文件。

### 项目模板

可复用的项目配置定义在 `project_templates` 下：

```yaml
project_templates:
  java_service:
    tech_stacks:
      - name: "Java"
        extensions: ["java", "xml"]
        analyzers: ["char_count"]
    global_excludes:
      - "**/target/**"
```

可以通过 Web 仪表板从模板创建新项目。

## 📊 聚合与图表类型

### 聚合类型

| 类型 | 描述 |
|------|------|
| `top_n` | 按值排序的前 N 项 |
| `sum` | 值的总和 |
| `avg` | 平均值 |
| `min` / `max` | 最小/最大值 |
| `distribution` | 分桶分布 |

### 图表类型

| 类型 | 描述 |
|------|------|
| `bar_row` | 水平条形图 |
| `bar_col` | 垂直条形图 |
| `pie` | 饼图 |
| `table` | 数据表格 |
| `gauge` | 仪表盘 |
| `radar` | 雷达图 |
| `line` | 折线图 |
| `heatmap` | 热力图 |

## 🏗️ 架构

```mermaid
flowchart TD
    A[Web 仪表板<br/>React + ECharts] -->|REST API| B[API 服务器<br/>Axum + 服务端驱动配置]
    B --> C[扫描引擎<br/>Git 集成 + 分析器]
    C --> D[SQLite 数据库<br/>指标 + 扫描历史]
```

## 📚 API 参考

### 核心端点

| 方法 | 路径 | 说明 |
|--------|------|------|
| GET | `/api/v1/projects/:name/scans/:scan_id/views/:view_id` | 获取聚合视图数据 |
| GET | `/api/v1/projects/:name/scans` | 列出项目的扫描记录 |
| GET | `/api/v1/projects/:name/trends/:view_id` | 获取趋势/时序数据 |
| GET | `/api/v1/projects/:name/scans/:scan_id/matches` | 获取匹配级别详情（分页） |
| GET | `/api/v1/scans/jobs/:job_id` | 获取扫描任务状态和进度 |
| GET | `/api/v1/config` | 获取当前配置 |

### 项目管理

| 方法 | 路径 | 说明 |
|--------|------|------|
| GET | `/api/v1/projects/unified` | 列出所有项目（配置 + 数据库） |
| POST | `/api/v1/projects/add-local` | 注册本地 Git 仓库 |
| POST | `/api/v1/projects` | 创建新项目 |
| DELETE | `/api/v1/projects/:name` | 删除项目 |
| POST | `/api/v1/scans/execute` | 执行新扫描 |

### Git 仓库管理

| 方法 | 路径 | 说明 |
|--------|------|------|
| GET | `/api/v1/git/repos` | 列出所有缓存的仓库 |
| POST | `/api/v1/git/clone` | 克隆远程仓库 |
| GET | `/api/v1/git/:repo_id/branches` | 列出分支 |
| POST | `/api/v1/git/:repo_id/checkout` | 切换分支 |
| POST | `/api/v1/git/:repo_id/pull` | 拉取最新变更 |
| GET | `/api/v1/git/:repo_id/commits` | 列出提交（支持搜索） |
| DELETE | `/api/v1/git/:repo_id` | 删除缓存的仓库 |

### 模板

| 方法 | 路径 | 说明 |
|--------|------|------|
| GET | `/api/v1/templates` | 列出项目模板 |

- 文档：Swagger UI `/swagger-ui`（需要服务器运行中），OpenAPI 规范 `/api-docs/openapi.json`

## 🤝 贡献

欢迎贡献！请查看上述文档了解指南。

## 📄 许可证

MIT License - 详见 [LICENSE](./LICENSE)
