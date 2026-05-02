# WebUI 执行页面使用指南

## 概述

CodePrism 现在支持通过 WebUI 直接执行代码扫描，无需使用命令行。新的执行页面允许用户：

1. 通过 Git URL 克隆代码仓库
2. 选择指定的分支进行 checkout
3. 执行快照扫描（分析特定提交）
4. 执行差异扫描（分析两个提交之间的差异）

## 快速开始

### 1. 启动服务器

```bash
cargo run -- serve --port 3000
```

或使用 release 版本（推荐用于生产环境）：

```bash
cargo build --release
./target/release/codeprism serve --port 3000
```

### 2. 访问 WebUI

打开浏览器访问：`http://localhost:3000`

点击导航栏中的"执行扫描"标签进入执行页面。

## 执行页面功能详解

### 表单字段说明

#### 必填字段

**Git 仓库 URL** ⭐
- 格式：`https://github.com/user/repo.git`
- 支持 HTTP 和 SSH 协议
- 示例：
  - `https://github.com/torvalds/linux.git`
  - `git@github.com:user/private-repo.git`

#### 可选字段

**项目名称**
- 用于在数据库中标识该扫描结果
- 如果不填写，将使用默认值 `scanned_project`
- 同一项目名称的多次扫描会被组织到一起

**分支**
- 指定要 checkout 的分支
- 示例：`main`, `develop`, `feature/new-feature`
- 如果不填写，将使用仓库默认分支

### 扫描类型

#### 1. 快照扫描（Snapshot）

扫描特定提交的代码快照。适用于：
- 分析特定版本的代码
- 监控发布版本的代码质量
- 获取某个时间点的代码指标

**参数：**
- 提交（可选）：指定具体的提交 hash 或 tag
  - 示例：`abc123def456`、`v1.0.0`、`HEAD`
  - 如果不填写，默认扫描最新提交

**API 请求示例：**

```bash
curl -X POST http://localhost:3000/api/v1/scan \
  -H "Content-Type: application/json" \
  -d '{
    "git_url": "https://github.com/user/repo.git",
    "scan_mode": "snapshot",
    "branch": "main",
    "project_name": "my-project"
  }'
```

#### 2. 差异扫描（Diff）

比较两个提交之间的差异，分析代码变化。适用于：
- 审查 Pull Request 的代码变化
- 分析版本更新之间的变化
- 持续集成中的增量分析

**参数：**
- 基础提交（必填）：起始提交或分支
  - 示例：`main`、`develop`、`abc123`
- 目标提交（可选）：结束提交
  - 如果不填写，默认为 `HEAD`

**API 请求示例：**

```bash
curl -X POST http://localhost:3000/api/v1/scan \
  -H "Content-Type: application/json" \
  -d '{
    "git_url": "https://github.com/user/repo.git",
    "scan_mode": "diff",
    "branch": "main",
    "base_commit": "v1.0.0",
    "commit": "v2.0.0",
    "project_name": "my-project"
  }'
```

## API 详细说明

### 端点

**POST /api/v1/scan**

### 请求体

```typescript
{
  git_url: string;           // 必填：Git 仓库 URL
  scan_mode: string;         // 必填："snapshot" 或 "diff"
  branch?: string;           // 可选：要 checkout 的分支
  commit?: string;           // 可选：提交 hash（快照）或目标提交（差异）
  base_commit?: string;      // 可选：基础提交（差异模式需要）
  project_name?: string;     // 可选：项目名称，默认为 "scanned_project"
}
```

### 响应

成功响应（HTTP 200）：

```json
{
  "scan_id": 0,
  "project_name": "my-project",
  "status": "started",
  "message": "Scan has been queued and will start shortly"
}
```

错误响应（HTTP 400/500）：

```json
{
  "scan_id": 0,
  "project_name": "",
  "status": "error",
  "message": "错误描述信息"
}
```

## 常见用例

### 场景 1：扫描 GitHub 仓库的最新代码

1. Git URL: `https://github.com/torvalds/linux.git`
2. 扫描类型: 快照扫描
3. 项目名称: `linux-kernel`
4. 分支: `master`
5. 提交: 留空（默认 HEAD）

### 场景 2：比较两个版本之间的差异

