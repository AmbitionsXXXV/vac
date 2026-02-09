use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, Gauge, List, ListItem, Padding, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
};

use std::path::PathBuf;
use std::time::SystemTime;

use crate::app::{App, EntryKind, Mode, SortOrder};
use crate::scanner::format_size;

/// UI 颜色主题
pub struct Theme {
    pub primary: Color,
    pub secondary: Color,
    pub accent: Color,
    pub warning: Color,
    pub danger: Color,
    pub success: Color,
    pub text: Color,
    pub text_dim: Color,
    pub bg: Color,
    pub bg_highlight: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary: Color::Cyan,
            secondary: Color::Blue,
            accent: Color::Magenta,
            warning: Color::Yellow,
            danger: Color::Red,
            success: Color::Green,
            text: Color::White,
            text_dim: Color::DarkGray,
            bg: Color::Reset,
            bg_highlight: Color::DarkGray,
        }
    }
}

/// 渲染整个 UI
pub fn render(frame: &mut Frame, app: &mut App) {
    let theme = Theme::default();

    let [header_area, main_area, footer_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(3),
    ])
    .areas(frame.area());

    render_header(frame, header_area, app, &theme);
    render_main(frame, main_area, app, &theme);
    render_footer(frame, footer_area, app, &theme);

    // 渲染覆盖层
    match app.mode {
        Mode::Help => render_help_popup(frame, &theme),
        Mode::Confirm => render_confirm_popup(frame, app, &theme),
        Mode::InputPath => render_input_popup(frame, app, &theme),
        Mode::Search => render_search_bar(frame, app, &theme),
        Mode::Stats => render_stats_popup(frame, app, &theme),
        _ => {}
    }

    // 渲染错误消息
    if app.error_message.is_some() {
        render_error_popup(frame, app, &theme);
    }
}

/// 渲染头部
fn render_header(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let title = vec![
        Span::styled(" VAC ", Style::default().fg(theme.primary).bold()),
        Span::styled("- macOS 磁盘清理工具", Style::default().fg(theme.text_dim)),
    ];

    let stats = format!(
        "路径: {} | 总计: {} ({} 项) | 已选: {} ({} 项)",
        app.breadcrumb(),
        format_size(app.total_size),
        app.entries.len(),
        format_size(app.selected_size),
        app.selections.len()
    );

    let header = Paragraph::new(Line::from(title))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.primary))
                .title_bottom(Line::from(stats).right_aligned()),
        )
        .alignment(Alignment::Center);

    frame.render_widget(header, area);
}

/// 渲染主内容区域
fn render_main(frame: &mut Frame, area: Rect, app: &mut App, theme: &Theme) {
    match app.mode {
        Mode::Scanning => render_scanning(frame, area, app, theme),
        _ => render_list(frame, area, app, theme),
    }
}

/// 渲染扫描进度
fn render_scanning(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let [_, center, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(5),
        Constraint::Fill(1),
    ])
    .areas(area);

    let [_, gauge_area, _] = Layout::horizontal([
        Constraint::Percentage(20),
        Constraint::Percentage(60),
        Constraint::Percentage(20),
    ])
    .areas(center);

    let gauge = Gauge::default()
        .block(
            Block::default()
                .title(" 扫描中... ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.primary)),
        )
        .gauge_style(Style::default().fg(theme.accent).bg(theme.bg_highlight))
        .percent(app.scan_progress as u16)
        .label(format!(
            "{}% | 已发现: {}",
            app.scan_progress,
            format_size(app.total_size)
        ));

    frame.render_widget(gauge, gauge_area);

    // 显示当前扫描路径
    let path_area = Rect::new(gauge_area.x, gauge_area.y + 5, gauge_area.width, 1);
    let path_text = Paragraph::new(app.current_scan_path.clone())
        .style(Style::default().fg(theme.text_dim))
        .alignment(Alignment::Center);
    frame.render_widget(path_text, path_area);
}

