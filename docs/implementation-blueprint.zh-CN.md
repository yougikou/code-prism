# CodePrism 本地开发、构建发布与删除保护实施蓝图

状态：实施中。L01、D01–D06、B01–B04 的本地实现已落地；本机 Node 检查未能运行，三平台打包、Docker 与 tag 发布仍需 CI 实际验证；V01 未完成。

基线：2026-10-25，提交 `a116133`，项目版本 `0.4.1`。

适用对象：按单个任务执行的低成本编码模型。开始前核对实际代码；行号不作为定位依据。

## 1. 目标与范围

1. 全新克隆后，一条命令生成完整、可验证的本机发布包。
2. 发布时，标签、源码 commit、二进制版本、压缩包和 Docker 镜像一致。
3. 删除操作只有三档：**移除登记、归档项目、永久删除**。
4. 本地或归属不明的仓库，任何 API 都不得删除其磁盘文件。
5. 删除失败必须可见，不得把“已发起删除”返回成“已删除”。
6. 本地测试固定使用项目根目录的 `.codeprism-dev/`，直接运行编译产物，无需复制文件。

不做：应用回收站、托管克隆定期清理、文件恢复、自动过期删除、通用任务平台、包管理器分发、增加发布平台、无关架构重构。归档恢复属于归档操作的逆操作，不是第四档删除。

永久删除就是不可恢复的操作。删除故障记录用于继续或诊断未完成操作，不提供恢复已删除文件的能力。

## 2. 当前代码事实

| 范围 | 入口 | 现状 |
| --- | --- | --- |
| 本地构建 | `crates/server/build.rs` | 调用 `npm run build`，不安装依赖；跳过前端时仅检查 dist 目录存在 |
| 源码安装说明 | `README.zh-CN.md` | 仅执行 `cargo build --release`，缺少前端依赖准备 |
| CI | `.github/workflows/ci.yml` | 有前后端检查；Rust 使用占位 HTML，不验证真实前端嵌入 |
| 发布 | `.github/workflows/release.yml` | 三个平台各建前端；手动输入 tag 未传给 checkout；发布未直接依赖质量检查 |
| 版本 | 根目录及各 crate 的 `Cargo.toml`、`web/package*.json` | 分散维护 `0.4.1` |
| Docker | `Dockerfile` | 仅复制二进制，没有 Python 和分析器；预发布也更新 latest |
| 分析器资源 | `crates/git_scanner/src/lib.rs` | 从当前工作目录查找 `custom_analyzers` |
| 仓库归属 | `crates/server/src/git_cache.rs`、`lib.rs` | 有 managed 和路径约束；启动补登记仍根据路径推断 managed |
| 移除仓库 | `crates/server/src/git_routes.rs::delete_repo` | 默认仅移除缓存登记；显式参数可异步删文件，错误被忽略 |
| 删除项目 | `crates/server/src/routes.rs::delete_project` | 删除 YAML、数据库数据及关联托管克隆；没有归档 |
| 前端 | `ExecutePage.tsx`、`ConfigPage.tsx` | 本地仓库也显示删文件选项；项目删除确认翻译键缺失 |

这些是静态检查结论，不代表现有构建或测试已通过。实施前不要依据本文声称远端 CI 或发布状态正常。

## 3. 删除行为契约（先固定，实施时不得自行扩大）

### 3.1 三档操作

| 操作 | 对象 | 登记与配置 | 扫描历史 | 磁盘文件 | 恢复方式 |
| --- | --- | --- | --- | --- | --- |
| 移除登记 | 仓库登记 | 移除该仓库登记；解除关联项目中指向该仓库的 repo_path；保留项目本身和分析规则 | 保留 | 全部保留 | 用户显式重新登记 |
| 归档项目 | 项目 | 保留完整配置，记录 archived 状态，默认列表隐藏 | 保留，可只读查看 | 全部保留 | 取消归档 |
| 永久删除 | 项目 | 删除该项目配置及关联仓库登记 | 删除该项目数据 | 仅显式勾选且满足全部保护条件的托管克隆可以删除；默认保留 | 无 |

