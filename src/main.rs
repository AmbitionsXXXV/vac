use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use color_eyre::Result;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use vac::app::{App, EntryKind, Mode};
use vac::cleaner::Cleaner;
use vac::scanner::{ScanKind, ScanMessage, Scanner};
use vac::ui;

fn main() -> Result<()> {
    color_eyre::install()?;

    let mut terminal = ratatui::init();
    let result = run(&mut terminal);

    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
    let mut app = App::new();
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

            // 确认删除界面
            if app.mode == Mode::Confirm {
                if let Some(rx) = handle_confirm_mode(&mut app, key.code, &cancel_generation) {
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
                    scan_rx = start_root_scan(&mut app, &cancel_generation);
                }
                KeyCode::Char('S') => {
                    // Shift+S: 扫描主目录
                    if let Some(scanner) = Scanner::new() {
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
                        app.navigation.enter(target.clone());
                        scan_rx = start_dir_scan(&mut app, target, &cancel_generation);
                    }
                }
                KeyCode::Backspace | KeyCode::Esc => {
                    if app.navigation.current_path.is_some() {
                        if app.scan_in_progress {
                            cancel_scan(&mut app, &cancel_generation, &mut scan_rx);
                        }
                        app.navigation.back();
                        if let Some(path) = app.navigation.current_path.clone() {
                            scan_rx = start_dir_scan(&mut app, path, &cancel_generation);
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
) -> Option<Receiver<ScanMessage>> {
    match key {
        KeyCode::Enter => {
            let rx = execute_clean(app, cancel_generation);
            app.mode = Mode::Normal;
            rx
        }
        KeyCode::Esc => {
            app.cancel_confirm();
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

    thread::spawn(move || {
        if let Some(scanner) = Scanner::new() {
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
    let result = Cleaner::clean(&selected_items);

    if result.success {
        app.last_clean_result = Some((result.freed_space, item_count));
        app.clear_selections();

        if let Some(path) = app.navigation.current_path.clone() {
            start_dir_scan(app, path, cancel_generation)
        } else {
            start_root_scan(app, cancel_generation)
        }
    } else {
        let error_msg = result.errors.join("\n");
        app.set_error(format!("部分清理失败:\n{}", error_msg));
        None
    }
}