/// 渲染可清理项目列表
fn render_list(frame: &mut Frame, area: Rect, app: &mut App, theme: &Theme) {
    // 更新可视区域高度（减去边框 2 行）
    app.visible_height = area.height.saturating_sub(2) as usize;
    if app.entries.is_empty() {
        let content = if app.scan_in_progress {
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "正在加载目录...",
                    Style::default().fg(theme.text_dim),
                )),
            ]
        } else {
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "VAC - macOS 磁盘清理工具",
                    Style::default().fg(theme.primary).bold(),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled("  s  ", Style::default().fg(theme.accent).bold()),
                    Span::styled("扫描预设可清理目录", Style::default().fg(theme.text)),
                ]),
                Line::from(vec![
                    Span::styled("  S  ", Style::default().fg(theme.accent).bold()),
                    Span::styled("扫描用户主目录", Style::default().fg(theme.text)),
                ]),
                Line::from(vec![
                    Span::styled("  d  ", Style::default().fg(theme.accent).bold()),
                    Span::styled("输入自定义路径扫描", Style::default().fg(theme.text)),
                ]),
                Line::from(vec![
                    Span::styled("  ?  ", Style::default().fg(theme.accent).bold()),
                    Span::styled("查看完整帮助", Style::default().fg(theme.text)),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "提示: 清理前请先备份重要数据",
                    Style::default().fg(theme.warning),
                )),
            ]
        };
        let empty_text = Paragraph::new(content).alignment(Alignment::Center).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.secondary))
                .title(" 可清理项目 "),
        );
        frame.render_widget(empty_text, area);
        return;
    }

    let items: Vec<ListItem> = app
        .entries
        .iter()
        .map(|entry| {
            let selected = app.is_selected(&entry.path);
            let checkbox = if selected { "[✓]" } else { "[ ]" };
            let size = entry
                .size
                .map(format_size)
                .unwrap_or_else(|| "…".to_string());
            let name = match entry.kind {
                EntryKind::Directory => format!("{}/", entry.name),
                EntryKind::File => entry.name.clone(),
            };
            let time_str = entry
                .modified_at
                .as_ref()
                .map(format_time)
                .unwrap_or_default();
            let mut spans = vec![
                Span::styled(
                    checkbox,
                    Style::default().fg(if selected {
                        theme.success
                    } else {
                        theme.text_dim
                    }),
                ),
                Span::raw(" "),
                Span::styled(name, Style::default().fg(theme.text)),
                Span::raw(" "),
                Span::styled(format!("({})", size), Style::default().fg(theme.warning)),
            ];
            if !time_str.is_empty() {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(time_str, Style::default().fg(theme.text_dim)));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.secondary))
                .title(" 可清理项目 ")
                .padding(Padding::horizontal(1)),
        )
        .highlight_style(
            Style::default()
                .bg(theme.bg_highlight)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, &mut app.list_state);

    // 滚动条
    if app.entries.len() > app.visible_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        let mut scrollbar_state =
            ScrollbarState::new(app.entries.len()).position(app.list_state.selected().unwrap_or(0));
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

/// 渲染底部状态栏
fn render_footer(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let sort_indicator = match app.sort_order {
        SortOrder::ByName => "[排序:名称]",
        SortOrder::BySize => "[排序:大小]",
        SortOrder::ByTime => "[排序:时间]",
    };

    let base_help = format!(
        "s: 扫描 | S: 扫描主目录 | d: 自定义路径 | o: 排序 {} | t: 统计 | Space: 选择 | c: 清理 | ?: 帮助 | q: 退出",
        sort_indicator
    );

    let help_text = match app.mode {
        Mode::Normal => {
            if let Some((freed, count)) = app.last_clean_result {
                format!(
                    "已释放 {} ({} 个项目) | {}",
                    format_size(freed),
                    count,
                    base_help
                )
            } else if app.scan_in_progress {
                format!("{} | 扫描中...", base_help)
            } else {
                base_help
            }
        }
        Mode::Scanning => "扫描中，请稍候... | Esc: 取消".to_string(),
        Mode::Confirm => {
            if app.use_trash {
                "Enter: 确认移至回收站 | d: 详情预览 | Esc: 取消".to_string()
            } else {
                "Enter: 确认删除 | d: 详情预览 | Esc: 取消".to_string()
            }
        }
        Mode::Help => "按任意键关闭帮助".to_string(),
        Mode::Stats => "按任意键关闭统计".to_string(),
        Mode::InputPath => "输入路径后按 Enter 确认 | Tab: 补全 | Esc: 取消".to_string(),
        Mode::Search => "Enter: 确认搜索 | Esc: 取消搜索".to_string(),
    };

    let footer = Paragraph::new(help_text)
        .style(Style::default().fg(theme.text_dim))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.secondary)),
        );

    frame.render_widget(footer, area);
}