不提供独立的“永久删除仓库”入口。仓库行只提供“移除登记”；磁盘文件删除是“永久删除项目”中的可选项，不是第四档操作。

移除登记之后，即使文件仍位于缓存目录，启动时也不得重新自动登记。取消归档不修改仓库文件或重新克隆。

永久删除本地项目可以删除项目记录和扫描历史，但必须保留本地源码。对于非托管仓库，删文件选项在 UI 隐藏，后端收到删文件请求时明确拒绝，不静默降级。

### 3.2 归档状态的唯一来源

- 在 core 的 `ProjectConfig` 添加 `archived: bool`，反序列化默认 false；YAML 是归档状态的唯一来源，不再添加一份数据库归档状态。
- `get_projects()` 保留返回全部项目的语义。另增明确的活动项目过滤方法，禁止全局修改旧方法导致历史数据不可见。
- 应用配置和统一项目列表返回 archived；前端默认过滤，归档管理视图显示归档项目。
- 归档项目禁止新扫描、克隆绑定、修改分析规则和项目重命名；允许只读历史、取消归档、永久删除。
- 模板不得继承 archived=true；模板实例化和新项目始终 active。
- CLI `scan --project ...` 与 HTTP 扫描入口必须执行相同归档检查，不能仅禁用按钮。
- 只有历史扫描、没有 YAML 配置的项目，归档时先生成最小项目配置保留其名称；不能丢失历史，也不能凭空生成仓库路径。

### 3.3 路径和归属保护

一个磁盘目录可删除必须同时满足：

1. 可信克隆登记明确标记由 CodePrism 创建；启动补登记、add-local 和未知旧记录不得仅凭目录位置升级为托管。
2. 克隆记录中的 repo_id 与预期 `cache_root/repo_id` 一致，且路径规范化后为该缓存根下的目标克隆目录；不能只检查任意后代路径。
3. 拒绝缓存根本身、根外路径、符号链接或 Windows junction/reparse point 导致的路径替换。对支持平台无法可靠判断的情形采取拒绝删除。
4. 未被其他项目引用，不与其他登记共享相同目录，也不是其他仓库路径的祖先目录。
5. 无正在执行的扫描、克隆、pull、checkout 或其他修改该目录的操作。
6. 无未提交修改、未跟踪文件、本地独有提交；若没有可验证远端跟踪关系或不能确定是否存在未推送内容，拒绝删文件。

检查与删除必须在同一应用内仓库互斥区内进行，删除前再次核对路径。应用锁不能防止外部进程替换目录：实现需说明剩余边界，不能宣称消除所有路径竞态。不引入自动强制删除绕过参数。

旧 managed 记录可继续展示，但只有具有可验证克隆来源和匹配路径的记录才能删文件；无法证明归属时保留文件，提示手动处理。

### 3.4 API 契约

新接口使用现有鉴权和 JSON 错误规范，不新增第二套认证。

| 方法与路径 | 请求 | 结果 |
| --- | --- | --- |
| `DELETE /api/v1/git/:repo_id` | 无请求体，禁止 remove_files=true | 只移除登记及路径绑定；返回 files_removed=false |
| `POST /api/v1/projects/:name/archive` | `{ "archived": true }`，false 表示取消归档 | 返回项目名称及 archived；重复相同操作幂等 |
| `GET /api/v1/projects/:name/deletion-preview` | 无 | 返回可删除范围和阻止原因，不产生副作用 |
| `DELETE /api/v1/projects/:name` | `{ "confirmation_name": "项目原名", "remove_managed_files": false }` | 唯一永久删除入口；必须精确匹配名称，默认不删文件 |

兼容策略：旧客户端调用不带确认名的项目 DELETE 返回 400；旧 `remove_files=true` 返回 400 并提示使用项目永久删除。禁止为了兼容继续保留隐式物理删除。更新文档、前端和脚本中全部调用点。

预览响应最小字段：`project_name`、`scan_count`、`repo_paths`、`can_remove_managed_files`、`blockers`。每条路径标注来源和是否会保留。UI 不依赖绝对路径字符串猜测权限。

