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
    /// 搜索模式
    Search,
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
    /// CocoaPods 缓存
    CocoaPods,
    /// npm 缓存
    NpmCache,
    /// pip 缓存
    PipCache,
    /// Docker 数据
    DockerData,
    /// Cargo 缓存
    CargoCache,
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
            ItemCategory::CocoaPods => "CocoaPods 缓存",
            ItemCategory::NpmCache => "npm 缓存",
            ItemCategory::PipCache => "pip 缓存",
            ItemCategory::DockerData => "Docker 数据",
            ItemCategory::CargoCache => "Cargo 缓存",
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
            ItemCategory::CocoaPods => "CocoaPods 缓存目录",
            ItemCategory::NpmCache => "npm 包下载缓存",
            ItemCategory::PipCache => "pip 包下载缓存",
            ItemCategory::DockerData => "Docker 容器和镜像数据",
            ItemCategory::CargoCache => "Cargo registry 下载缓存",
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

/// 导航栈帧：保存一层目录的路径、条目和滚动位置
#[derive(Debug, Clone)]
struct NavFrame {
    path: PathBuf,
    entries: Vec<CleanableEntry>,
    selected_index: Option<usize>,
}

/// 导航状态
#[derive(Debug, Clone, Default)]
pub struct NavigationState {
    pub current_path: Option<PathBuf>,
    stack: Vec<NavFrame>,
}

impl NavigationState {
    pub fn new() -> Self {
        Self {
            current_path: None,
            stack: Vec::new(),
        }
    }

    pub fn reset_root(&mut self) {
        self.stack.clear();
        self.current_path = None;
    }

    pub fn enter(
        &mut self,
        path: PathBuf,
        current_entries: Vec<CleanableEntry>,
        selected_index: Option<usize>,
    ) {
        self.stack.push(NavFrame {
            path: path.clone(),
            entries: current_entries,
            selected_index,
        });
        self.current_path = Some(path);
    }

