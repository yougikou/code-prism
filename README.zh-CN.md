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

CodePrism 是一个使用 Rust 构建的**高性能代码分析工具**。它可以扫描 Git 仓库、提取代码指标，并通过直观的 Web 仪表板提供可操作的洞察。

![CodePrism Dashboard](screenshot.png)

## ✨ 功能特性

- 🚀 **高性能** - 使用 Rust 构建，速度极快
- 📊 **丰富的分析** - 多种聚合类型和图表可视化
- 🔄 **Git 集成** - 支持快照和差异扫描模式
- 🎨 **服务端驱动 UI** - 通过 YAML 配置仪表板
- 📦 **多项目支持** - 在一个配置文件中管理多个项目
- 🔌 **可扩展分析器** - 内置、正则、Python 和 WASM 分析器

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

聚合视图中的 `func` 对象支持以下字段：

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `type` | string | **是** | 聚合类型：`sum`, `avg`, `top_n`, `min`, `max`, `distribution` |
| `metric_key` | string | 否 | 按指标键筛选（如 `"char_count"`） |
| `category` | string | 否 | 按类别筛选（如 `"logging"`） |
| `analyzer_id` | string | 否 | 按分析器 ID 筛选 |
| `limit` | integer | `top_n` 需要 | 返回的结果数量 |
| `buckets` | float[] | `distribution` 需要 | 分布统计的桶边界 |

**支持的分组键：**

`group_by` 字段支持以下键：`tech_stack`, `category`, `change_type`, `metric_key`, `analyzer_id`。

**示例：**

```yaml
# 仅按 metric_key 筛选
func:
  type: "sum"
  metric_key: "char_count"

# 仅按 category 筛选（不指定 metric_key）
func:
  type: "sum"
  category: "logging"
group_by: ["metric_key"]

# 无筛选条件（统计所有数据）
func:
  type: "sum"
```

### 保留的 metric_key

以下 `metric_key` 为系统保留，自定义分析器应避免使用：

| metric_key | 说明 |
|------------|------|
| `file_count` | 内置分析器，与扫描文件记录对应 |
| `char_count` | 内置分析器，文件字符数 |

### 自定义分析器指南

开发自定义分析器时，需理解 `analyzer_id` 和 `metric_key` 的区别：

| 字段 | 用途 | 作用域 |
|------|------|--------|
| `analyzer_id` | 标识**哪个分析器**产生了指标 | 每个分析器全局唯一 |
| `metric_key` | 标识**什么类型的测量值** | 可跨分析器共享 |
| `category` | 指标分组 | 用于过滤/组织 |

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

```
┌─────────────────────────────────────────┐
│              Web 仪表板                 │
│         (React + ECharts)               │
└─────────────────┬───────────────────────┘
                  │ REST API
┌─────────────────▼───────────────────────┐
│             API 服务器                  │
│    (Axum + 服务端驱动配置)              │
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│            扫描引擎                     │
│     (Git 集成 + 分析器)                 │
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│          SQLite 数据库                  │
│      (指标 + 扫描历史)                  │
└─────────────────────────────────────────┘
```

## 📚 文档

- [项目蓝图](./PROJECT_BLUEPRINT.md)
- [模块结构](./STRUCTURE_AND_MODULES.md)
- [API 文档](http://localhost:3000/swagger-ui)（需要服务器运行中）

## 🤝 贡献

欢迎贡献！请查看上述文档了解指南。

## 📄 许可证

MIT License - 详见 [LICENSE](./LICENSE)
