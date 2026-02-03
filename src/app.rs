use ratatui::widgets::ListState;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::scanner::ScanKind;

/// 应用运行模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 正常浏览模式
    Normal,
    /// 扫描中
    Scanning,
    /// 确认删除
    Confirm,
    /// 帮助界面
    Help,
    /// 路径输入模式
    InputPath,
}

/// 排序方式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    /// 按名称排序（目录优先）
    #[default]
    ByName,
    /// 按大小降序排序
    BySize,
}

impl SortOrder {
    pub fn as_str(&self) -> &'static str {
        match self {
            SortOrder::ByName => "名称",
            SortOrder::BySize => "大小",
        }
    }

    pub fn toggle(&self) -> Self {
        match self {
            SortOrder::ByName => SortOrder::BySize,
            SortOrder::BySize => SortOrder::ByName,
        }
    }
}

/// 扫描项类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemCategory {
    /// 系统缓存
    SystemCache,
    /// 应用缓存
    AppCache,
    /// 日志文件
    Logs,
    /// 临时文件
    Temp,
    /// Xcode 派生数据
    XcodeDerivedData,
    /// npm/yarn 缓存
    NodeModules,
    /// Homebrew 缓存
    HomebrewCache,
    /// 下载文件
    Downloads,
    /// 垃圾桶
    Trash,
}

impl ItemCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            ItemCategory::SystemCache => "系统缓存",
            ItemCategory::AppCache => "应用缓存",
            ItemCategory::Logs => "日志文件",
            ItemCategory::Temp => "临时文件",
            ItemCategory::XcodeDerivedData => "Xcode 派生数据",
            ItemCategory::NodeModules => "node_modules",
            ItemCategory::HomebrewCache => "Homebrew 缓存",
            ItemCategory::Downloads => "下载文件夹",
            ItemCategory::Trash => "垃圾桶",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ItemCategory::SystemCache => "macOS 系统级缓存文件",
            ItemCategory::AppCache => "应用程序产生的缓存",
            ItemCategory::Logs => "系统和应用的日志文件",
            ItemCategory::Temp => "临时文件和目录",
            ItemCategory::XcodeDerivedData => "Xcode 构建产物和索引",
            ItemCategory::NodeModules => "Node.js 依赖目录",
            ItemCategory::HomebrewCache => "Homebrew 下载缓存",
            ItemCategory::Downloads => "下载文件夹中的文件",
            ItemCategory::Trash => "回收站中的文件",
        }
    }
}

/// 条目类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Directory,
    File,
}

/// 可清理条目
#[derive(Debug, Clone)]
pub struct CleanableEntry {
    pub kind: EntryKind,
    pub category: Option<ItemCategory>,
    pub path: PathBuf,
    pub name: String,
    pub size: Option<u64>,
}

/// 选中条目
#[derive(Debug, Clone)]
pub struct SelectedEntry {
    pub kind: EntryKind,
    pub size: Option<u64>,
}

/// 导航状态
#[derive(Debug, Clone)]
pub struct NavigationState {
    pub current_path: Option<PathBuf>,
    pub path_stack: Vec<PathBuf>,
}

impl NavigationState {
    pub fn new() -> Self {
        Self {
            current_path: None,
            path_stack: Vec::new(),
        }
    }

    pub fn reset_root(&mut self) {
        self.path_stack.clear();
        self.current_path = None;
    }

    pub fn enter(&mut self, path: PathBuf) {
        self.path_stack.push(path.clone());
        self.current_path = Some(path);
    }

    pub fn back(&mut self) {
        self.path_stack.pop();
        self.current_path = self.path_stack.last().cloned();
    }

    pub fn breadcrumb(&self) -> String {
        match &self.current_path {
            Some(path) => path.display().to_string(),
            None => "/".to_string(),
        }
    }
}

