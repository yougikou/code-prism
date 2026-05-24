# CodePrism CLI Scripts

通过服务器 API 操作 CodePrism，避免 CLI 与服务器直接竞争 SQLite 数据库。

## 依赖

- **Bash (Mac/Linux)**: `curl` + `jq`
- **PowerShell (Windows)**: 无额外依赖（使用 `Invoke-RestMethod`）

## 安装

将 `scripts/` 目录添加到 `PATH` 中：

```bash
# Mac/Linux
export PATH="/path/to/code-prism/scripts:$PATH"

# Windows (PowerShell)
$env:Path = "C:\path\to\code-prism\scripts;$env:Path"
```

或者添加到 shell 配置文件（`~/.bashrc`, `~/.zshrc` 等）中永久生效。

## 服务器地址

默认 `http://localhost:3000`。可通过以下方式配置：

```bash
# 1. 环境变量
export CODEPRISM_SERVER=http://localhost:8080

# 2. 命令行参数（优先级高于环境变量）
codeprism-scan --server http://localhost:8080 --project myapp --mode snapshot

# PowerShell
$env:CODEPRISM_SERVER = 'http://localhost:8080'
.\codeprism-scan.ps1 -Server http://localhost:8080 -Project myapp -Mode snapshot
```

## 命令

### codeprism-scan

触发扫描。通过项目名或 repo-id 对已缓存仓库扫描，或者克隆新仓库扫描。

```bash
# 对已缓存仓库做快照扫描（推荐：用项目名）
codeprism-scan --project myapp --mode snapshot --wait

# 对已缓存仓库做差异扫描
codeprism-scan --project myapp --mode diff --base-ref HEAD~1 --wait

# 指定分支和提交
codeprism-scan --project myapp --branch main --ref abc123 --mode snapshot

# 用 repo-id（UUID）指定仓库
codeprism-scan --repo-id "uuid-here" --mode diff --base-ref HEAD~3

# 克隆新仓库并扫描
codeprism-scan --git-url https://github.com/user/repo.git --project myapp --mode snapshot

# 不等待，仅获取 job_id（异步）
codeprism-scan --project myapp --mode snapshot --json
```

### codeprism-scan-list

列出项目的扫描记录。

```bash
# 列出快照扫描
codeprism-scan-list --project myapp

# 列出差异扫描
codeprism-scan-list --project myapp --mode diff

# JSON 格式输出
codeprism-scan-list --project myapp --json
```

### codeprism-repo-list

列出服务器缓存的 Git 仓库。

```bash
codeprism-repo-list
codeprism-repo-list --json
```

### codeprism-repo-clone

将远程仓库克隆到服务器缓存中。

```bash
codeprism-repo-clone --git-url https://github.com/user/repo.git
codeprism-repo-clone --git-url https://github.com/user/repo.git --project myapp
```

## 工作流示例

```bash
# 1. 克隆仓库
codeprism-repo-clone --git-url https://github.com/myteam/myapp.git --project myapp

# 2. 触发快照扫描并等待完成
codeprism-scan --project myapp --mode snapshot --wait

# 3. 列出扫描记录
codeprism-scan-list --project myapp

# 4. 触发差异扫描
codeprism-scan --project myapp --mode diff --base-ref HEAD~1 --wait

# 5. 脚本中获取 scan_id
scan_id=$(codeprism-scan --project myapp --mode snapshot --wait --json | jq -r '.scan_id')
```

## PowerShell 使用

所有功能与 Bash 版本一致，参数名使用 PascalCase：

```powershell
.\codeprism-repo-clone.ps1 -GitUrl https://github.com/user/repo.git -Project myapp
.\codeprism-scan.ps1 -Project myapp -Mode snapshot -Wait
.\codeprism-scan.ps1 -Project myapp -Mode diff -BaseRef HEAD~1 -Wait
.\codeprism-scan-list.ps1 -Project myapp
.\codeprism-repo-list.ps1
```

## 参数说明

| 参数 | Bash | PowerShell | 说明 |
|------|------|-----------|------|
| 服务器地址 | `--server <url>` | `-Server <url>` | 默认 http://localhost:3000 |
| 项目名称 | `--project <name>` | `-Project <name>` | 自动解析到 repo-id |
| 仓库 ID | `--repo-id <uuid>` | `-RepoId <uuid>` | 直接指定缓存仓库 |
| Git URL | `--git-url <url>` | `-GitUrl <url>` | 克隆新仓库 |
| 目标提交 | `--ref <commit>` | `-Ref <commit>` | 默认使用最新提交 |
| 分支 | `--branch <name>` | `-Branch <name>` | 默认使用当前分支 |
| 基线提交 | `--base-ref <commit>` | `-BaseRef <commit>` | diff 模式必需 |
| 扫描模式 | `--mode snapshot|diff` | `-Mode snapshot|diff` | 默认 snapshot |
| 等待完成 | `--wait` | `-Wait` | 轮询直到扫描完成 |
| JSON 输出 | `--json` | `-Json` | 输出原始 JSON |