    pub fn back(&mut self) -> Option<(Vec<CleanableEntry>, Option<usize>)> {
        let popped = self.stack.pop()?;
        self.current_path = self.stack.last().map(|f| f.path.clone());
        if self.current_path.is_some() {
            Some((popped.entries, popped.selected_index))
        } else {
            None
        }
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
    /// 可视区域高度（由渲染时更新）
    pub visible_height: usize,
    /// 上次清理结果：(释放空间, 条目数)
    pub last_clean_result: Option<(u64, usize)>,
    /// 确认弹窗滚动偏移
    pub confirm_scroll: usize,
    /// 搜索查询字符串
    pub search_query: String,
    /// 搜索前的原始条目（用于取消搜索时恢复）
    pub pre_search_entries: Vec<CleanableEntry>,
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
            visible_height: 20,
            last_clean_result: None,
            confirm_scroll: 0,
            search_query: String::new(),
            pre_search_entries: Vec::new(),
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

    /// 跳到列表第一项
    pub fn first(&mut self) {
        if !self.entries.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    /// 跳到列表最后一项
    pub fn last(&mut self) {
        if !self.entries.is_empty() {
            self.list_state.select(Some(self.entries.len() - 1));
        }
    }

    /// 向下翻半页
    pub fn page_down(&mut self, visible_height: usize) {
        if self.entries.is_empty() {
            return;
        }
        let half_page = (visible_height / 2).max(1);
        let current = self.list_state.selected().unwrap_or(0);
        let target = (current + half_page).min(self.entries.len() - 1);
        self.list_state.select(Some(target));
    }

    /// 向上翻半页
    pub fn page_up(&mut self, visible_height: usize) {
        if self.entries.is_empty() {
            return;
        }
        let half_page = (visible_height / 2).max(1);
        let current = self.list_state.selected().unwrap_or(0);
        let target = current.saturating_sub(half_page);
        self.list_state.select(Some(target));
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
        // 收集路径和元数据，避免 clone 整个 entries
        let info: Vec<_> = self
            .entries
            .iter()
            .map(|e| (e.path.clone(), e.kind, e.size))
            .collect();
        for (path, kind, size) in info {
            if !all_selected {
                if let std::collections::hash_map::Entry::Vacant(entry) =
                    self.selections.entry(path)
                {
                    entry.insert(SelectedEntry { kind, size });
                    if let Some(s) = size {
                        self.selected_size += s;
                    }
                }
            } else if let Some(prev) = self.selections.remove(&path)
                && let Some(s) = prev.size
            {
                self.selected_size = self.selected_size.saturating_sub(s);
            }
        }
    }

    /// 更新条目选中状态
    fn set_selected(&mut self, path: &PathBuf, selected: bool, entry: &CleanableEntry) {
        if selected {
            if let std::collections::hash_map::Entry::Vacant(vacant) =
                self.selections.entry(path.clone())
            {
                vacant.insert(SelectedEntry {
                    kind: entry.kind,
                    size: entry.size,
                });
                if let Some(size) = entry.size {
                    self.selected_size += size;
                }
            }
        } else if let Some(prev) = self.selections.remove(path)
            && let Some(size) = prev.size
        {
            self.selected_size = self.selected_size.saturating_sub(size);
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

    /// 恢复根目录条目视图
    pub fn restore_root_entries(&mut self) {
        self.sort_root_entries();
    }

    /// 从缓存恢复目录条目视图（回退到上一级目录时使用）
    pub fn restore_cached_dir_entries(
        &mut self,
        cached_entries: Vec<CleanableEntry>,
        selected_index: Option<usize>,
    ) {
        let selected_path = selected_index
            .and_then(|index| cached_entries.get(index))
            .map(|entry| entry.path.clone());

        self.set_entries(cached_entries);
        self.sort_dir_entries();

        if let Some(selected_path) = selected_path {
            if let Some(restored_index) = self
                .entries
                .iter()
                .position(|entry| entry.path == selected_path)
            {
                self.list_state.select(Some(restored_index));
            }
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
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.path == *path)
            && entry.size.is_none()
        {
            entry.size = Some(size);
            self.total_size += size;
        }

        if let Some(selected) = self.selections.get_mut(path)
            && selected.size.is_none()
        {
            selected.size = Some(size);
            self.selected_size += size;
        }
    }

    /// 根层条目排序
    pub fn sort_root_entries(&mut self) {
        match self.sort_order {
            SortOrder::ByName => {
                self.root_entries.sort_by(|a, b| match (a.kind, b.kind) {
                    (EntryKind::Directory, EntryKind::File) => std::cmp::Ordering::Less,
                    (EntryKind::File, EntryKind::Directory) => std::cmp::Ordering::Greater,
                    _ => a.name.cmp(&b.name),
                });
            }
            SortOrder::BySize => {
                self.root_entries
                    .sort_by(|a, b| b.size.unwrap_or(0).cmp(&a.size.unwrap_or(0)));
            }
        }
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
        if self.navigation.current_path.is_none() {
            self.sort_root_entries();
        } else {
            self.sort_dir_entries();
        }
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
            self.confirm_scroll = 0;
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

    /// 进入搜索模式
    pub fn start_search(&mut self) {
        self.search_query.clear();
        self.pre_search_entries = self.entries.clone();
        self.mode = Mode::Search;
    }

    /// 搜索输入字符
    pub fn search_char(&mut self, c: char) {
        self.search_query.push(c);
        self.apply_search_filter();
    }

    /// 搜索删除字符
    pub fn search_backspace(&mut self) {
        self.search_query.pop();
        self.apply_search_filter();
    }

    /// 应用搜索过滤
    fn apply_search_filter(&mut self) {
        if self.search_query.is_empty() {
            self.set_entries(self.pre_search_entries.clone());
            return;
        }
        let query = self.search_query.to_lowercase();
        let filtered: Vec<CleanableEntry> = self
            .pre_search_entries
            .iter()
            .filter(|entry| entry.name.to_lowercase().contains(&query))
            .cloned()
            .collect();
        self.set_entries(filtered);
    }

    /// 确认搜索（保留过滤结果）
    pub fn confirm_search(&mut self) {
        self.mode = Mode::Normal;
    }

    /// 取消搜索（恢复原始列表）
    pub fn cancel_search(&mut self) {
        self.mode = Mode::Normal;
        let restored = self.pre_search_entries.clone();
        self.set_entries(restored);
        self.search_query.clear();
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

    fn named_entry(name: &str, kind: EntryKind, size: Option<u64>) -> CleanableEntry {
        CleanableEntry {
            kind,
            category: None,
            path: PathBuf::from(format!("/tmp/{name}")),
            name: name.to_string(),
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

    #[test]
    fn sort_root_entries_respects_sort_order_by_size() {
        let mut app = App::new();
        app.root_entries = vec![
            named_entry("small", EntryKind::File, Some(10)),
            named_entry("big", EntryKind::File, Some(100)),
            named_entry("mid", EntryKind::File, Some(50)),
        ];
        app.sort_order = SortOrder::BySize;
        app.sort_root_entries();

        let names: Vec<&str> = app.root_entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["big", "mid", "small"]);
    }

    #[test]
    fn sort_root_entries_respects_sort_order_by_name() {
        let mut app = App::new();
        app.root_entries = vec![
            named_entry("c_file", EntryKind::File, Some(10)),
            named_entry("a_dir", EntryKind::Directory, Some(100)),
            named_entry("b_file", EntryKind::File, Some(50)),
        ];
        app.sort_order = SortOrder::ByName;
        app.sort_root_entries();

        let names: Vec<&str> = app.root_entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["a_dir", "b_file", "c_file"]);
    }

    #[test]
    fn toggle_sort_order_at_root_applies_to_root_entries() {
        let mut app = App::new();
        app.root_entries = vec![
            named_entry("z_small", EntryKind::File, Some(1)),
            named_entry("a_big", EntryKind::File, Some(100)),
        ];
        // 初始在根目录（navigation.current_path 为 None）
        assert!(app.navigation.current_path.is_none());
        app.sort_order = SortOrder::ByName;
        app.sort_root_entries();

        // 切换到 BySize
        app.toggle_sort_order();
        assert_eq!(app.sort_order, SortOrder::BySize);
        let names: Vec<&str> = app.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["a_big", "z_small"]);
    }

    #[test]
    fn toggle_sort_order_in_subdir_applies_to_dir_entries() {
        let mut app = App::new();
        app.navigation
            .enter(PathBuf::from("/tmp/subdir"), Vec::new(), None);
        app.entries = vec![
            named_entry("z_item", EntryKind::File, Some(1)),
            named_entry("a_item", EntryKind::File, Some(100)),
        ];
        app.sort_order = SortOrder::BySize;

        // 切换到 ByName
        app.toggle_sort_order();
        assert_eq!(app.sort_order, SortOrder::ByName);
        let names: Vec<&str> = app.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["a_item", "z_item"]);
    }

    #[test]
    fn restore_root_entries_applies_current_sort_order() {
        let mut app = App::new();
        app.root_entries = vec![
            named_entry("z_item", EntryKind::File, Some(1)),
            named_entry("a_item", EntryKind::File, Some(100)),
        ];
        app.sort_order = SortOrder::ByName;

        app.restore_root_entries();
        let names: Vec<&str> = app.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["a_item", "z_item"]);
    }

    #[test]
    fn restore_cached_dir_entries_applies_current_sort_order_and_preserves_selection() {
        let mut app = App::new();
        app.sort_order = SortOrder::BySize;
        app.navigation
            .enter(PathBuf::from("/tmp/parent"), Vec::new(), None);

        let cached_entries = vec![
            named_entry("z_small", EntryKind::File, Some(1)),
            named_entry("a_big", EntryKind::File, Some(100)),
        ];
        // 之前在缓存顺序中选中 z_small（索引 0），切换到 BySize 后应仍选中该条目
        app.restore_cached_dir_entries(cached_entries, Some(0));

        let names: Vec<&str> = app.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["a_big", "z_small"]);
        assert_eq!(app.list_state.selected(), Some(1));
    }

    #[test]
    fn back_returns_cached_entries_and_selected_index() {
        let mut nav = NavigationState::new();
        let root_entries = vec![
            named_entry("dir_a", EntryKind::Directory, Some(100)),
            named_entry("dir_b", EntryKind::Directory, Some(50)),
        ];

        // 进入 dir_a，缓存根层条目和选中位置
        nav.enter(PathBuf::from("/tmp/dir_a"), root_entries.clone(), Some(0));
        assert_eq!(nav.current_path, Some(PathBuf::from("/tmp/dir_a")));

        // 回退：应恢复缓存的条目和选中位置
        let result = nav.back();
        assert!(result.is_none()); // 回到根目录，栈为空
        assert!(nav.current_path.is_none());
    }

    #[test]
    fn back_from_nested_restores_parent_cache() {
        let mut nav = NavigationState::new();
        let level1_entries = vec![
            named_entry("child_a", EntryKind::Directory, Some(30)),
            named_entry("child_b", EntryKind::File, Some(20)),
        ];

        // 进入第一层（从根进入，缓存空的根条目）
        nav.enter(PathBuf::from("/tmp/dir"), Vec::new(), Some(0));
        // 进入第二层，缓存第一层条目
        nav.enter(
            PathBuf::from("/tmp/dir/sub"),
            level1_entries.clone(),
            Some(1),
        );
        assert_eq!(nav.current_path, Some(PathBuf::from("/tmp/dir/sub")));

        // 从第二层回退，应恢复进入第二层时缓存的条目（level1_entries）
        let result = nav.back();
        assert!(result.is_some());
        let (cached, idx) = result.unwrap();
        assert_eq!(nav.current_path, Some(PathBuf::from("/tmp/dir")));
        assert_eq!(cached.len(), 2); // 进入第二层时缓存的 level1_entries
        assert_eq!(idx, Some(1));

        // 再回退到根目录
        let result = nav.back();
        assert!(result.is_none());
        assert!(nav.current_path.is_none());
    }

    #[test]
    fn back_restores_entries_in_app() {
        let mut app = App::new();
        let root_entries = vec![named_entry("dir_parent", EntryKind::Directory, Some(200))];
        app.set_entries(root_entries.clone());

        // 进入第一层子目录，缓存根条目
        let parent_entries = vec![
            named_entry("file_a", EntryKind::File, Some(100)),
            named_entry("file_b", EntryKind::File, Some(50)),
        ];
        app.navigation
            .enter(PathBuf::from("/tmp/parent"), app.entries.clone(), Some(0));
        app.set_entries(parent_entries.clone());

        // 进入第二层子目录，缓存第一层条目
        app.navigation.enter(
            PathBuf::from("/tmp/parent/child"),
            app.entries.clone(),
            Some(1),
        );
        app.set_entries(vec![named_entry("sub_file", EntryKind::File, Some(10))]);
        assert_eq!(app.entries.len(), 1);

        // 从第二层回退到第一层：恢复缓存
        if let Some((cached_entries, selected_index)) = app.navigation.back() {
            app.set_entries(cached_entries);
            app.list_state.select(selected_index);
        }
        assert_eq!(app.entries.len(), 2);
        assert_eq!(app.list_state.selected(), Some(1));
        let names: Vec<&str> = app.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["file_a", "file_b"]);
    }

    #[test]
    fn reset_root_clears_navigation_stack() {
        let mut nav = NavigationState::new();
        nav.enter(PathBuf::from("/tmp/a"), Vec::new(), None);
        nav.enter(PathBuf::from("/tmp/a/b"), Vec::new(), None);
        assert!(nav.current_path.is_some());

        nav.reset_root();
        assert!(nav.current_path.is_none());
        assert!(nav.back().is_none());
    }
}
