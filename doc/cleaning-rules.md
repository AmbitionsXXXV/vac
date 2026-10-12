# 清理规则总览（对照 Mole）

本文对照 [Mole](https://github.com/tw93/Mole)（bash 实现的 macOS 清理工具）的规则目录，列出 VAC 已覆盖的规则、已知缺陷和可以补的规则。Mole 引用以其仓库根目录为基准，形如 `lib/clean/dev.sh:5714`，核对时间 2026-10-12（Mole `49475f1d`）。

## 动作类型

Mole 把规则分成四种动作，VAC 目前只有第一种：

| 动作 | 含义 | VAC |
|---|---|---|
| 直接删除 | 删除目录内容或文件 | ✅ 全部规则 |
| sudo 删除 | `/Library`、`/private/var` 下的系统路径 | ❌ 不支持，安全检查拒绝 |
| 委托工具 | 调用工具自身命令，如 `brew cleanup`、`go clean -modcache`、`xcrun simctl delete unavailable` | ❌ |
| 仅提示 | 只报告体积和建议命令，从不删除，如 `brew autoremove`、Docker、模拟器 runtime | ❌ |

## VAC 现有规则与 Mole 的差异

| VAC 分类 | VAC 路径 | Mole 做法 | 差异 |
|---|---|---|---|
| 系统缓存 | `~/Library/Caches` 整体 | 逐个顶层子项删除，跳过受保护 bundle、正在运行的 app 和打开中的 SQLite（`lib/clean/user.sh:250-308`、`lib/core/file_ops.sh:1258`） | VAC 无进程和保护名单检查 |
| 日志 | `~/Library/Logs` | 同范围，额外拒绝祖先目录是符号链接的路径（`user.sh:193-249`） | 基本一致 |
| 临时文件 | `/tmp`、`/var/tmp` | 系统级路径需 sudo，且按 7 天年龄过滤 | 已修复：`/var/tmp` 规范化后是 `/private/var/tmp`，此前被安全检查拒绝；VAC 不按年龄过滤 |
| 下载文件夹 | `~/Downloads` 整体 | 只删未完成下载（`.download`/`.crdownload`/`.part`），并用 `lsof` 确认未被打开（`user.sh:344-415`） | VAC 范围远大于 Mole，依赖用户确认 |
| 垃圾桶 | `~/.Trash` | 同范围，需完全磁盘访问权限（`user.sh:40-157`） | 一致 |
| Xcode 派生数据 | `~/Library/Developer/Xcode/DerivedData` | 按项目删除，Xcode/构建进程运行时跳过（`lib/clean/app_caches.sh:100-266`） | VAC 无进程检查 |
| Homebrew 缓存 | `~/Library/Caches/Homebrew/downloads` | 只删 `downloads/*`（`lib/clean/dev.sh:5710-5714`） | 已与 Mole 对齐；此前连 `api/` 和 `bootsnap/` 一起删，下次 brew 命令约 90 秒无输出。注意“系统缓存”分类仍覆盖整个 `~/Library/Caches`，选中它时 Homebrew 的 `api/` 也会被删 |
| Homebrew 暂存 | `<前缀>/var/homebrew/tmp/.cellar`、`.caskroom` | Mole 没有单独规则，由 `brew cleanup` 的 `cleanup_temp_staging` 覆盖（`lib/clean/brew.sh:99-413`） | VAC 直接删除，brew 运行时跳过 |
| CocoaPods | `~/Library/Caches/CocoaPods` | 被 `~/Library/Caches/*` 规则覆盖 | 一致 |
| npm | `~/.npm/_cacache` | `npm cache clean --force`，再删残留子目录（`dev.sh:451-490`） | 结果接近 |
| pip | `~/Library/Caches/pip` | `pip3 cache purge`（`dev.sh:92-102`） | 结果接近 |
| Docker 数据 | `~/Library/Containers/com.docker.docker/Data` | **仅提示**，只显示 `docker system df`（`dev.sh:1324-1350`） | **风险**：VAC 会删除容器和镜像，Mole 认为不可证明可再生 |
| Cargo | `~/.cargo/registry/cache` | 同路径加 `$RUSTUP_HOME/downloads/*`，cargo/rustc 运行时跳过（`dev.sh:1137-1262`） | VAC 无进程检查 |

## Mole 有而 VAC 没有的规则

按收益和实现难度排序，前几项与 VAC 现有模型（目录 + 直接删除）最贴合。

| 优先级 | 规则 | Mole 路径或命令 | 防护 | 来源 |
|---|---|---|---|---|
| 高 | 前端工具缓存 | `~/.cache/{typescript,electron,node-gyp,vite,webpack,eslint,prettier}`、`~/.turbo/cache`、`~/.parcel-cache` | 无 | `dev.sh:1377-1388` |
| 高 | Bun / Yarn / pnpm | Bun 缓存、`~/.yarn/cache`、`~/Library/Caches/Yarn`；pnpm 用 `pnpm store prune` | pnpm 运行时跳过 | `dev.sh:557-572`、`:315-445` |
| 高 | uv / Go | `uv cache prune`；`go clean -modcache -cache` | Go 进程检查 | `dev.sh:274-300`、`:830-1072` |
| 高 | Xcode 设备支持 | `~/Library/Developer/Xcode/{iOS,watchOS,tvOS} DeviceSupport/*`，保留最新 2 个 | Xcode 运行时不删 | `dev.sh:2064-2365` |
| 高 | 模拟器 | `CoreSimulator/Caches/*`；`xcrun simctl delete unavailable` | 模拟器进程检查 | `app_caches.sh:267-300`、`dev.sh:2662-2870` |
| 中 | `brew cleanup` | `brew cleanup --prune=30`，7 天内运行过则跳过 | 删除前快照并恢复活跃链接 | `brew.sh:99-413` |
| 中 | Homebrew 服务日志 | `<前缀>/var/log` 下 7 天前的 `*.log`、`*.gz` | 属主、单链接、`lsof` 未打开 | `brew.sh:415-650` |
| 中 | 浏览器缓存 | Chrome/Edge/Arc/Brave 等配置文件下的 `Code Cache`、`GPUCache`、`Service Worker/CacheStorage` | 浏览器运行时跳过 | `user.sh:1805-2014`、`lib/clean/caches.sh:49-200` |
| 中 | 编辑器缓存 | VS Code / Cursor 的 Cache、CachedData、CachedExtensionVSIXs，`.obsolete` 列出的旧扩展 | 只删 `.obsolete` 中值为 true 的直接子目录 | `app_caches.sh:381-485` |
| 中 | 沙盒应用缓存 | `~/Library/Containers/*/Data/Library/Caches/*` | 跳过关键和数据保护 bundle、符号链接 | `user.sh:1237-1330` |
| 中 | 项目构建产物 | `node_modules`、`target`、`.venv` 等 34 种，限 `~/Projects`、`~/GitHub` 等根目录 | 须在项目根下；含 git 跟踪文件的跳过；7 天内修改的默认不选 | `lib/clean/purge_shared.sh:15-66`、`lib/clean/project.sh` |
| 低 | 孤儿应用数据 | 已卸载 app 留下的 `~/Library/Caches/<bundle-id>`、`Saved Application State` | `mdfind` 找不到 app 且 30 天未动；每类最多 100 个 | `lib/clean/apps.sh:450-892` |
| 低 | iOS 固件 | `~/Library/iTunes/*Software Updates/*.ipsw` | 白名单 | `user.sh:2615-2700` |
| 低 | 系统级清理 | `/Library/Caches`、`/private/var/log` 等 7 天前的文件 | 需 sudo，限 120 秒 | `lib/clean/system.sh` |
| 仅提示 | 大体积不可再生数据 | Docker、Time Machine 快照、`brew autoremove`、多余 Rust 工具链和 NDK、iOS 备份、Hugging Face 模型 | 从不删除 | `user.sh:3023-3373` |

## Mole 安全模型中 VAC 缺少的部分

1. **进程检查**：Mole 在删除前确认所属 app 没有运行，SQLite 文件有打开句柄时拒绝删除（`lsof` 无法判断时按打开处理）。VAC 只有 Homebrew 暂存这一处做了进程检查。
2. **年龄阈值**：系统日志和临时文件 7 天，孤儿数据 30 天，GPU 缓存 1 天（`lib/core/base.sh:148-158`）。VAC 删除整个目录内容，不看修改时间。
3. **保护名单**：`SYSTEM_CRITICAL_BUNDLES`（129 项）和 `DATA_PROTECTED_BUNDLES`（352 项，含输入法、密码管理器、VPN）位于 `lib/core/app_protection_data.sh:19`、`:207`。VAC 清理 `~/Library/Caches` 时不区分这些。
4. **用户白名单**：`~/.config/mole/whitelist` 支持精确和 glob 匹配。VAC 配置只能追加扫描目标，不能排除路径。
5. **删除前复查**：Mole 在删除前一刻对目标和父目录做 inode 身份复查，`find` 超时时丢弃部分结果，不删除前缀。
6. **sudo 路径额外约束**：拒绝符号链接，拒绝父目录非 root 所有或组/全局可写的路径（`file_ops.sh:1917-1990`）。VAC 不支持 sudo，暂无需此项。

## 建议顺序

1. ~~修复 `/var/tmp` 被拒绝、Homebrew 缓存范围过大~~（已完成）。
2. 把 Docker 数据改为仅提示，或者在确认弹窗里单独警告。
3. 增加“仅提示”这一类条目，用于 Docker、`brew autoremove` 这类 VAC 不应直接删除的内容。
4. 再补高优先级的开发者缓存规则，每条规则配进程检查。