HTTP 错误约定：400 确认或参数错误；404 对象不存在；409 任务进行中、共享引用、归档限制、仓库内容不安全；500 持久化或文件操作失败。永久删除执行时重新计算预览条件，不能信任浏览器提交的权限判断。

### 3.5 并发和失败处理

- 采用“有关联活动任务则返回 409”，本轮不实现取消后等待。
- 对同一个项目以及配置文件写入串行化。扫描提交和执行、clone/pull/checkout、配置修改也要参与对应门禁，避免检查后又启动工作。
- 配置文件使用同目录唯一临时文件、写入并原子替换；持久化错误必须向上传递。不要复用固定 `.tmp` 文件。
- SQLite 关联数据删除使用一个事务；只回收没有其他引用的共享 match_contents。
- YAML、SQLite、目录删除不是一个事务，不得宣称“全部原子”。
- 在数据库新增小型 `project_deletion_operations` 表，记录 operation_id、项目名称、明确的待删目录与来源、必要的配置前后快照、阶段、已完成步骤及最后错误。使用 CREATE TABLE IF NOT EXISTS 兼容旧库，不引入通用迁移框架。
- 确认、门禁和预检通过后，先持久化删除意图，再删选定文件，最后删除配置和数据库数据；持有应用锁期间禁止新任务。目录逐一处理，完成一项记录一项。
- 文件删除失败不得继续销毁项目元数据和配置；保留操作记录并返回失败。文件可能已部分删除，错误必须明确这一点；不承诺回滚文件。
- YAML 更新或 DB commit 失败时，用持久化阶段记录恢复一致性。服务启动先检查未完成操作，将有关项目标记为操作受阻；允许只读查看状态，禁止自动恢复扫描或自动继续删文件。通过独立维护命令显式重试同一 operation_id，不增加新的删除档位。
- 删除意图一旦持久化，禁止用同名项目创建或重命名覆盖其目标。重试必须重新校验来源和路径，已经不存在的目标按已完成处理；路径被替换则拒绝继续。
- 仅全部必要步骤成功后返回成功；异步 spawn 后立即返回 files_removed=true 的实现必须删除。
- 维护命令固定为 `codeprism --config <path> resume-project-deletion <operation_id> --confirmation-name <name>`，仅在服务停止时使用，并校验操作绑定的配置文件和数据库。服务与维护命令共用数据目录进程锁，禁止同时运行写操作。

实现过程中若无法满足上述恢复契约，应保留失败状态并报告缺口，不能用忽略错误的方式完成任务。

## 4. 构建、打包、发布契约

### 4.1 统一入口

在根目录 package.json 统一提供 npm 命令，内部仅使用一个 Node.js 脚本 `scripts/dev.mjs`。复用项目已有 Node 环境，不增加 Python 开发脚本或 Bash/PowerShell 双份实现，不引入额外任务运行器。根 package.json 设 private=true，不把 web 改造成 npm workspace。

```sh
npm run dev       # 日常开发：启动隔离后端和 Vite
npm run check     # 提交前：检查及测试
npm run build     # 需要可执行文件时：release 构建
npm run package   # 需要分发时：构建并打包
```

三个平台使用相同 npm 命令。根目录命令不要求先 npm install；脚本按需准备 web 依赖。通过脚本自身位置定位项目根，子进程使用参数数组和明确 cwd；Windows 的 npm.cmd 使用受控平台调用方式，不把用户输入拼进 shell 字符串。

