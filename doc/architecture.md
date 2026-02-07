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
    ├── ui.rs           # UI 渲染
    ├── scanner.rs      # 磁盘扫描器
    └── cleaner.rs      # 文件清理器
```

## 模块说明

### app.rs - 应用状态管理

核心数据结构：

- `App`: 应用主状态，包含模式、条目列表、扫描进度、选择状态、搜索状态等
- `Mode`: 应用运行模式 (Normal, Scanning, Confirm, Help, InputPath, Search)
- `SortOrder`: 排序方式 (ByName, BySize)
- `EntryKind`: 条目类型（目录/文件）
- `ItemCategory`: 扫描项分类（系统缓存、日志、临时文件、下载、垃圾桶、Xcode、Homebrew、CocoaPods、npm、pip、Docker、Cargo 等）
- `CleanableEntry`: 当前视图条目
- `SelectedEntry`: 已选条目元数据
- `NavigationState`: 导航状态（当前路径、路径栈）

导航方法：

- `next()` / `previous()`: 单步移动
- `first()` / `last()`: 跳到首/末项
- `page_down()` / `page_up()`: 翻半页（接受可视高度参数）

搜索方法：

- `start_search()`: 进入搜索模式，保存原始条目
- `search_char()` / `search_backspace()`: 实时过滤
- `confirm_search()` / `cancel_search()`: 确认或恢复

### ui.rs - UI 渲染

使用 ratatui 渲染 TUI 界面：

- `render()`: 主渲染函数，协调头部、主体、底部和弹窗
- `render_header()`: 头部标题、路径与统计信息（总计条目数、已选条目数）
- `render_main()`: 主内容区（列表或扫描进度）
- `render_list()`: 列表渲染，含空状态欢迎页和滚动条
- `render_scanning()`: 扫描进度条（显示已发现的可释放空间）
- `render_footer()`: 底部快捷键提示 + 清理完成通知
- `render_help_popup()`: 帮助弹窗
- `render_confirm_popup()`: 可滚动预览的确认删除弹窗
- `render_input_popup()`: 路径输入弹窗
- `render_search_bar()`: 搜索栏
- `render_error_popup()`: 错误弹窗（仅 Enter/Esc 可关闭）

### scanner.rs - 磁盘扫描器

支持三种扫描模式：

1. **Root 扫描**：扫描 macOS 常见可清理目录（含 CocoaPods、npm、pip、Docker、Cargo 缓存）
2. **ListDir 扫描**：列出指定目录的内容（用于目录浏览）
3. **DiskScan 扫描**：扫描指定路径的顶层目录/文件并计算大小

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

### main.rs - 事件循环

- 事件轮询间隔根据扫描状态动态调整（扫描中 16ms / 空闲 100ms）
- 支持 Ctrl+d/u 等组合键通过 `KeyModifiers` 判断
- 分离了各模式（Normal、Confirm、InputPath、Search、Scanning、Help）的键盘处理逻辑

## 技术栈

- **ratatui**: TUI 框架
- **crossterm**: 跨平台终端操作
- **color-eyre**: 错误处理
- **walkdir**: 目录遍历
- **bytesize**: 字节大小格式化
- **directories**: 系统目录获取
- **rayon**: 并行计算（目录大小）

## 状态流转

```text
[启动] → Normal (欢迎页)
         ↓ 's'/'S'
       Scanning ←───────────┐
         ↓ 完成              │
       Normal                │
         ↓ Enter             │
       浏览目录               │
         ↓ 'o'               │
       切换排序               │
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
         ↑ Esc
         ↓ Enter
       清理并刷新 → 通知释放空间
         ↓ '?'
        Help
         ↓ any key
       Normal
```
