use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use clap::Parser;
use color_eyre::Result;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use vac::app::{App, EntryKind, Mode};
use vac::cleaner::Cleaner;
use vac::cli::Cli;
use vac::config::AppConfig;
use vac::scanner::{ScanKind, ScanMessage, Scanner, format_size, scanner_from_config};
use vac::ui;

fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    if cli.is_non_interactive() {
        return run_non_interactive(cli);
    }

    let mut terminal = ratatui::init();
    let result = run_tui(&mut terminal);

    ratatui::restore();
    result
}

fn run_tui(terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
    let config = AppConfig::load();
    let mut app = App::with_config(&config);
    let mut scan_rx: Option<Receiver<ScanMessage>> = None;
    let cancel_generation = Arc::new(AtomicU64::new(0));

    loop {
        terminal.draw(|frame| ui::render(frame, &mut app))?;

        // 处理扫描消息
        if let Some(rx) = &scan_rx {
            while let Ok(msg) = rx.try_recv() {
                if msg.job_id() != app.scan_generation {
                    continue;
                }

                match msg {
                    ScanMessage::Progress { progress, path, .. } => {
                        app.scan_progress = progress;
                        app.current_scan_path = path;
                    }
                    ScanMessage::RootItem { entry, .. } => {
                        app.apply_root_entry(entry);
                    }
                    ScanMessage::DirEntry { entry, .. } => {
                        app.apply_dir_entry(entry);
                    }
                    ScanMessage::DirEntrySize { path, size, .. } => {
                        app.apply_entry_size(&path, size);
                    }
                    ScanMessage::Done { .. } => {
                        match app.scan_kind {
                            ScanKind::Root | ScanKind::DiskScan => app.sort_root_entries(),
                            ScanKind::ListDir => app.sort_dir_entries(),
                        }
                        app.finish_scan();
                        scan_rx = None;
                        break;
                    }
                    ScanMessage::Error { message, .. } => {
                        app.set_error(message);
                        app.finish_scan();
                        scan_rx = None;
                        break;
                    }
                }
            }
        }

        let poll_timeout = if scan_rx.is_some() {
            Duration::from_millis(16)
        } else {
            Duration::from_millis(100)
        };
        if event::poll(poll_timeout)?
            && let Event::Key(key) = event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            // 处理错误消息时，仅 Enter/Esc 关闭
            if app.error_message.is_some() {
                match key.code {
                    KeyCode::Enter | KeyCode::Esc => app.clear_error(),
                    _ => {}
                }
                continue;
            }

            // 帮助界面任意键关闭
            if app.mode == Mode::Help {
                app.toggle_help();
                continue;
            }

            // 统计面板任意键关闭
            if app.mode == Mode::Stats {
                app.toggle_stats();
                continue;
            }

            // 确认删除界面
            if app.mode == Mode::Confirm {
                if let Some(rx) =
                    handle_confirm_mode(&mut app, key.code, &cancel_generation, &config)
                {
                    scan_rx = Some(rx);
                }
                continue;
            }

            // 路径输入模式
            if app.mode == Mode::InputPath {
                match key.code {
                    KeyCode::Esc => app.cancel_input(),
                    KeyCode::Enter => {
                        if let Some(path) = app.confirm_input() {
                            scan_rx = start_disk_scan(&mut app, path, &cancel_generation);
                        }
                    }
                    KeyCode::Tab => app.input_tab_complete(),
                    KeyCode::BackTab => app.input_tab_complete_prev(),
                    KeyCode::Backspace => app.input_backspace(),
                    KeyCode::Char(c) => app.input_char(c),
                    _ => {}
                }
                continue;
            }

            // 搜索模式
            if app.mode == Mode::Search {
                match key.code {
                    KeyCode::Esc => app.cancel_search(),
                    KeyCode::Enter => app.confirm_search(),
                    KeyCode::Backspace => app.search_backspace(),
                    KeyCode::Char(c) => app.search_char(c),
                    _ => {}
                }
                continue;
            }

            // 根扫描中仅允许取消/退出
            if app.mode == Mode::Scanning {
                match key.code {
                    KeyCode::Esc => cancel_scan(&mut app, &cancel_generation, &mut scan_rx),
                    KeyCode::Char('q') => app.quit(),
                    _ => {}
                }
                continue;
            }

            // 清除上次清理结果通知
            app.last_clean_result = None;

            // 扫描中按 Esc 可取消
            if app.scan_in_progress && key.code == KeyCode::Esc {
                cancel_scan(&mut app, &cancel_generation, &mut scan_rx);
                continue;
            }

            match key.code {
                KeyCode::Char('q') => app.quit(),
                KeyCode::Char('?') => app.toggle_help(),
                KeyCode::Char('s') => {
                    scan_rx = start_root_scan(&mut app, &cancel_generation, &config);
                }
                KeyCode::Char('S') => {
                    // Shift+S: 扫描主目录
                    if let Some(scanner) = scanner_from_config(&config) {
                        let home = scanner.home_dir().clone();
                        scan_rx = start_disk_scan(&mut app, home, &cancel_generation);
                    }
                }
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    let h = app.visible_height;
                    app.page_down(h);
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    let h = app.visible_height;
                    app.page_up(h);
                }
                KeyCode::Char('d') => {
                    app.start_input();
                }
                KeyCode::Char('o') => {
                    app.toggle_sort_order();
                }
                KeyCode::Down | KeyCode::Char('j') => app.next(),
                KeyCode::Up | KeyCode::Char('k') => app.previous(),
                KeyCode::Char('g') => app.first(),
                KeyCode::Char('G') => app.last(),
                KeyCode::PageDown => {
                    let h = app.visible_height;
                    app.page_down(h);
                }
                KeyCode::PageUp => {
                    let h = app.visible_height;
                    app.page_up(h);
                }
                KeyCode::Char('/') => app.start_search(),
                KeyCode::Char('t') => app.toggle_stats(),
                KeyCode::Char(' ') => app.toggle_selected(),
                KeyCode::Char('a') => app.toggle_all(),
                KeyCode::Char('c') => app.enter_confirm_mode(),
                KeyCode::Enter => {
                    let target = app.current_entry().and_then(|e| {
                        if e.kind == EntryKind::Directory {
                            Some(e.path.clone())
                        } else {
                            None
                        }
                    });
                    if let Some(target) = target {
                        let selected_index = app.list_state.selected();
                        app.navigation
                            .enter(target.clone(), app.entries.clone(), selected_index);
                        scan_rx = start_dir_scan(&mut app, target, &cancel_generation);
                    }
                }
                KeyCode::Backspace | KeyCode::Esc => {
                    if app.navigation.current_path.is_some() {
                        if app.scan_in_progress {
                            cancel_scan(&mut app, &cancel_generation, &mut scan_rx);
                        }
                        if let Some((cached_entries, selected_index)) = app.navigation.back() {
                            app.restore_cached_dir_entries(cached_entries, selected_index);
                        } else {
                            app.restore_root_entries();
                        }
                    }
                }
                _ => {}
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn bump_generation(app: &mut App, cancel_generation: &Arc<AtomicU64>) -> u64 {
    app.scan_generation = app.scan_generation.wrapping_add(1);
    cancel_generation.store(app.scan_generation, Ordering::SeqCst);
    app.scan_generation
}

fn cancel_scan(
    app: &mut App,
    cancel_generation: &Arc<AtomicU64>,
    scan_rx: &mut Option<Receiver<ScanMessage>>,
) {
    bump_generation(app, cancel_generation);
    app.scan_in_progress = false;
    if app.mode == Mode::Scanning {
        app.mode = Mode::Normal;
    }
    app.scan_progress = 0;
    *scan_rx = None;
}

fn handle_confirm_mode(
    app: &mut App,
    key: KeyCode,
    cancel_generation: &Arc<AtomicU64>,
    config: &AppConfig,
) -> Option<Receiver<ScanMessage>> {
    match key {
        KeyCode::Enter => {
            let rx = execute_clean(app, cancel_generation, config);
            app.mode = Mode::Normal;
            rx
        }
        KeyCode::Esc => {
            app.cancel_confirm();
            None
        }
        KeyCode::Char('d') => {
            if app.dry_run_active {
                app.dry_run_active = false;
            } else {
                let selected_items = app.get_selected_items();
                app.dry_run_result = Some(Cleaner::dry_run(&selected_items));
                app.dry_run_active = true;
            }
            None
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.confirm_scroll = app.confirm_scroll.saturating_add(1);
            None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.confirm_scroll = app.confirm_scroll.saturating_sub(1);
            None
        }
        _ => None,
    }
}

fn start_root_scan(
    app: &mut App,
    cancel_generation: &Arc<AtomicU64>,
    config: &AppConfig,
) -> Option<Receiver<ScanMessage>> {
    let job_id = bump_generation(app, cancel_generation);
    app.scan_kind = ScanKind::Root;
    app.scan_in_progress = true;
    app.mode = Mode::Scanning;
    app.scan_progress = 0;
    app.current_scan_path = "准备扫描...".to_string();
    app.navigation.reset_root();
    app.clear_entries();
    app.clear_root_entries();

    let (tx, rx) = mpsc::channel();
    let cancel_clone = cancel_generation.clone();
    let extra_targets = config.expanded_extra_targets();

    thread::spawn(move || {
        if let Some(scanner) = Scanner::with_extra_targets(extra_targets) {
            scanner.scan_root_with_progress(job_id, tx, cancel_clone);
        } else {
            let _ = tx.send(ScanMessage::Error {
                job_id,
                message: "无法初始化扫描器".to_string(),
            });
        }
    });

    Some(rx)
}

fn start_dir_scan(
    app: &mut App,
    path: std::path::PathBuf,
    cancel_generation: &Arc<AtomicU64>,
) -> Option<Receiver<ScanMessage>> {
    let job_id = bump_generation(app, cancel_generation);
    app.scan_kind = ScanKind::ListDir;
    app.scan_in_progress = true;
    app.mode = Mode::Normal;
    app.scan_progress = 0;
    app.current_scan_path = path.display().to_string();
    app.clear_entries();

    let (tx, rx) = mpsc::channel();
    let cancel_clone = cancel_generation.clone();

    thread::spawn(move || {
        if let Some(scanner) = Scanner::new() {
            scanner.scan_dir_listing(job_id, path, tx, cancel_clone);
        } else {
            let _ = tx.send(ScanMessage::Error {
                job_id,
                message: "无法初始化扫描器".to_string(),
            });
        }
    });

    Some(rx)
}

fn start_disk_scan(
    app: &mut App,
    path: std::path::PathBuf,
    cancel_generation: &Arc<AtomicU64>,
) -> Option<Receiver<ScanMessage>> {
    let job_id = bump_generation(app, cancel_generation);
    app.scan_kind = ScanKind::DiskScan;
    app.scan_in_progress = true;
    app.mode = Mode::Scanning;
    app.scan_progress = 0;
    app.current_scan_path = format!("扫描: {}", path.display());
    app.navigation.reset_root();
    app.clear_entries();
    app.clear_root_entries();

    let (tx, rx) = mpsc::channel();
    let cancel_clone = cancel_generation.clone();

    thread::spawn(move || {
        if let Some(scanner) = Scanner::new() {
            scanner.scan_disk_with_progress(job_id, path, tx, cancel_clone);
        } else {
            let _ = tx.send(ScanMessage::Error {
                job_id,
                message: "无法初始化扫描器".to_string(),
            });
        }
    });

    Some(rx)
}

fn execute_clean(
    app: &mut App,
    cancel_generation: &Arc<AtomicU64>,
    config: &AppConfig,
) -> Option<Receiver<ScanMessage>> {
    let selected_items = app.get_selected_items();

    if selected_items.is_empty() {
        return None;
    }

    // 安全检查
    for item in &selected_items {
        if !Cleaner::is_safe_to_delete(&item.path) {
            app.set_error(format!("不安全的路径: {}", item.path.display()));
            return None;
        }
    }

    let item_count = selected_items.len();
    let result = if config.safety.move_to_trash {
        Cleaner::trash_items(&selected_items)
    } else {
        Cleaner::clean(&selected_items)
    };

    if result.success {
        app.last_clean_result = Some((result.freed_space, item_count));
        app.clear_selections();

        if let Some(path) = app.navigation.current_path.clone() {
            start_dir_scan(app, path, cancel_generation)
        } else {
            start_root_scan(app, cancel_generation, config)
        }
    } else {
        let error_msg = result.errors.join("\n");
        app.set_error(format!("部分清理失败:\n{}", error_msg));
        None
    }
}

// ── 非交互模式 ──────────────────────────────────────────────

use vac::app::{CleanableEntry, SortOrder};
use vac::cli::ScanTarget;

/// 非交互模式的扫描结果条目（用于 JSON 输出）
#[derive(serde::Serialize)]
struct ReportEntry {
    path: String,
    name: String,
    kind: String,
    size: Option<u64>,
    size_display: String,
    modified_at: Option<String>,
}

/// 非交互模式的 dry-run 条目（用于 JSON 输出）
#[derive(serde::Serialize)]
struct DryRunReportItem {
    path: String,
    file_count: usize,
    dir_count: usize,
    size: u64,
    size_display: String,
}

/// 非交互模式的清理结果（用于 JSON 输出）
#[derive(serde::Serialize)]
struct CleanReport {
    success: bool,
    freed_space: u64,
    freed_space_display: String,
    item_count: usize,
    use_trash: bool,
    errors: Vec<String>,
}

/// 非交互模式的完整报告（用于 JSON 输出）
#[derive(serde::Serialize)]
struct ScanReport {
    scan_target: String,
    sort_order: String,
    total_items: usize,
    total_size: u64,
    total_size_display: String,
    entries: Vec<ReportEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dry_run: Option<DryRunReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    clean_result: Option<CleanReport>,
}

/// Dry-run 报告
#[derive(serde::Serialize)]
struct DryRunReport {
    total_files: usize,
    total_dirs: usize,
    total_size: u64,
    total_size_display: String,
    items: Vec<DryRunReportItem>,
}

/// 同步执行扫描并收集结果
fn run_scan_blocking(scan_target: &ScanTarget, config: &AppConfig) -> Result<Vec<CleanableEntry>> {
    let (tx, rx) = mpsc::channel();
    let cancel = Arc::new(AtomicU64::new(0));
    let job_id = 1u64;
    cancel.store(job_id, Ordering::SeqCst);

    match scan_target {
        ScanTarget::Preset => {
            let extra_targets = config.expanded_extra_targets();
            let cancel_clone = cancel.clone();
            thread::spawn(move || {
                if let Some(scanner) = Scanner::with_extra_targets(extra_targets) {
                    scanner.scan_root_with_progress(job_id, tx, cancel_clone);
                } else {
                    let _ = tx.send(ScanMessage::Error {
                        job_id,
                        message: "无法初始化扫描器".to_string(),
                    });
                }
            });
        }
        ScanTarget::Home => {
            let cancel_clone = cancel.clone();
            thread::spawn(move || {
                if let Some(scanner) = Scanner::new() {
                    let home = scanner.home_dir().clone();
                    scanner.scan_disk_with_progress(job_id, home, tx, cancel_clone);
                } else {
                    let _ = tx.send(ScanMessage::Error {
                        job_id,
                        message: "无法初始化扫描器".to_string(),
                    });
                }
            });
        }
        ScanTarget::Path(path) => {
            let path = path.clone();
            let cancel_clone = cancel.clone();
            thread::spawn(move || {
                if let Some(scanner) = Scanner::new() {
                    scanner.scan_disk_with_progress(job_id, path, tx, cancel_clone);
                } else {
                    let _ = tx.send(ScanMessage::Error {
                        job_id,
                        message: "无法初始化扫描器".to_string(),
                    });
                }
            });
        }
    }

    let mut entries = Vec::new();
    for msg in rx {
        match msg {
            ScanMessage::RootItem { entry, .. } => {
                entries.push(entry);
            }
            ScanMessage::DirEntry { entry, .. } => {
                entries.push(entry);
            }
            ScanMessage::DirEntrySize { path, size, .. } => {
                if let Some(entry) = entries.iter_mut().find(|e| e.path == path) {
                    entry.size = Some(size);
                }
            }
            ScanMessage::Progress { progress, .. } => {
                eprint!("\r扫描进度: {}%", progress);
            }
            ScanMessage::Done { .. } => {
                eprintln!("\r扫描完成。      ");
                break;
            }
            ScanMessage::Error { message, .. } => {
                return Err(color_eyre::eyre::eyre!("扫描失败: {}", message));
            }
        }
    }

    Ok(entries)
}

/// 对条目排序
fn sort_entries(entries: &mut [CleanableEntry], sort_order: &SortOrder) {
    match sort_order {
        SortOrder::ByName => {
            entries.sort_by(|a, b| {
                use vac::app::EntryKind;
                match (a.kind, b.kind) {
                    (EntryKind::Directory, EntryKind::File) => std::cmp::Ordering::Less,
                    (EntryKind::File, EntryKind::Directory) => std::cmp::Ordering::Greater,
                    _ => a.name.cmp(&b.name),
                }
            });
        }
        SortOrder::BySize => {
            entries.sort_by(|a, b| b.size.unwrap_or(0).cmp(&a.size.unwrap_or(0)));
        }
        SortOrder::ByTime => {
            entries.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
        }
    }
}

/// 格式化 SystemTime 为 "YYYY-MM-DD HH:MM:SS" 字符串（CLI 输出用）
fn format_time_cli(time: &std::time::SystemTime) -> String {
    let duration = time
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs() as i64;

    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    let mut remaining_days = days;
    let mut year = 1970i32;

    loop {
        let days_in_year = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            366
        } else {
            365
        };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_months: [i64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];

    let mut month = 0usize;
    for (i, &dim) in days_in_months.iter().enumerate() {
        if remaining_days < dim {
            month = i;
            break;
        }
        remaining_days -= dim;
    }

    let day = remaining_days + 1;
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        year,
        month + 1,
        day,
        hours,
        minutes,
        seconds
    )
}

/// 非交互模式入口
fn run_non_interactive(cli: Cli) -> Result<()> {
    let config = AppConfig::load();

    let sort_order = match cli.sort.as_str() {
        "name" => SortOrder::ByName,
        "time" => SortOrder::ByTime,
        _ => SortOrder::BySize,
    };

    let scan_target = cli.scan.as_ref().expect("scan target is required");
    let scan_target_name = match scan_target {
        ScanTarget::Preset => "preset".to_string(),
        ScanTarget::Home => "home".to_string(),
        ScanTarget::Path(p) => p.display().to_string(),
    };

    eprintln!("VAC - 非交互模式");
    eprintln!("扫描目标: {}", scan_target_name);

    let mut entries = run_scan_blocking(scan_target, &config)?;
    sort_entries(&mut entries, &sort_order);

    let total_size: u64 = entries.iter().filter_map(|e| e.size).sum();

    // 构建报告条目
    let report_entries: Vec<ReportEntry> = entries
        .iter()
        .map(|e| ReportEntry {
            path: e.path.display().to_string(),
            name: e.name.clone(),
            kind: match e.kind {
                EntryKind::Directory => "directory".to_string(),
                EntryKind::File => "file".to_string(),
            },
            size: e.size,
            size_display: e
                .size
                .map(format_size)
                .unwrap_or_else(|| "未知".to_string()),
            modified_at: e.modified_at.as_ref().map(format_time_cli),
        })
        .collect();

    // Dry-run
    let dry_run_report = if cli.dry_run {
        let result = Cleaner::dry_run(&entries);
        Some(DryRunReport {
            total_files: result.total_files,
            total_dirs: result.total_dirs,
            total_size: result.total_size,
            total_size_display: format_size(result.total_size),
            items: result
                .items
                .iter()
                .map(|item| DryRunReportItem {
                    path: item.path.display().to_string(),
                    file_count: item.file_count,
                    dir_count: item.dir_count,
                    size: item.size,
                    size_display: format_size(item.size),
                })
                .collect(),
        })
    } else {
        None
    };

    // 清理
    let use_trash = cli.trash || config.safety.move_to_trash;
    let clean_report = if cli.clean && !cli.dry_run {
        // 安全检查
        for entry in &entries {
            if !Cleaner::is_safe_to_delete(&entry.path) {
                return Err(color_eyre::eyre::eyre!(
                    "不安全的路径: {}",
                    entry.path.display()
                ));
            }
        }

        let item_count = entries.len();
        let result = if use_trash {
            Cleaner::trash_items(&entries)
        } else {
            Cleaner::clean(&entries)
        };

        Some(CleanReport {
            success: result.success,
            freed_space: result.freed_space,
            freed_space_display: format_size(result.freed_space),
            item_count,
            use_trash,
            errors: result.errors,
        })
    } else {
        None
    };

    let report = ScanReport {
        scan_target: scan_target_name.clone(),
        sort_order: cli.sort.clone(),
        total_items: entries.len(),
        total_size,
        total_size_display: format_size(total_size),
        entries: report_entries,
        dry_run: dry_run_report,
        clean_result: clean_report,
    };

    // 输出结果
    if let Some(ref output_path) = cli.output {
        let json = serde_json::to_string_pretty(&report)?;
        std::fs::write(output_path, &json)?;
        eprintln!("报告已写入: {}", output_path.display());
    } else {
        // 输出到终端
        print_report_to_terminal(&report, &entries, use_trash);
    }

    Ok(())
}

/// 将报告输出到终端
fn print_report_to_terminal(report: &ScanReport, entries: &[CleanableEntry], use_trash: bool) {
    println!();
    println!(
        "扫描结果: {} 个项目 | 总大小: {}",
        report.total_items, report.total_size_display
    );
    println!("{}", "─".repeat(70));

    for entry in entries {
        let kind_icon = match entry.kind {
            EntryKind::Directory => "📁",
            EntryKind::File => "📄",
        };
        let size_str = entry
            .size
            .map(format_size)
            .unwrap_or_else(|| "未知".to_string());
        let time_str = entry
            .modified_at
            .as_ref()
            .map(|t| format!("  {}", format_time_cli(t)))
            .unwrap_or_default();

        println!(
            "  {} {:>10}  {}{}",
            kind_icon, size_str, entry.name, time_str
        );
    }
    println!("{}", "─".repeat(70));

    // Dry-run 结果
    if let Some(ref dry_run) = report.dry_run {
        println!();
        println!("Dry-run 预览:");
        println!(
            "  总计: {} 个文件 / {} 个目录 / {}",
            dry_run.total_files, dry_run.total_dirs, dry_run.total_size_display
        );
        for item in &dry_run.items {
            println!(
                "  • {} — {} 文件 / {} 目录 / {}",
                item.path, item.file_count, item.dir_count, item.size_display
            );
        }
    }

    // 清理结果
    if let Some(ref clean) = report.clean_result {
        println!();
        let action = if use_trash {
            "移至回收站"
        } else {
            "已删除"
        };
        if clean.success {
            println!(
                "{}: {} ({} 个项目)",
                action, clean.freed_space_display, clean.item_count
            );
        } else {
            println!("清理部分失败:");
            for err in &clean.errors {
                println!("  ✗ {}", err);
            }
        }
    }

    println!();
}