- `dev`：固定 `.codeprism-dev/`，启动 debug 后端和 Vite；详见 L01。不要求记忆额外 web、serve、init 或环境选择命令。
- `build`：`npm ci` → 前端构建 → `cargo build --release --locked`；内部为 Cargo 设置前端复用标志，避免同次构建重复执行前端。
- `check`：前端 lint/test/build → fmt/clippy/workspace tests → 自定义分析器测试；使用真实 dist。
- `package`：默认先 build，组装本机平台包、运行校验、压缩、生成 SHA256SUMS；不创建 tag、不推送、不发布。
- 提供内部参数 `--target` 和 `--prebuilt` 供 CI 复用。prebuilt 也必须验证二进制和前端资源，缺失或不匹配直接失败；不静默重新构建或下载。
- Node/npm 和 Rust 用于开发构建；Python 仅因现有 Python 分析器及其校验需要而保留，不承担开发命令编排。首次使用给出清楚的依赖提示。直接 cargo build 仍可用，但需要先 npm ci，不再称它为完整安装入口。
- 保留现有 build.rs 自动构建行为，避免本轮大改；复用分支至少验证非空 index.html、引用资源存在。Cargo 追踪 dist 变化和相关前端配置变化，确保资源更新后重新嵌入。
- 不通过占位 dist 生成实际发布包。可为纯 Rust 质量检查保留占位方式，但最终集成检查必须构建并请求真实前端资源。

### 4.2 版本与产物

- 根 `[workspace.package]` 管理 Rust 版本，根 package 和所有成员使用 `version.workspace = true`。
- 统一脚本提供 `version <x.y.z[-prerelease]>` 更新 workspace、web/package.json、package-lock.json 并同步 Cargo.lock；不自动提交、不打标签。
- 提供只读版本校验，比较 workspace、前端、lockfile、发布 tag、二进制 `--version`。只接受合法版本，拒绝版本漂移。
- 明确锁定 Rust 和 Node 工具版本；根据现有代码和依赖实际验证后选择版本，不在未经构建验证时凭空填写最低版本。
- Cargo 构建使用 --locked，npm 使用 ci；工作流使用工具链文件和同一 Node 版本来源。
- 保持现有三个平台和压缩包名称，校验和列出相应文件名。
- 在干净 staging 目录组装：二进制、custom_analyzers、平台脚本及公共辅助文件、脚本说明、LICENSE。排除 __pycache__、pyc、用户配置、数据库和克隆目录。
- 将文件清单集中到打包脚本，Unix 和 Windows 仅归档格式及脚本选择不同；不再在 workflow 内复制两套清单。
- 在包中附 `build-info.json`：version、commit、target、工具版本；不得包含令牌、环境变量全集或开发者绝对路径。

### 4.3 分析器资源和 Docker

- 分析器目录采用统一解析方法，由 scanner 和 test-analyzers 共用。
- 查找顺序：显式 `CODEPRISM_ANALYZERS_DIR` → 配置目录/custom_analyzers → 可执行文件目录/custom_analyzers → 可执行文件相邻 ../share/codeprism/custom_analyzers → 当前目录/custom_analyzers（兼容回退）。
- 显式目录不存在时直接报错；自动查找失败且所选配置需要脚本分析器时给出明确错误。不能显示分析成功却少执行分析器。
- 支持解析器测试传入候选路径，测试中不修改全局进程 cwd。
- 安装说明保留整套资源；系统安装将脚本放到 share/codeprism，不能只移动二进制而宣称功能完整。
- Docker 安装 Python 3，复制分析器至 `/usr/local/share/codeprism/custom_analyzers`，显式设置对应环境变量。数据工作目录保持 `/workspace`。
- Linux 二进制构建环境必须与运行镜像的 glibc 等 ABI 兼容；不能直接假定 ubuntu-latest 构建物可运行于 bookworm。固定兼容构建环境，或相应调整运行基础镜像并实际执行容器验证。
- Docker 至少运行 --version、init-config、check-config、自定义分析器自测；不能仅检查 docker build 成功。

### 4.4 发布流水线

目标依赖关系：

```text
解析 tag / 固定 commit / 校验版本
              ↓
质量检查 + 一次真实前端构建
              ↓
三平台 Rust 编译、打包及解压后验证
              ↓
Docker 构建及运行验证（先不推送）
              ↓
汇总校验和 → 发布 Release / 推送 Docker
```

