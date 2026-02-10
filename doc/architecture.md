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
    ├── main.rs         # 程序入口、事件循环、键盘处理、CLI 非交互模式
    ├── lib.rs          # 库入口，模块导出
    ├── app.rs          # 应用状态管理
    ├── cli.rs          # CLI 参数定义（clap）
    ├── config.rs       # 配置文件加载与解析
    ├── ui.rs           # UI 渲染
    ├── scanner.rs      # 磁盘扫描器
    ├── cleaner.rs      # 文件清理器
    └── utils.rs        # 共享工具函数（时间格式化、路径展开）
```

## 模块说明

### cli.rs - CLI 参数定义

使用 `clap` (derive 模式) 定义命令行参数：

- `Cli`: 顶层 CLI 参数结构
  - `--scan <MODE_OR_PATH>`: 非交互扫描，可选值 `preset`（预设目录）、`home`（主目录）、或指定路径
  - `--dry-run`: 仅模拟删除，不执行实际清理
  - `--clean`: 执行清理（清理扫描到的所有项目）
  - `--output <FILE>`: 将结果输出为 JSON 文件
  - `--sort <ORDER>`: 排序方式（name / size / time），默认 size
  - `--trash`: 使用回收站而非永久删除（覆盖配置文件设置）
- `ScanTarget`: 扫描目标枚举（Preset / Home / Path）
- `Cli::is_non_interactive()`: 判断是否为非交互模式

无参数启动时进入 TUI 交互界面；传入 `--scan` 参数后进入非交互模式直接输出结果。

### config.rs - 配置文件管理

从 `~/.config/vac/config.toml` 加载用户配置：

- `AppConfig`: 顶层配置结构，包含 `ScanConfig`、`UiConfig` 和 `SafetyConfig`
- `ScanConfig`: 扫描配置，支持 `extra_targets` 额外扫描目标（支持 `~` 展开）
- `UiConfig`: UI 配置，支持 `default_sort` 设定默认排序方式
- `SafetyConfig`: 安全配置，支持 `move_to_trash` 设定是否移至回收站（默认 false）
- `AppConfig::load()`: 从配置文件加载，文件不存在或解析失败时返回默认值
- `AppConfig::expanded_extra_targets()`: 展开 `~` 并过滤不存在的路径

使用 `serde` + `toml` crate 进行反序列化，所有字段均有 `#[serde(default)]` 标注以支持部分配置。

### utils.rs - 共享工具

跨模块复用的公共函数与常量：

- `expand_tilde(path)`: 统一将 `~` 展开为主目录绝对路径
- `format_time(time, include_time)`: 统一时间格式化
  - `include_time = false` 输出 `YYYY-MM-DD`
  - `include_time = true` 输出 `YYYY-MM-DD HH:MM:SS`
- 时间计算常量：`SECONDS_PER_DAY`、`EPOCH_YEAR`

`app.rs`、`cli.rs`、`config.rs`、`main.rs`、`ui.rs` 均通过该模块复用路径与时间逻辑，避免重复实现。

### app.rs - 应用状态管理

核心数据结构：

- `App`: 应用主状态，包含模式、条目列表、扫描进度、选择状态、搜索状态、dry-run 状态、Tab 补全状态等
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

- `sort_entries_by(entries, order)`: 通用排序函数（按名称/大小/时间）
- `sort_root_entries()`: 根层条目排序，支持 ByName / BySize / ByTime 三种方式
- `sort_dir_entries()`: 目录条目排序，支持三种方式
- `toggle_sort_order()`: 循环切换排序方式（名称 → 大小 → 时间 → 名称），自动区分根目录和子目录场景
- `restore_root_entries()`: 恢复根目录视图时重新应用当前排序

排序方式在整个 session 中保持一致：用户通过 `o` 键设置的排序方式会在进入子目录、返回上级目录、返回根目录时持续生效，无需重复切换。

搜索方法：

- `start_search()`: 进入搜索模式，保存原始条目
- `search_char()` / `search_backspace()`: 实时过滤
- `confirm_search()` / `cancel_search()`: 确认或恢复

路径输入与 Tab 补全：