/// 渲染帮助弹窗
fn render_help_popup(frame: &mut Frame, theme: &Theme) {
    let area = centered_rect(70, 80, frame.area());
    frame.render_widget(Clear, area);

    let help_content = vec![
        Line::from(Span::styled(
            "快捷键说明",
            Style::default().fg(theme.primary).bold(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "扫描操作",
            Style::default().fg(theme.secondary).bold(),
        )),
        Line::from(vec![
            Span::styled("  s          ", Style::default().fg(theme.accent)),
            Span::raw("扫描预设可清理目录"),
        ]),
        Line::from(vec![
            Span::styled("  S          ", Style::default().fg(theme.accent)),
            Span::raw("扫描用户主目录"),
        ]),
        Line::from(vec![
            Span::styled("  d          ", Style::default().fg(theme.accent)),
            Span::raw("输入自定义路径扫描"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "浏览与排序",
            Style::default().fg(theme.secondary).bold(),
        )),
        Line::from(vec![
            Span::styled("  Enter      ", Style::default().fg(theme.accent)),
            Span::raw("进入目录"),
        ]),
        Line::from(vec![
            Span::styled("  Backspace  ", Style::default().fg(theme.accent)),
            Span::raw("返回上一级"),
        ]),
        Line::from(vec![
            Span::styled("  Esc        ", Style::default().fg(theme.accent)),
            Span::raw("返回上一级/取消扫描"),
        ]),
        Line::from(vec![
            Span::styled("  ↑/k        ", Style::default().fg(theme.accent)),
            Span::raw("向上移动"),
        ]),
        Line::from(vec![
            Span::styled("  ↓/j        ", Style::default().fg(theme.accent)),
            Span::raw("向下移动"),
        ]),
        Line::from(vec![
            Span::styled("  g/G        ", Style::default().fg(theme.accent)),
            Span::raw("跳到顶部/底部"),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+d/u   ", Style::default().fg(theme.accent)),
            Span::raw("向下/上翻半页"),
        ]),
        Line::from(vec![
            Span::styled("  PgDn/PgUp  ", Style::default().fg(theme.accent)),
            Span::raw("向下/上翻半页"),
        ]),
        Line::from(vec![
            Span::styled("  /          ", Style::default().fg(theme.accent)),
            Span::raw("搜索/过滤列表"),
        ]),
        Line::from(vec![
            Span::styled("  o          ", Style::default().fg(theme.accent)),
            Span::raw("切换排序方式 (名称/大小/时间)"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "选择与清理",
            Style::default().fg(theme.secondary).bold(),
        )),
        Line::from(vec![
            Span::styled("  Space      ", Style::default().fg(theme.accent)),
            Span::raw("选择/取消选择当前项"),
        ]),
        Line::from(vec![
            Span::styled("  a          ", Style::default().fg(theme.accent)),
            Span::raw("全选/取消全选"),
        ]),
        Line::from(vec![
            Span::styled("  c          ", Style::default().fg(theme.accent)),
            Span::raw("执行清理"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "其他",
            Style::default().fg(theme.secondary).bold(),
        )),
        Line::from(vec![
            Span::styled("  t          ", Style::default().fg(theme.accent)),
            Span::raw("空间占用统计"),
        ]),
        Line::from(vec![
            Span::styled("  ?          ", Style::default().fg(theme.accent)),
            Span::raw("显示/隐藏帮助"),
        ]),
        Line::from(vec![
            Span::styled("  q          ", Style::default().fg(theme.accent)),
            Span::raw("退出程序"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "注意: 清理操作不可逆，请谨慎操作！",
            Style::default().fg(theme.warning),
        )),
    ];

    let help = Paragraph::new(help_content)
        .block(
            Block::default()
                .title(" 帮助 ")
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(theme.primary))
                .padding(Padding::uniform(1)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(help, area);
}

/// 渲染路径输入弹窗
fn render_input_popup(frame: &mut Frame, app: &App, theme: &Theme) {
    // 动态计算弹窗高度：基础行数 + 候选列表行数
    let max_visible_completions = 5;
    let completion_count = app.tab_completions.len().min(max_visible_completions);
    let has_completions = !app.tab_completions.is_empty();
    // 基础: 标题(1) + 空行(1) + 提示(1) + 空行(1) + 输入行(1) + 空行(1) + 操作提示(1)
    //       + padding(2) + border(2) = 12 行
    // 候选列表: 空行(1) + 候选项(N) + 可能的省略提示(1)
    let extra_lines = if has_completions {
        1 + completion_count + if app.tab_completions.len() > max_visible_completions { 1 } else { 0 }
    } else {
        0
    };
    let popup_height = (12 + extra_lines) as u16;
    let percent_y = ((popup_height as u32) * 100 / frame.area().height as u32).max(20) as u16;
    let area = centered_rect(60, percent_y.min(80), frame.area());
    frame.render_widget(Clear, area);

    let input_display = if app.input_buffer.is_empty() {
        Span::styled(
            "输入路径 (支持 ~ 表示主目录)",
            Style::default().fg(theme.text_dim),
        )
    } else {
        Span::styled(&app.input_buffer, Style::default().fg(theme.text))
    };

    let mut content = vec![
        Line::from(Span::styled(
            "磁盘扫描",
            Style::default().fg(theme.primary).bold(),
        )),
        Line::from(""),
        Line::from("请输入要扫描的目录路径:"),
        Line::from(""),
        Line::from(vec![
            Span::raw("> "),
            input_display,
            Span::styled("█", Style::default().fg(theme.accent)),
        ]),
    ];

    // 显示 Tab 补全候选列表
    if has_completions {
        content.push(Line::from(""));
        let current_index = app.tab_completion_index.unwrap_or(0);
        for (i, completion) in app.tab_completions.iter().enumerate().take(max_visible_completions)
        {
            let is_selected = i == current_index;
            if is_selected {
                content.push(Line::from(vec![
                    Span::styled("  ▶ ", Style::default().fg(theme.accent)),
                    Span::styled(
                        completion.as_str(),
                        Style::default().fg(theme.accent).bold(),
                    ),
                ]));
            } else {
                content.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(completion.as_str(), Style::default().fg(theme.text_dim)),
                ]));
            }
        }
        if app.tab_completions.len() > max_visible_completions {
            content.push(Line::from(Span::styled(
                format!("    ... 共 {} 项", app.tab_completions.len()),
                Style::default().fg(theme.text_dim),
            )));
        }
    }

    content.push(Line::from(""));
    content.push(Line::from(vec![
        Span::styled("Enter", Style::default().fg(theme.accent)),
        Span::raw(" 确认 | "),
        Span::styled("Tab", Style::default().fg(theme.accent)),
        Span::raw(" 补全 | "),
        Span::styled("Esc", Style::default().fg(theme.accent)),
        Span::raw(" 取消"),
    ]));

    let input_box = Paragraph::new(content)
        .block(
            Block::default()
                .title(" 输入路径 ")
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(theme.primary))
                .padding(Padding::uniform(1)),
        )
        .alignment(Alignment::Center);

    frame.render_widget(input_box, area);
}

/// 渲染确认删除弹窗（可滚动预览列表）
fn render_confirm_popup(frame: &mut Frame, app: &App, theme: &Theme) {
    let area = centered_rect(60, 60, frame.area());
    frame.render_widget(Clear, area);

    if app.dry_run_active {
        render_dry_run_view(frame, area, app, theme);
        return;
    }

    let selected_count = app.selections.len();

    // 收集待删路径，按大小降序
    let mut items: Vec<(PathBuf, u64)> = app
        .selections
        .iter()
        .map(|(path, entry)| (path.clone(), entry.size.unwrap_or(0)))
        .collect();
    items.sort_by(|a, b| b.1.cmp(&a.1));

    // 头部信息行
    let action_title = if app.use_trash {
        "⚠ 确认移至回收站"
    } else {
        "⚠ 确认删除"
    };
    let mut lines = vec![
        Line::from(Span::styled(
            action_title,
            Style::default().fg(theme.warning).bold(),
        )),
        Line::from(""),
        Line::from(format!(
            "共 {} 个项目 | 释放空间: {}",
            selected_count,
            format_size(app.selected_size)
        )),
        Line::from(""),
    ];

    // 可视列表区高度 = popup 总高 - 边框(2) - padding(2) - 头(4) - 尾(3)
    let visible_height = area.height.saturating_sub(11) as usize;
    let scroll = app
        .confirm_scroll
        .min(items.len().saturating_sub(visible_height));

    for (path, size) in items.iter().skip(scroll).take(visible_height) {
        let name: String = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());
        lines.push(Line::from(vec![
            Span::styled("  • ", Style::default().fg(theme.text_dim)),
            Span::styled(name, Style::default().fg(theme.text)),
            Span::raw("  "),
            Span::styled(
                format!("({})", format_size(*size)),
                Style::default().fg(theme.warning),
            ),
        ]));
    }

    if items.len() > visible_height {
        lines.push(Line::from(Span::styled(
            format!("  ... 共 {} 项，j/k 滚动", items.len()),
            Style::default().fg(theme.text_dim),
        )));
    }

    lines.push(Line::from(""));
    let warning_text = if app.use_trash {
        "文件将移至系统回收站，可从回收站恢复"
    } else {
        "此操作不可逆！"
    };
    let warning_color = if app.use_trash {
        theme.warning
    } else {
        theme.danger
    };
    lines.push(Line::from(Span::styled(
        warning_text,
        Style::default().fg(warning_color),
    )));
    lines.push(Line::from(vec![
        Span::styled("Enter", Style::default().fg(theme.accent)),
        Span::raw(" 确认 | "),
        Span::styled("d", Style::default().fg(theme.accent)),
        Span::raw(" 详情预览 | "),
        Span::styled("Esc", Style::default().fg(theme.accent)),
        Span::raw(" 取消 | "),
        Span::styled("j/k", Style::default().fg(theme.accent)),
        Span::raw(" 滚动"),
    ]));

    let confirm = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(theme.warning))
            .padding(Padding::uniform(1)),
    );

    frame.render_widget(confirm, area);
}