- tag push 与手动输入走同一解析逻辑；手动发布要求 tag 已存在。所有 checkout 使用解析出的同一 commit SHA，而不是默认分支。
- 将用户输入放在环境变量或工具参数中，验证后使用；禁止直接嵌入 shell 命令。
- 前端 artifact 关联同一 commit，三平台下载后检查，Rust 编译时复用。
- 复用质量工作流或共享入口，发布必须依赖成功的检查；移除 matrix 中没有启用的 cross 分支。
- Release 增加 npm/Rust 缓存；缓存键包含工具链、平台、lockfile。按版本设置 concurrency，重复发布不并行覆盖产物。
- 全部平台包完成后，在新目录解压并验证资源、--version、init-config、check-config 和分析器执行；校验阶段不能借用源码树内分析器。
- 正式版本才更新 Docker latest；预发布仅发布版本标签。
- 所有本地验证通过后才产生外部发布副作用；Release 与 GHCR 无跨服务事务，部分发布失败须保留已验证产物并支持同 SHA 重跑，禁止重新解析成不同源码。
- 发布脚本和文档不得替用户执行 push/tag 或创建正式 Release。实际发布是独立的用户操作。

## 5. 分批实施任务

每个任务单独提交。不得把整份蓝图一次交给模型要求“全部完成”；依赖未完成时先做前置任务。任务范围外的发现写入交接记录。

### L01：项目内固定开发目录，一条命令启动

文件：根 package.json、scripts/dev.mjs、.gitignore、scanner 的分析器路径解析、CLI test-analyzers、三语言 README。

用户已确定采用最简单方案：**固定项目内隔离，不增加可配置 workspace、profile、多环境目录或数据迁移命令。** 不修改普通 CLI 的默认运行位置。

日常只需在项目根目录执行（Windows 相同）：

```sh
npm run dev
```

根 package.json 通过 `node scripts/dev.mjs dev` 分派，不再新增独立启动、初始化、前端、后端脚本。

固定布局：

```text
code-prism/
├── package.json                 # 四个统一命令
├── scripts/dev.mjs
├── custom_analyzers/            # 直接读取开发中的资源
├── target/debug/codeprism       # 直接执行，不复制；Windows 为 .exe
└── .codeprism-dev/              # 整目录加入 .gitignore
    ├── codeprism.yaml
    ├── codeprism.db             # 包含 SQLite 的 -wal、-shm 等运行文件
    └── cloned_repos/            # 包含仓库登记缓存
```

实施步骤：

1. 脚本根据自身路径定位源码根，在根目录执行增量 debug 构建，复用现有 build.rs 生成前端。首次缺少依赖或 package-lock.json 改变时执行 npm ci，成功后记录锁文件摘要于 `.codeprism-dev/`；安装失败不得更新摘要。
2. 创建固定 `.codeprism-dev/`。首次在此目录生成最小可用配置，数据库为 `sqlite:codeprism.db`；不复制根目录已有配置，不扫描或登记真实仓库。已有开发配置和数据库原样保留。
3. 用编译产物的绝对路径启动 serve，仅对子进程指定 cwd 为 `.codeprism-dev/`。使用现有配置和缓存默认规则，使数据库及克隆文件自然落在该目录，无需新加应用路径参数。不得在脚本主进程中全局 chdir。
4. 为子进程设置 `CODEPRISM_ANALYZERS_DIR` 为源码根的 custom_analyzers 绝对路径；scanner 的自动发现、显式脚本路径和 test-analyzers 共用该目录解析。此处只实现环境变量覆盖和当前目录兼容回退，B03 再扩展安装资源查找规则。环境变量显式指定的目录不存在时必须报错。
5. 同一 dev 命令还启动 web 中的 Vite，使用现有 /api 代理连接后端。前端修改热更新；Rust 修改后先采用停止并重跑 npm run dev，不新增 watch 工具。启动时打印 Vite 访问 URL、开发配置和数据库路径；端口冲突直接报错，不自动切换到代理不匹配的端口。Ctrl+C 或任一服务异常退出时清理整个子进程组，不遗留后台服务。
6. 启动前验证开发配置解析后的数据库文件在 `.codeprism-dev/` 内；若用户手动改成外部数据库，报错并说明该开发入口只允许隔离数据，不自动重写配置。
7. 本轮不新增 reset、清空或自动删除命令。需要全新手工环境时，文档说明停止服务后手动删除 `.codeprism-dev/`；这是用户手动操作，脚本不得自动执行。根目录既有数据保持不动。