- `start_input()` / `cancel_input()`: 进入/退出路径输入模式
- `input_char()` / `input_backspace()`: 路径输入编辑，编辑时自动重置补全状态
- `confirm_input()`: 确认输入并返回展开后的路径
- `input_tab_complete()`: Tab 正向补全/循环，根据当前 `input_buffer` 列出匹配目录
- `input_tab_complete_prev()`: Shift+Tab 反向循环候选项
- `reset_tab_completions()`: 清空补全状态（`tab_completions` 和 `tab_completion_index`）
- `expand_input_tilde()`: 将路径中的 `~` 展开为主目录绝对路径
- `build_tab_completions()`: 内部方法，读取文件系统构建候选列表，只匹配目录，保留 `~` 前缀显示
  - `parse_path_input()`: 解析父目录和补全前缀
  - `read_matching_dirs()`: 读取并过滤匹配目录
  - `build_completion_display_path()`: 生成最终显示路径

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
- `render_input_popup()`: 路径输入弹窗（含 Tab 补全候选列表高亮显示，最多展示 5 个候选项）
- `render_search_bar()`: 搜索栏
- `render_error_popup()`: 错误弹窗（仅 Enter/Esc 可关闭）
- `styled_block()` / `help_line()` / `path_short_name()`: 通用 UI 复用辅助函数
- 时间显示统一复用 `utils::format_time()`

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

实现中包含两个去重辅助函数：

- `add_target_if_exists()`: 统一处理条件目标追加
- `is_cancelled()`: 统一处理取消代次检查

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

回收站支持：

- `Cleaner::trash_items(items)`: 将选中项移至系统回收站而非永久删除
- 对目录：移动目录内容至回收站，保留目录本身
- 对文件：直接移至回收站
- 使用 `trash` crate 调用系统原生回收站 API

清理循环通过 `process_items()` 统一，`clean()` 与 `trash_items()` 只保留策略差异。安全检查中的禁止路径列表由模块级常量 `FORBIDDEN_PATHS` 统一维护，并被测试复用。

### main.rs - 事件循环与 CLI 入口

- 启动时使用 `clap` 解析 CLI 参数
- 若传入 `--scan` 参数，进入非交互模式：同步扫描 → 排序 → 输出结果（终端或 JSON 文件）
- 非交互模式支持 `--dry-run`（模拟删除）、`--clean`（执行清理）、`--trash`（移至回收站）
- 无参数启动时加载 `AppConfig` 配置文件，进入 TUI 交互界面
- 事件轮询间隔根据扫描状态动态调整（扫描中 16ms / 空闲 100ms）
- 支持 Ctrl+d/u 等组合键通过 `KeyModifiers` 判断
- 分离了各模式（Normal、Confirm、InputPath、Search、Scanning、Help、Stats）的键盘处理逻辑
- `execute_clean()` 根据 `config.safety.move_to_trash` 选择 trash 或永久删除
- `spawn_scan_thread()` 统一封装扫描线程启动流程
- 非交互模式排序复用 `app::sort_entries_by()`，时间格式化复用 `utils::format_time()`

## 技术栈

- **ratatui**: TUI 框架
- **crossterm**: 跨平台终端操作
- **color-eyre**: 错误处理
- **walkdir**: 目录遍历
- **bytesize**: 字节大小格式化
- **directories**: 系统目录获取
- **rayon**: 并行计算（目录大小）
- **serde**: 序列化/反序列化框架
- **serde_json**: JSON 序列化（CLI 报告输出）
- **toml**: TOML 配置文件解析
- **clap**: 命令行参数解析（derive 模式）
- **trash**: 系统回收站 API（移至回收站功能）

## 版本管理与 Changelog

- 使用 [git-cliff](https://git-cliff.org/) 自动从 commit 历史生成 `CHANGELOG.md`
- 配置文件 `cliff.toml` 定制了 commit message 解析规则，匹配项目 `type: emoji description` 风格
- commit body 中的逐项说明会自动展开为子列表
- 生成命令：`git-cliff --tag <version> -o CHANGELOG.md`
- 追加新版本：`git-cliff --tag <version> --unreleased --prepend CHANGELOG.md`

## 状态流转

### TUI 交互模式

```text
[启动] → 解析 CLI 参数 → 无 --scan → 加载配置 → Normal (欢迎页)
         ↓ 's'/'S'
       Scanning ←───────────────┐
            ↓ 完成              │
          Normal                │
            ↓ Enter             │
       浏览目录                 │
         ↓ 'o'                  │
       切换排序 (名称/大小/时间)│
         ↓ 'd'                  │
       InputPath ───────────────┘
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
       清理 (trash/永久删除) → 通知释放空间
         ↓ 't'
       Stats (统计面板)
         ↓ any key
       Normal
         ↓ '?'
        Help
         ↓ any key
       Normal
```

### CLI 非交互模式

```text
[启动] → 解析 CLI 参数 → 有 --scan → 加载配置
         ↓
       同步扫描 → 排序
         ↓
       [--dry-run?] → 模拟删除统计
         ↓
       [--clean?] → 执行清理 (--trash 则移至回收站)
         ↓
       [--output?] → 输出 JSON 文件
         ↓
       终端输出结果 → 退出
```