/// 应用状态
pub struct App {
    /// 当前模式
    pub mode: Mode,
    /// 是否退出
    pub should_quit: bool,
    /// 当前视图条目
    pub entries: Vec<CleanableEntry>,
    /// 根层条目缓存
    pub root_entries: Vec<CleanableEntry>,
    /// 列表状态
    pub list_state: ListState,
    /// 扫描进度 (0-100)
    pub scan_progress: u8,
    /// 当前扫描路径
    pub current_scan_path: String,
    /// 总计可清理大小（当前视图）
    pub total_size: u64,
    /// 已选择大小（跨目录）
    pub selected_size: u64,
    /// 错误消息
    pub error_message: Option<String>,
    /// 选中条目
    pub selections: HashMap<PathBuf, SelectedEntry>,
    /// 导航状态
    pub navigation: NavigationState,
    /// 扫描代次
    pub scan_generation: u64,
    /// 当前扫描类型
    pub scan_kind: ScanKind,
    /// 是否扫描中
    pub scan_in_progress: bool,
    /// 排序方式
    pub sort_order: SortOrder,
    /// 路径输入缓冲区
    pub input_buffer: String,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));

        Self {
            mode: Mode::Normal,
            should_quit: false,
            entries: Vec::new(),
            root_entries: Vec::new(),
            list_state,
            scan_progress: 0,
            current_scan_path: String::new(),
            total_size: 0,
            selected_size: 0,
            error_message: None,
            selections: HashMap::new(),
            navigation: NavigationState::new(),
            scan_generation: 0,
            scan_kind: ScanKind::Root,
            scan_in_progress: false,
            sort_order: SortOrder::default(),
            input_buffer: String::new(),
        }
    }

    /// 选择下一项
    pub fn next(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => (i + 1) % self.entries.len(),
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    /// 选择上一项
    pub fn previous(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.entries.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    /// 当前高亮条目
    pub fn current_entry(&self) -> Option<&CleanableEntry> {
        let index = self.list_state.selected()?;
        self.entries.get(index)
    }

    /// 切换当前项的选中状态
    pub fn toggle_selected(&mut self) {
        if let Some(entry) = self.current_entry().cloned() {
            let path = entry.path.clone();
            let selected = self.selections.contains_key(&path);
            self.set_selected(&path, !selected, &entry);
        }
    }

    /// 全选/取消全选（当前视图）
    pub fn toggle_all(&mut self) {
        let all_selected = self
            .entries
            .iter()
            .all(|entry| self.selections.contains_key(&entry.path));
        let entries = self.entries.clone();
        for entry in &entries {
            self.set_selected(&entry.path, !all_selected, entry);
        }
    }

    /// 更新条目选中状态
    fn set_selected(&mut self, path: &PathBuf, selected: bool, entry: &CleanableEntry) {
        if selected {
            if !self.selections.contains_key(path) {
                self.selections.insert(
                    path.clone(),
                    SelectedEntry {
                        kind: entry.kind,
                        size: entry.size,
                    },
                );
                if let Some(size) = entry.size {
                    self.selected_size += size;
                }
            }
        } else if let Some(prev) = self.selections.remove(path) {
            if let Some(size) = prev.size {
                self.selected_size = self.selected_size.saturating_sub(size);
            }
        }
    }

    pub fn is_selected(&self, path: &PathBuf) -> bool {
        self.selections.contains_key(path)
    }

    /// 设置当前视图条目
    pub fn set_entries(&mut self, entries: Vec<CleanableEntry>) {
        self.entries = entries;
        self.total_size = self.entries.iter().filter_map(|e| e.size).sum();
        if self.entries.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
        }
    }

    /// 清空当前视图条目
    pub fn clear_entries(&mut self) {
        self.entries.clear();
        self.total_size = 0;
        self.list_state.select(None);
    }

    /// 清空根条目缓存
    pub fn clear_root_entries(&mut self) {
        self.root_entries.clear();
    }

    /// 应用根层条目
    pub fn apply_root_entry(&mut self, entry: CleanableEntry) {
        self.root_entries.push(entry.clone());
        if self.navigation.current_path.is_none() {
            if let Some(size) = entry.size {
                self.total_size += size;
            }
            self.entries.push(entry);
            if self.entries.len() == 1 {
                self.list_state.select(Some(0));
            }
        }
    }

    /// 应用目录条目
    pub fn apply_dir_entry(&mut self, entry: CleanableEntry) {
        if let Some(size) = entry.size {
            self.total_size += size;
        }
        self.entries.push(entry);
        if self.entries.len() == 1 {
            self.list_state.select(Some(0));
        }
    }

    /// 回填条目大小
    pub fn apply_entry_size(&mut self, path: &PathBuf, size: u64) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.path == *path) {
            if entry.size.is_none() {
                entry.size = Some(size);
                self.total_size += size;
            }
        }

        if let Some(selected) = self.selections.get_mut(path) {
            if selected.size.is_none() {
                selected.size = Some(size);
                self.selected_size += size;
            }
        }
    }

    /// 根层条目排序
    pub fn sort_root_entries(&mut self) {
        self.root_entries
            .sort_by(|a, b| b.size.unwrap_or(0).cmp(&a.size.unwrap_or(0)));
        if self.navigation.current_path.is_none() {
            self.set_entries(self.root_entries.clone());
        }
    }

    /// 目录条目排序
    pub fn sort_dir_entries(&mut self) {
        match self.sort_order {
            SortOrder::ByName => {
                self.entries.sort_by(|a, b| match (a.kind, b.kind) {
                    (EntryKind::Directory, EntryKind::File) => std::cmp::Ordering::Less,
                    (EntryKind::File, EntryKind::Directory) => std::cmp::Ordering::Greater,
                    _ => a.name.cmp(&b.name),
                });
            }
            SortOrder::BySize => {
                self.entries
                    .sort_by(|a, b| b.size.unwrap_or(0).cmp(&a.size.unwrap_or(0)));
            }
        }
        if !self.entries.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    /// 切换排序方式
    pub fn toggle_sort_order(&mut self) {
        self.sort_order = self.sort_order.toggle();
        self.sort_dir_entries();
    }

    /// 获取选中的项目
    pub fn get_selected_items(&self) -> Vec<CleanableEntry> {
        self.selections
            .iter()
            .map(|(path, entry)| CleanableEntry {
                kind: entry.kind,
                category: None,
                path: path.clone(),
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.display().to_string()),
                size: entry.size,
            })
            .collect()
    }

    /// 进入确认删除模式
    pub fn enter_confirm_mode(&mut self) {
        if self.selected_size > 0 {
            self.mode = Mode::Confirm;
        }
    }

    /// 取消确认
    pub fn cancel_confirm(&mut self) {
        self.mode = Mode::Normal;
    }

    /// 显示/隐藏帮助
    pub fn toggle_help(&mut self) {
        self.mode = if self.mode == Mode::Help {
            Mode::Normal
        } else {
            Mode::Help
        };
    }

    /// 退出应用
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// 设置错误消息
    pub fn set_error(&mut self, msg: String) {
        self.error_message = Some(msg);
    }

    /// 清除错误消息
    pub fn clear_error(&mut self) {
        self.error_message = None;
    }

    /// 面包屑路径
    pub fn breadcrumb(&self) -> String {
        self.navigation.breadcrumb()
    }

    /// 重置扫描状态
    pub fn finish_scan(&mut self) {
        self.scan_in_progress = false;
        if self.mode == Mode::Scanning {
            self.mode = Mode::Normal;
        }
        self.scan_progress = 100;
    }

    /// 清除所有选中
    pub fn clear_selections(&mut self) {
        self.selections.clear();
        self.selected_size = 0;
    }

    /// 进入路径输入模式
    pub fn start_input(&mut self) {
        self.input_buffer.clear();
        self.mode = Mode::InputPath;
    }

    /// 输入字符
    pub fn input_char(&mut self, c: char) {
        self.input_buffer.push(c);
    }

    /// 删除输入字符
    pub fn input_backspace(&mut self) {
        self.input_buffer.pop();
    }

    /// 确认输入并返回路径
    pub fn confirm_input(&mut self) -> Option<PathBuf> {
        self.mode = Mode::Normal;
        let path = self.input_buffer.trim();
        if path.is_empty() {
            return None;
        }
        // 展开 ~ 为用户主目录
        let expanded = if path.starts_with('~') {
            if let Some(home) = directories::UserDirs::new() {
                let home_str = home.home_dir().display().to_string();
                path.replacen('~', &home_str, 1)
            } else {
                path.to_string()
            }
        } else {
            path.to_string()
        };
        Some(PathBuf::from(expanded))
    }

    /// 取消输入
    pub fn cancel_input(&mut self) {
        self.input_buffer.clear();
        self.mode = Mode::Normal;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn entry(path: &str, size: Option<u64>) -> CleanableEntry {
        CleanableEntry {
            kind: EntryKind::File,
            category: None,
            path: PathBuf::from(path),
            name: "item".to_string(),
            size,
        }
    }

    #[test]
    fn toggle_selected_updates_selected_size() {
        let mut app = App::new();
        app.entries = vec![entry("/tmp/a", Some(10)), entry("/tmp/b", Some(5))];
        app.list_state.select(Some(0));

        app.toggle_selected();
        assert_eq!(app.selected_size, 10);

        app.toggle_selected();
        assert_eq!(app.selected_size, 0);
    }

    #[test]
    fn toggle_all_selects_and_deselects() {
        let mut app = App::new();
        app.entries = vec![entry("/tmp/a", Some(3)), entry("/tmp/b", Some(7))];

        app.toggle_all();
        assert_eq!(app.selections.len(), 2);
        assert_eq!(app.selected_size, 10);

        app.toggle_all();
        assert!(app.selections.is_empty());
        assert_eq!(app.selected_size, 0);
    }

    #[test]
    fn apply_entry_size_updates_selected_size() {
        let mut app = App::new();
        let entry = entry("/tmp/a", None);
        app.entries = vec![entry.clone()];
        app.list_state.select(Some(0));
        app.toggle_selected();

        app.apply_entry_size(&PathBuf::from("/tmp/a"), 12);
        assert_eq!(app.selected_size, 12);
    }
}