验收：从源码根或其他目录执行同一脚本都使用同一个固定开发目录；连续启动保留配置和历史；源代码修改后增量构建再运行，无复制步骤；根目录不新增数据库、配置和克隆目录；分析器在新 cwd 下仍能加载；Windows 路径含空格可用；普通 CLI 用法不受影响。

B01/B02 在此脚本上完善 check/build/package，不覆盖已实现的 dev。日常只记 dev 和 check；构建与分发时才使用另两个命令。开发运行目录和删除保护中的缓存根分别为 `.codeprism-dev/` 与其 `cloned_repos/`，禁止误用源码根作为允许删除的缓存根。

### D01：先关闭现有隐式物理删除

文件：`git_routes.rs`、`routes.rs`、`web/src/services/data.ts`、`ExecutePage.tsx`、`ConfigPage.tsx`、三语言 JSON、相关服务端测试。

1. 仓库删除只移除登记，拒绝 remove_files=true；删除项目前暂时保留文件。
2. 原项目 DELETE 要求 confirmation_name；补齐请求调用和三语言确认，显示项目名、不可恢复范围和源码保留说明。
3. 删除流程不再调用后台 remove_dir_all，不再提前声称 files_removed=true。
4. UI 将仓库操作改名“移除登记”，移除磁盘文件复选框。

验收：默认移除和删除项目均保留临时测试仓库文件；旧危险参数和缺少确认名的请求被拒绝。此任务是过渡防护，不提前开放新的文件永久删除。

### D02：可信归属与登记持久化

依赖 D01。文件：`git_cache.rs`、`git_routes.rs`、`lib.rs`、`routes.rs`。

1. 启动补登记默认非托管；只由成功 clone 流程创建可信来源记录，未知旧数据保守处理。
2. 提取统一路径/引用校验，返回具体不可删除原因；列表返回来源、托管状态和能力字段。
3. 移除登记同时清除所有指向同一规范化目录的配置绑定；保留分析规则和历史。
4. cache save 改为原子写入、返回 Result；写入失败不报告成功。配置写入采用共享串行入口。

验收：重启不复活被移除的登记；本地路径即使在缓存目录内也不变成托管；根目录、根外目录、伪造记录和链接替换被拒绝。移除登记失败不丢失可重试信息。

### D03：归档和取消归档后端

依赖 D02。文件：core/lib.rs、server/config.rs、配置转换调用点、项目列表与配置路由、scan 入口、CLI。

1. 按第 3 节添加 archived 和 API；保留只读访问，活动列表默认排除。
2. 所有配置保存、复制、模板实例化都保留或正确初始化归档状态。
3. 处理 legacy 单项目格式：首次变更规范化为显式 projects；删除最后一个项目后不得由 legacy fallback 自动复活 default。须明确区分“旧格式没有 projects 字段”和“新格式 projects 为空”，不能只检查 Vec 是否为空。
4. 历史项目归档可创建最小配置；重命名、修改、扫描与归档使用同一项目门禁。

验收：归档与取消归档重启后保持；旧 YAML 正常读取；归档项目通过 CLI/API 均不能新扫描；最后一个项目删除后列表为空。

### D04：项目活动门禁及永久删除预览

依赖 D03。文件：state.rs、scan_scheduler.rs、扫描/配置/Git 路由；可新增 `project_lifecycle.rs` 和 `repo_safety.rs`。

1. 提供项目和仓库活动门禁，覆盖排队任务和正在运行的任务；不能仅查询一次 scan_jobs。
2. 实现预览接口，统计数据、列出路径、共享引用、工作区和未推送内容等 blockers。
3. 旧项目删除逻辑改为调用共同服务，避免 UI 和 API 两套权限判断。