/// 渲染 dry-run 详情视图
fn render_dry_run_view(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let mut lines = vec![
        Line::from(Span::styled(
            "🔍 删除预览 (Dry-run)",
            Style::default().fg(theme.primary).bold(),
        )),
        Line::from(""),
    ];

    if let Some(ref result) = app.dry_run_result {
        lines.push(Line::from(vec![
            Span::styled("总计: ", Style::default().fg(theme.text)),
            Span::styled(
                format!("{} 个文件", result.total_files),
                Style::default().fg(theme.warning),
            ),
            Span::raw(" / "),
            Span::styled(
                format!("{} 个目录", result.total_dirs),
                Style::default().fg(theme.secondary),
            ),
            Span::raw(" / "),
            Span::styled(
                format_size(result.total_size),
                Style::default().fg(theme.danger),
            ),
        ]));
        lines.push(Line::from(""));

        let visible_height = area.height.saturating_sub(11) as usize;
        let scroll = app
            .confirm_scroll
            .min(result.items.len().saturating_sub(visible_height));

        for item in result.items.iter().skip(scroll).take(visible_height) {
            let name: String = item
                .path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| item.path.display().to_string());
            lines.push(Line::from(vec![
                Span::styled("  • ", Style::default().fg(theme.text_dim)),
                Span::styled(name, Style::default().fg(theme.text)),
            ]));
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(
                    format!("{} 文件", item.file_count),
                    Style::default().fg(theme.warning),
                ),
                Span::raw(" / "),
                Span::styled(
                    format!("{} 目录", item.dir_count),
                    Style::default().fg(theme.secondary),
                ),
                Span::raw(" / "),
                Span::styled(format_size(item.size), Style::default().fg(theme.danger)),
            ]));
        }

        if result.items.len() > visible_height {
            lines.push(Line::from(Span::styled(
                format!("  ... 共 {} 项，j/k 滚动", result.items.len()),
                Style::default().fg(theme.text_dim),
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Enter", Style::default().fg(theme.accent)),
        Span::raw(" 确认删除 | "),
        Span::styled("d", Style::default().fg(theme.accent)),
        Span::raw(" 返回列表 | "),
        Span::styled("Esc", Style::default().fg(theme.accent)),
        Span::raw(" 取消"),
    ]));

    let popup = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(theme.primary))
            .padding(Padding::uniform(1)),
    );

    frame.render_widget(popup, area);
}

