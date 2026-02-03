use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Gauge, List, ListItem, Padding, Paragraph, Wrap},
};

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
        "路径: {} | 总计: {} | 已选: {}",
        app.breadcrumb(),
        format_size(app.total_size),
        format_size(app.selected_size)
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
        .label(format!("{}%", app.scan_progress));

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
    if app.entries.is_empty() {
        let empty_msg = if app.scan_in_progress {
            "正在加载目录..."
        } else {
            "按 's' 开始扫描磁盘"
        };
        let empty_text = Paragraph::new(empty_msg)
            .style(Style::default().fg(theme.text_dim))
            .alignment(Alignment::Center)
            .block(
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
            let line = Line::from(vec![
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
            ]);
            ListItem::new(line)
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
}

/// 渲染底部状态栏
fn render_footer(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let sort_indicator = match app.sort_order {
        SortOrder::ByName => "[排序:名称]",
        SortOrder::BySize => "[排序:大小]",
    };

    let base_help = format!(
        "s: 扫描 | S: 扫描主目录 | d: 自定义路径 | o: 切换排序 {} | ↑/↓: 移动 | Space: 选择 | c: 清理 | ?: 帮助 | q: 退出",
        sort_indicator
    );

    let help_text = match app.mode {
        Mode::Normal => {
            if app.scan_in_progress {
                format!("{} | 扫描中...", base_help)
            } else {
                base_help
            }
        }
        Mode::Scanning => "扫描中，请稍候... | Esc: 取消".to_string(),
        Mode::Confirm => "Enter: 确认删除 | Esc: 取消".to_string(),
        Mode::Help => "按任意键关闭帮助".to_string(),
        Mode::InputPath => "输入路径后按 Enter 确认 | Esc: 取消".to_string(),
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
            Span::styled("  o          ", Style::default().fg(theme.accent)),
            Span::raw("切换排序方式 (名称/大小)"),
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
    let area = centered_rect(60, 25, frame.area());
    frame.render_widget(Clear, area);

    let input_display = if app.input_buffer.is_empty() {
        Span::styled("输入路径 (支持 ~ 表示主目录)", Style::default().fg(theme.text_dim))
    } else {
        Span::styled(&app.input_buffer, Style::default().fg(theme.text))
    };

    let content = vec![
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
        Line::from(""),
        Line::from(vec![
            Span::styled("Enter", Style::default().fg(theme.accent)),
            Span::raw(" 确认 | "),
            Span::styled("Esc", Style::default().fg(theme.accent)),
            Span::raw(" 取消"),
        ]),
    ];

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

/// 渲染确认删除弹窗
fn render_confirm_popup(frame: &mut Frame, app: &App, theme: &Theme) {
    let area = centered_rect(50, 30, frame.area());
    frame.render_widget(Clear, area);

    let selected_count = app.selections.len();
    let content = vec![
        Line::from(Span::styled(
            "⚠ 确认删除",
            Style::default().fg(theme.warning).bold(),
        )),
        Line::from(""),
        Line::from(format!("即将删除 {} 个项目", selected_count)),
        Line::from(format!("释放空间: {}", format_size(app.selected_size))),
        Line::from(""),
        Line::from(Span::styled(
            "此操作不可逆！",
            Style::default().fg(theme.danger),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Enter", Style::default().fg(theme.accent)),
            Span::raw(" 确认 | "),
            Span::styled("Esc", Style::default().fg(theme.accent)),
            Span::raw(" 取消"),
        ]),
    ];

    let confirm = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(theme.warning))
                .padding(Padding::uniform(1)),
        )
        .alignment(Alignment::Center);

    frame.render_widget(confirm, area);
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
            Line::from("按任意键关闭"),
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