验收：扫描与删除竞态下至多一方获得操作许可；pull/checkout/clone 期间不能删除；预览不修改任何状态；共享目录、未跟踪文件和独有提交均阻止物理删除。

### D05：可诊断、可重试的永久删除

依赖 D04。文件：database/init.sql、生命周期服务、项目 DELETE、服务启动检查、CLI 维护命令。

1. 添加持久化操作记录和第 3.5 节的明确阶段，不引入通用后台框架。
2. 实现确认、预检、可选文件删除、配置/DB 删除、内存更新和失败处理。
3. 文件删除默认 false；显式 true 才允许删通过保护检查的托管目录。
4. 实现启动受阻识别及服务停止时的显式重试；与正常服务共用进程锁。

验收：只删目标项目数据，保留其他项目和共享 match_contents；文件删除失败不销毁元数据；模拟每个持久化阶段中断后能识别未完成操作；部分删除可重试；操作完成才返回成功。

### D06：三档前端操作和文档

依赖 D03、D05。文件：ExecutePage.tsx、ConfigPage.tsx、AppContext.tsx、services/data.ts、三语言 JSON、三个 README。

1. 项目管理提供活动/归档筛选、归档及取消归档；归档历史只读可查看。
2. 永久删除先展示预览，输入原名后才允许提交；托管文件选项默认不勾选，不满足条件时显示原因。
3. 提交中禁用按钮，展示服务端错误及部分删除状态；失败不把项目从列表移除。
4. 请求复用现有 requestJson；更新成功后刷新上下文，当前项目被归档/删除时选择下一个活动项目或显示空状态。

验收：三档文案与第 3.1 节一致，没有回收站或独立清理入口；三语言无缺失键；直接调用 API 不能绕过保护。

### B01：版本与工具链统一

可与 D 系列独立实施。文件：所有 Cargo.toml、Cargo.lock、根 package.json、web/package*.json、新工具链文件、scripts/dev.mjs。

验收：Rust 版本单处维护；版本更新命令同步 lockfile；校验能发现前端和 tag 不一致；不提交、不打标签；锁定工具版本经实际构建验证。

### B02：统一构建和打包

依赖 B01。文件：根 package.json、scripts/dev.mjs、verify-release-payload.py、server/build.rs、三语言 README。

验收：从干净环境按文档执行 package 生成完整本机包及校验和；从其他 cwd 调用也可用；npm/Cargo/校验失败即非零退出；不修改用户配置和数据库；prebuilt 不匹配时报错。

### B03：资源解析与 Docker 运行验证

依赖 B02。文件：scanner/lib.rs、CLI、共享资源解析模块、Dockerfile、打包脚本、README。

验收：解压包移到新目录且从其他 cwd 启动，仍能执行内置脚本；自定义资源覆盖有明确优先级；Docker 能实际运行二进制和分析器；没有 glibc 运行错误。

### B04：发布流水线收敛

依赖 B02、B03。文件：.github/workflows/ci.yml、release.yml、必要的共享工作流。

验收：发布 DAG 符合第 4.4 节；手动 tag 与所有 checkout SHA 一致；真实前端只构建一次；测试失败不发布；预发布不改 latest；三平台包解压验证成功；重复发布按同一 SHA 处理。

### V01：最终验收和交接

依赖全部任务（含 L01）。汇总下节证据；更新 README 的本地开发、安装、构建、三档操作、API 和不可恢复说明。移除过渡文案，记录剩余平台验证缺口。

禁止在这一步顺带升级依赖、重排无关文件或执行正式发布。

## 6. 验证矩阵

所有删除测试使用临时目录、临时 SQLite 和临时 YAML；不得指向仓库现有 codeprism.db、codeprism.yaml、cloned_repos 或用户真实项目。

