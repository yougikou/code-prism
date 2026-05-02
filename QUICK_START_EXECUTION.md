# 快速测试执行页面

## 编译和运行

```bash
# 编译项目（调试模式，编译速度快）
cargo build

# 运行服务器
cargo run -- serve --port 3000
```

服务器启动后，访问：`http://localhost:3000`

## WebUI 使用步骤

### 1. 进入执行页面

点击导航栏中的"执行扫描"按钮。

### 2. 填写表单

#### 快照扫描示例

```
Git 仓库 URL: https://github.com/user/repo.git
项目名称: my-project
扫描类型: 快照扫描
分支: main
提交: (留空，使用最新)
```

点击"开始扫描"按钮。

#### 差异扫描示例

```
Git 仓库 URL: https://github.com/user/repo.git
项目名称: my-project-diff
扫描类型: 差异扫描
分支: main
基础提交: v1.0.0
目标提交: v2.0.0
```

点击"开始扫描"按钮。

### 3. 查看结果

扫描在后台进行。完成后可以：
1. 切换到"分析仪表板"查看结果
2. 或者等待页面显示完成消息

## API 测试

使用 curl 测试 API：

### 快照扫描

```bash
curl -X POST http://localhost:3000/api/v1/scan \
  -H "Content-Type: application/json" \
  -d '{
    "git_url": "https://github.com/torvalds/linux.git",
    "scan_mode": "snapshot",
    "project_name": "linux-test"
  }'
```

### 差异扫描

```bash
curl -X POST http://localhost:3000/api/v1/scan \
  -H "Content-Type: application/json" \
  -d '{
    "git_url": "https://github.com/torvalds/linux.git",
    "scan_mode": "diff",
    "base_commit": "v6.0",
    "commit": "v6.1",
    "project_name": "linux-diff-test"
  }'
```

## 完整特性列表

✅ **已实现**

- [x] 后端扫描 API 端点 (`POST /api/v1/scan`)
- [x] Git URL 克隆支持
- [x] Branch 选择和 checkout
- [x] Snapshot 扫描模式
- [x] Diff 扫描模式
- [x] 前端执行页面
- [x] 导航栏切换
- [x] 表单验证
- [x] 状态反馈
- [x] 后台异步执行

📋 **可选计划中的功能**

- [ ] 扫描进度显示
- [ ] 实时日志流
- [ ] 扫描任务队列
- [ ] 取消扫描功能
- [ ] 认证和授权

## 故障排除

### 问题 1：前端构建失败

```bash
cd web
npm install
npm run build
cd ..
cargo build
```

### 问题 2：Git 克隆超时

增加超时时间或检查网络连接。临时目录位置：
- Linux/Mac: `/tmp/codeprism-*`
- Windows: `%TEMP%\codeprism-*`

### 问题 3：权限错误

确保有足够的权限：
- 读取 Git 仓库
- 写入临时目录
- 写入 SQLite 数据库

## 下一步

1. 查看 [EXECUTION_PAGE_GUIDE.md](./EXECUTION_PAGE_GUIDE.md) 了解详细用法
2. 查看 [README.md](./README.md) 了解其他功能
3. 查看 [CLAUDE.md](./CLAUDE.md) 了解项目架构
