# VAC 架构文档

## 概述

VAC (Vacuum) 是一个基于 ratatui 的 macOS 磁盘清理 CLI 工具，采用 TUI (Terminal User Interface) 界面。

## 项目结构

```text
vac/
├── Cargo.toml          # 项目配置和依赖
├── doc/                # 文档目录
│   ├── architecture.md # 架构文档
│   └── usage.md        # 使用说明
└── src/
    ├── main.rs         # 程序入口、事件循环、键盘处理
    ├── lib.rs          # 库入口，模块导出
    ├── app.rs          # 应用状态管理
    ├── config.rs       # 配置文件加载与解析
    ├── ui.rs           # UI 渲染
    ├── scanner.rs      # 磁盘扫描器
    └── cleaner.rs      # 文件清理器
```

## 模块说明

### config.rs - 配置文件管理

从 `~/.config/vac/config.toml` 加载用户配置：

- `AppConfig`: 顶层配置结构，包含 `ScanConfig` 和 `UiConfig`
- `ScanConfig`: 扫描配置，支持 `extra_targets` 额外扫描目标（支持 `~` 展开）
- `UiConfig`: UI 配置，支持 `default_sort` 设定默认排序方式
- `AppConfig::load()`: 从配置文件加载，文件不存在或解析失败时返回默认值
- `AppConfig::expanded_extra_targets()`: 展开 `~` 并过滤不存在的路径

使用 `serde` + `toml` crate 进行反序列化，所有字段均有 `#[serde(default)]` 标注以支持部分配置。

### app.rs - 应用状态管理

核心数据结构：

- `App`: 应用主状态，包含模式、条目列表、扫描进度、选择状态、搜索状态、dry-run 状态等
- `Mode`: 应用运行模式 (Normal, Scanning, Confirm, Help, InputPath, Search, Stats)
- `SortOrder`: 排序方式 (ByName, BySize, ByTime)
- `EntryKind`: 条目类型（目录/文件）
- `ItemCategory`: 扫描项分类（系统缓存、日志、临时文件、下载、垃圾桶、Xcode、Homebrew、CocoaPods、npm、pip、Docker、Cargo、自定义目标等）
- `CleanableEntry`: 当前视图条目（含 `modified_at` 时间字段）
- `SelectedEntry`: 已选条目元数据
- `NavigationState`: 导航状态（当前路径、带缓存的导航栈）
- `NavFrame`: 导航栈帧，保存路径、条目快照和滚动位置

构造方法：

- `App::new()`: 使用默认配置创建
- `App::with_config(config)`: 根据配置文件创建，应用默认排序等设置

导航方法：

- `next()` / `previous()`: 单步移动
- `first()` / `last()`: 跳到首/末项
- `page_down()` / `page_up()`: 翻半页（接受可视高度参数）

目录导航缓存：

- `enter()`: 进入子目录时，将当前条目列表和滚动位置保存到导航栈帧中
- `back()`: 回退时直接从栈帧恢复缓存的条目和滚动位置，无需重新扫描磁盘
- 回退恢复缓存后会重新按当前 `sort_order` 排序，保持排序指示与列表顺序一致
- 回退到根目录时通过 `restore_root_entries()` 从 `root_entries` 缓存恢复

排序方法：

- `sort_root_entries()`: 根层条目排序，支持 ByName / BySize / ByTime 三种方式
- `sort_dir_entries()`: 目录条目排序，支持三种方式
- `toggle_sort_order()`: 循环切换排序方式（名称 → 大小 → 时间 → 名称），自动区分根目录和子目录场景
- `restore_root_entries()`: 恢复根目录视图时重新应用当前排序

排序方式在整个 session 中保持一致：用户通过 `o` 键设置的排序方式会在进入子目录、返回上级目录、返回根目录时持续生效，无需重复切换。

搜索方法：

- `start_search()`: 进入搜索模式，保存原始条目
- `search_char()` / `search_backspace()`: 实时过滤
- `confirm_search()` / `cancel_search()`: 确认或恢复

统计方法：

- `toggle_stats()`: 切换统计面板显示（仅在有根扫描数据时可用）
- `get_category_stats()`: 按分类聚合 `root_entries`，返回分类名和总大小列表

### ui.rs - UI 渲染

使用 ratatui 渲染 TUI 界面：