/// 渲染错误弹窗
fn render_error_popup(frame: &mut Frame, app: &App, theme: &Theme) {
    if let Some(ref msg) = app.error_message {
        let area = centered_rect(60, 20, frame.area());
        frame.render_widget(Clear, area);

        let content = vec![
            Line::from(Span::styled(
                "❌ 错误",
                Style::default().fg(theme.danger).bold(),
            )),
            Line::from(""),
            Line::from(msg.as_str()),
            Line::from(""),
            Line::from("按 Enter 或 Esc 关闭"),
        ];

        let error = Paragraph::new(content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Double)
                    .border_style(Style::default().fg(theme.danger))
                    .padding(Padding::uniform(1)),
            )
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        frame.render_widget(error, area);
    }
}

/// 渲染统计面板弹窗
fn render_stats_popup(frame: &mut Frame, app: &App, theme: &Theme) {
    let area = centered_rect(70, 70, frame.area());
    frame.render_widget(Clear, area);

    let stats = app.get_category_stats();
    let total_size: u64 = stats.iter().map(|(_, s)| *s).sum();

    let mut lines = vec![
        Line::from(Span::styled(
            "空间占用统计",
            Style::default().fg(theme.primary).bold(),
        )),
        Line::from(""),
    ];

    // 进度条宽度（字符数）
    let bar_width = 20usize;

    for (category_name, size) in &stats {
        let percent = if total_size > 0 {
            (*size as f64 / total_size as f64 * 100.0) as u16
        } else {
            0
        };
        let filled = (percent as usize * bar_width / 100).min(bar_width);
        let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);

        // 分类名固定宽度对齐
        let padded_name = format!("{:<14}", category_name);
        let size_str = format!("{:>10}", format_size(*size));

        lines.push(Line::from(vec![
            Span::styled(padded_name, Style::default().fg(theme.text)),
            Span::raw(" "),
            Span::styled(size_str, Style::default().fg(theme.warning)),
            Span::raw("  "),
            Span::styled(bar, Style::default().fg(theme.accent)),
            Span::raw("  "),
            Span::styled(
                format!("{:>3}%", percent),
                Style::default().fg(theme.text_dim),
            ),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("总计: ", Style::default().fg(theme.text)),
        Span::styled(
            format_size(total_size),
            Style::default().fg(theme.warning).bold(),
        ),
        Span::raw(format!(" ({} 个分类)", stats.len())),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "按任意键关闭",
        Style::default().fg(theme.text_dim),
    )));

    let popup = Paragraph::new(lines).block(
        Block::default()
            .title(" 统计 ")
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(theme.primary))
            .padding(Padding::uniform(1)),
    );

    frame.render_widget(popup, area);
}

