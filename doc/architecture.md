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
    ├── main.rs         # 程序入口
    ├── lib.rs          # 库入口，模块导出
    ├── app.rs          # 应用状态管理
    ├── ui.rs           # UI 渲染
    ├── scanner.rs      # 磁盘扫描器
    └── cleaner.rs      # 文件清理器
```

## 模块说明

### app.rs - 应用状态管理

核心数据结构：

- `App`: 应用主状态，包含模式、条目列表、扫描进度等
- `Mode`: 应用运行模式 (Normal, Scanning, Confirm, Help, InputPath)
- `SortOrder`: 排序方式 (ByName, BySize)
- `EntryKind`: 条目类型（目录/文件）
- `CleanableEntry`: 当前视图条目
- `NavigationState`: 导航状态（当前路径、路径栈）

### ui.rs - UI 渲染

使用 ratatui 渲染 TUI 界面：

- `render()`: 主渲染函数
- `render_header()`: 头部标题、路径与统计信息
- `render_main()`: 主内容区（列表或扫描进度）
- `render_footer()`: 底部快捷键提示
- 弹窗：帮助、确认删除、错误提示

### scanner.rs - 磁盘扫描器

支持三种扫描模式：

1. **Root 扫描**：扫描 macOS 常见可清理目录
   - 系统缓存 (`~/Library/Caches`)
   - 日志文件 (`~/Library/Logs`)
   - 临时文件 (`/tmp`, `/var/tmp`)
   - Xcode 派生数据
   - Homebrew 缓存
   - 下载文件夹
   - 垃圾桶

2. **ListDir 扫描**：列出指定目录的内容（用于目录浏览）

3. **DiskScan 扫描**：扫描指定路径的顶层目录/文件并计算大小

支持异步扫描，通过 `mpsc::channel` 发送进度消息，并支持目录列表扫描与目录大小回填：

- `ScanMessage::Progress` - 进度更新
- `ScanMessage::RootItem` - 根目录扫描完成
- `ScanMessage::DirEntry` - 目录条目
- `ScanMessage::DirEntrySize` - 目录大小回填
- `ScanMessage::Done` - 全部完成
- `ScanMessage::Error` - 扫描出错

### cleaner.rs - 文件清理器

安全清理选中的文件/目录：

- 路径安全检查
- 保留目录结构，仅清理内容
- 错误收集和报告

## 技术栈

- **ratatui**: TUI 框架
- **crossterm**: 跨平台终端操作
- **color-eyre**: 错误处理
- **walkdir**: 目录遍历
- **bytesize**: 字节大小格式化
- **directories**: 系统目录获取

## 状态流转

```text
[启动] → Normal
         ↓ 's'/'S'
       Scanning ←───────┐
         ↓ 完成          │
       Normal            │
         ↓ Enter         │
       浏览目录           │
         ↓ 'o'           │
       切换排序           │
         ↓ 'd'           │
       InputPath ────────┘
         ↓ Enter (输入路径)
       Scanning
         ↓ 完成
       Normal
         ↓ c
       Confirm
         ↑ Esc
         ↓ Enter
       清理并刷新
         ↓ '?'
        Help
         ↓ any key
       Normal
```