- `render()`: 主渲染函数，协调头部、主体、底部和弹窗
- `render_header()`: 头部标题、路径与统计信息（总计条目数、已选条目数）
- `render_main()`: 主内容区（列表或扫描进度）
- `render_list()`: 列表渲染，含空状态欢迎页、滚动条、修改时间显示
- `render_scanning()`: 扫描进度条（显示已发现的可释放空间）
- `render_footer()`: 底部快捷键提示 + 清理完成通知
- `render_help_popup()`: 帮助弹窗
- `render_confirm_popup()`: 可滚动预览的确认删除弹窗，支持 Dry-run 视图切换
- `render_dry_run_view()`: Dry-run 详情视图（文件数/目录数/大小）
- `render_stats_popup()`: 空间占用统计面板（按分类展示进度条）
- `render_input_popup()`: 路径输入弹窗
- `render_search_bar()`: 搜索栏
- `render_error_popup()`: 错误弹窗（仅 Enter/Esc 可关闭）
- `format_time()`: 将 SystemTime 格式化为 YYYY-MM-DD 字符串

### scanner.rs - 磁盘扫描器

支持三种扫描模式：

1. **Root 扫描**：扫描 macOS 常见可清理目录（含 CocoaPods、npm、pip、Docker、Cargo 缓存）及配置文件中的额外目标
2. **ListDir 扫描**：列出指定目录的内容（用于目录浏览）
3. **DiskScan 扫描**：扫描指定路径的顶层目录/文件并计算大小

构造方法：

- `Scanner::new()`: 基础创建
- `Scanner::with_extra_targets(extra_targets)`: 带额外扫描目标创建（从配置文件获取）
- `scanner_from_config(config)`: 根据 AppConfig 创建 Scanner 的便捷工厂函数

扫描时会读取文件/目录的最后修改时间（`metadata.modified()`），支持按时间排序。

异步扫描通过 `mpsc::channel` 发送进度消息。目录大小计算使用 **rayon** 并行处理，显著提升多目录场景的扫描速度。所有 `WalkDir` 遍历均设置 `follow_links(false)` 避免符号链接循环。

消息类型：

- `ScanMessage::Progress` - 进度更新
- `ScanMessage::RootItem` - 根目录扫描条目
- `ScanMessage::DirEntry` - 目录条目
- `ScanMessage::DirEntrySize` - 目录大小回填
- `ScanMessage::Done` - 全部完成
- `ScanMessage::Error` - 扫描出错

### cleaner.rs - 文件清理器

安全清理选中的文件/目录：

- 使用 `Path::canonicalize()` 解析符号链接后做路径安全检查
- 禁止删除系统关键目录和用户根目录本身
- 仅允许用户目录子路径和临时目录
- 保留目录结构，仅清理内容
- 错误收集和报告

Dry-run 支持：

- `Cleaner::dry_run(items)`: 不执行删除，遍历统计每个选中项的文件数、目录数和总大小
- `DryRunResult`: 包含总计和每项的 `DryRunItem` 详情
- `count_path_contents()`: 内部方法，使用 WalkDir 遍历并计数

### main.rs - 事件循环

- 启动时加载 `AppConfig` 配置文件，传递给 `App` 和 `Scanner`
- 事件轮询间隔根据扫描状态动态调整（扫描中 16ms / 空闲 100ms）
- 支持 Ctrl+d/u 等组合键通过 `KeyModifiers` 判断
- 分离了各模式（Normal、Confirm、InputPath、Search、Scanning、Help、Stats）的键盘处理逻辑

## 技术栈

- **ratatui**: TUI 框架
- **crossterm**: 跨平台终端操作
- **color-eyre**: 错误处理
- **walkdir**: 目录遍历
- **bytesize**: 字节大小格式化
- **directories**: 系统目录获取
- **rayon**: 并行计算（目录大小）
- **serde**: 序列化/反序列化框架
- **toml**: TOML 配置文件解析

## 版本管理与 Changelog

- 使用 [git-cliff](https://git-cliff.org/) 自动从 commit 历史生成 `CHANGELOG.md`
- 配置文件 `cliff.toml` 定制了 commit message 解析规则，匹配项目 `type: emoji description` 风格
- commit body 中的逐项说明会自动展开为子列表
- 生成命令：`git-cliff --tag <version> -o CHANGELOG.md`
- 追加新版本：`git-cliff --tag <version> --unreleased --prepend CHANGELOG.md`

## 状态流转

```text
[启动] → 加载配置 → Normal (欢迎页)
         ↓ 's'/'S'
       Scanning ←───────────┐
         ↓ 完成              │
       Normal                │
         ↓ Enter             │
       浏览目录               │
         ↓ 'o'               │
       切换排序 (名称/大小/时间)│
         ↓ 'd'               │
       InputPath ────────────┘
         ↓ Enter (输入路径)
       Scanning
         ↓ 完成
       Normal
         ↓ '/'
       Search
         ↓ Enter/Esc
       Normal
         ↓ 'c'
       Confirm (可滚动预览)
         ↕ 'd' (切换 Dry-run 视图)
         ↑ Esc
         ↓ Enter
       清理并刷新 → 通知释放空间
         ↓ 't'
       Stats (统计面板)
         ↓ any key
       Normal
         ↓ '?'
        Help
         ↓ any key
       Normal
```