1. Git URL: `https://github.com/user/myapp.git`
2. 扫描类型: 差异扫描
3. 项目名称: `myapp`
4. 分支: `main`
5. 基础提交: `v1.0.0`
6. 目标提交: `v2.0.0`

### 场景 3：分析开发分支的变化

1. Git URL: `https://github.com/user/project.git`
2. 扫描类型: 差异扫描
3. 项目名称: `project-dev`
4. 分支: `develop`
5. 基础提交: `main`
6. 目标提交: 留空（使用 HEAD，即开发分支最新）

## 工作流程

### 后端处理流程

```
1. 用户提交扫描请求
   ↓
2. 验证请求参数
   ↓
3. 在线程池中克隆 Git 仓库到临时目录
   ↓
4. 如果指定 branch，进行 checkout
   ↓
5. 立即返回 202 响应（异步处理）
   ↓
6. 后台异步执行扫描
   ├─ 运行所有配置的分析器
   ├─ 将结果存储到 SQLite 数据库
   └─ 清理临时目录
   ↓
7. 扫描完成，用户可在仪表板查看结果
```

### 性能考虑

- **克隆时间**：取决于仓库大小（通常 10MB-1GB）
- **扫描时间**：取决于代码行数和配置的分析器数量
- **后台执行**：用户不需要等待，立即获得响应

## 故障排除

### 问题：Git 克隆失败

**可能原因：**
- Git URL 格式错误
- 网络连接问题
- 仓库访问权限问题（私有仓库）

**解决方案：**
- 检查 URL 格式是否正确
- 确保网络连接正常
- 对于私有仓库，使用 SSH URL 或提供凭证

### 问题：Branch checkout 失败

**可能原因：**
- 分支名称拼写错误
- 分支不存在

**解决方案：**
- 确认分支名称正确
- 不指定分支，使用默认分支

### 问题：差异扫描无结果

**可能原因：**
- 基础提交和目标提交相同
- 提交不存在

**解决方案：**
- 确保基础提交早于目标提交
- 验证提交 hash 或分支名称是否存在

## 技术实现细节

### 依赖关系

后端扫描功能使用以下主要组件：

```
routes.rs (execute_scan)
    ├─ AppState (包含 core_config 和 db)
    ├─ Scanner (codeprism_scanner)
    ├─ git2 (Git 操作)
    └─ tokio (异步运行)
```

### 关键文件

| 文件 | 作用 |
|------|------|
| `crates/server/src/routes.rs` | `execute_scan` 端点实现 |
| `crates/server/src/lib.rs` | 路由配置和 AppState 初始化 |
| `web/src/components/ExecutePage.tsx` | 前端表单和 UI |
| `web/src/App.tsx` | 页面路由 |

### 异步架构

执行页面采用异步架构以支持长时间运行的扫描：

1. **同步克隆**：在 `spawn_blocking` 中执行 Git 克隆
2. **异步扫描**：在 `tokio::spawn` 中异步执行扫描
3. **后台清理**：扫描完成后自动清理临时文件

这样设计可以：
- 防止阻塞 HTTP 处理器
- 支持并发扫描
- 立即返回响应给用户

## 配置

扫描过程使用 `codeprism.yaml` 中配置的分析器。确保：

```yaml
tech_stacks:
  - name: "Python"
    extensions: ["py"]
    analyzers: ["file_count", "char_count"]
    
custom_regex_analyzers:
  todo_finder:
    pattern: "TODO|FIXME"
    metric_key: "todo_count"
```

## 扩展功能建议

### 已计划改进

- [ ] 显示扫描进度（百分比）
- [ ] 实时日志流
- [ ] 取消正在进行的扫描
- [ ] 扫描历史记录和对比
- [ ] 并发扫描限制设置
- [ ] 认证和权限管理

### 社区贡献欢迎

如有建议或发现 bug，请在 GitHub Issues 中反馈。

## 更新日志

### v0.1.0 (2024-05)

- ✅ 实现执行页面
- ✅ 支持快照扫描
- ✅ 支持差异扫描
- ✅ Git URL 克隆和 branch checkout
- ✅ 后台异步扫描

## 许可证

参见项目根目录的 LICENSE 文件。