| 类别 | 必测情景 |
| --- | --- |
| 本地开发 | 固定 .codeprism-dev；首次生成与重复保留；其他 cwd 启动；外部数据库拒绝；源码分析器加载；无复制；退出不留后台服务 |
| 移除登记 | 文件和扫描历史保留；配置解除路径绑定；重启不重新登记；重新登记可用 |
| 归档 | 归档/取消归档幂等并可持久化；默认隐藏；只读历史；CLI/API 写操作受限 |
| 兼容 | 旧 managed 缺省值；缓存损坏；旧 YAML；历史项目无配置；删除最后一个项目；新模板不带归档状态 |
| 永久删除默认 | 精确确认名；仅目标数据被删；全部源码保留；未知对象 404；缺少确认 400 |
| 托管文件删除 | 本地/未知来源拒绝；正常可信克隆可删；共享引用/根目录/链接替换/脏工作区/未推送提交拒绝 |
| 并发 | queued/running scan、clone/pull/checkout、配置修改、重复删除；请求重入不越过门禁 |
| 失败 | 缓存写失败、YAML 替换失败、DB commit 失败、目录删除失败、各阶段重启；不得假成功 |
| 构建 | 干净克隆、无 npm、缺依赖、前端失败、dist 缺文件、dist 改动触发重嵌入、版本漂移 |
| 包与运行 | 三平台解压、校验和、资源无字节码、外部 cwd、路径含空格、Python 自测、真实 HTML/JS 请求 |
| 发布 | 输入 tag 不存在/版本不符、手动 SHA 一致、质量失败阻断、预发布 latest、Docker 运行兼容 |

常规检查通过统一 check 入口运行，内部至少包括：

```sh
npm --prefix web ci
npm --prefix web run lint
npm --prefix web test
npm --prefix web run build
cargo fmt --all -- --check
# 以下由统一脚本以子进程环境设置 CODEPRISM_SKIP_WEB_BUILD=1
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run --locked -- test-analyzers
```

按变更运行有关测试；最终执行全量检查。没有 Windows/Linux runner 时如实记录“未运行”，不能用 macOS 通过推断另外两平台通过。故障注入使用测试替身或临时资源，不制造真实用户数据损坏。

开发脚本数量约束：只新增 scripts/dev.mjs 一个编排脚本，所有本地流程通过根 package.json 的四个命令使用。原 scripts/verify-release-payload.py 已并入 Node 校验子命令；Python 仍用于运行现有 Python 分析器测试，不承担开发命令编排。现有 codeprism-scan 等属于产品附带 CLI 工具，不要求开发者记忆，也不在本轮删除或重写。不得为每个步骤继续新增 dev.sh、build.py、start.ps1 等入口。

当前验证记录：本机 macOS 已通过 `npm run check` 和 `npm run package`；Docker 实际运行、Windows/Linux 原生 runner、tag 发布全流程尚未在本机执行，需由 GitHub Actions 验证。开发服务器端口绑定受当前环境限制，`.codeprism-dev/` 后端与 Vite 联调仍待允许本地监听的环境验收。

## 7. 交给实施模型的执行提示词

复制下面模板，每次仅替换一个任务编号：

```text
阅读 docs/implementation-blueprint.zh-CN.md，执行任务 <D01>。
先检查工作区及适用的 AGENTS.md，再阅读该任务直接相关文件。
严格遵守第 3、4 节及 L01 契约；只完成该任务和必要测试，不提前实施后续任务。
保留已有用户改动，不运行正式发布，不操作真实仓库、配置或数据库。
不引入回收站、定期清理、独立永久删除仓库入口或 force 绕过保护。
如果前置任务未完成，报告依赖缺口，不自行跳过。
关键契约无法实现时明确说明，不用 TODO、忽略错误或假成功伪装完成。
完成后汇报：修改文件、行为变化、运行的检查及结果、未验证的平台、后续依赖。
未被明确要求时不创建 commit；由负责人审核后独立提交。
```

推荐先完成 L01，以便后续手工验证不污染源码目录；然后 D01 → D02 → D03 → D04 → D05 → D06；B01 → B02 → B03 → B04；最后 V01。一次仅让一个实施模型修改共享的配置写入、状态、路由和 scripts/dev.mjs，避免相互覆盖。

每次交接记录任务编号、实际变更、验证命令、结果和未完成项。不以“代码已写”替代验收通过，不以单个单元测试替代三平台发布验证。