/// 渲染搜索栏（底部浮层）
fn render_search_bar(frame: &mut Frame, app: &App, theme: &Theme) {
    let area = frame.area();
    let bar_area = Rect::new(
        area.x,
        area.y + area.height.saturating_sub(3),
        area.width,
        3,
    );
    frame.render_widget(Clear, bar_area);

    let search_display = if app.search_query.is_empty() {
        Span::styled("输入关键词搜索...", Style::default().fg(theme.text_dim))
    } else {
        Span::styled(&app.search_query, Style::default().fg(theme.text))
    };

    let content = Line::from(vec![
        Span::styled("/", Style::default().fg(theme.accent).bold()),
        Span::raw(" "),
        search_display,
        Span::styled("█", Style::default().fg(theme.accent)),
    ]);

    let bar = Paragraph::new(content).block(
        Block::default()
            .title(" 搜索 ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.accent)),
    );

    frame.render_widget(bar, bar_area);
}

/// 格式化 SystemTime 为 "YYYY-MM-DD" 字符串
fn format_time(time: &SystemTime) -> String {
    let duration = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs() as i64;

    // 简单日期计算（不引入额外依赖）
    let days = secs / 86400;
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
    format!("{:04}-{:02}-{:02}", year, month + 1, day)
}

/// 计算居中矩形区域
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let [_, center, _] = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .areas(area);

    let [_, center, _] = Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .areas(center);

    center
}
