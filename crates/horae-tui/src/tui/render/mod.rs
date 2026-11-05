//! TUI 渲染，按渲染职责拆分：
//! - [`banners`] — 顶部横幅（周回顾进度 / 番茄成就）与状态栏。
//! - [`input`] — 输入/编辑弹层、快速录入实时解析预览与 Tab 补全下拉。
//! - [`popups`] — 确认弹层、今日任务/到期提醒弹窗与专注模式进度环。
//! - [`help`] — 语法说明面板与左侧引导栏内容。
//! - [`detail`] — 右侧任务详情面板内容。
//!
//! `AppRender` trait 与其实现（视图分发、三栏布局、列表/工作流/详情/抽屉）
//! 是一个整体，留在本文件；各子模块通过 `pub(super)` 方法供其调用。

use super::app::{App, Mode, Pane, View};
use super::icons::Icon;
use super::keys::{ctx_of, help_rows, KeyGroup};
use super::mouse::{HitTarget, ScrollArea};
use super::ui::build_list_items;
use ratatui::symbols::border;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

pub(crate) trait AppRender {
    fn render(&mut self, f: &mut Frame);
    fn render_lock_screen(&mut self, f: &mut Frame, size: Rect);
    fn render_focus_mode(&mut self, f: &mut Frame, area: Rect);
    fn render_help_drawer(&mut self, f: &mut Frame, area: Rect);
    fn render_syntax_drawer(&mut self, f: &mut Frame, area: Rect);
    fn centered_rect(&self, percent_x: u16, percent_y: u16, r: Rect) -> Rect;
    fn render_guide(&mut self, f: &mut Frame, area: Rect);
    fn render_list(&mut self, f: &mut Frame, area: Rect);
    fn render_workflow(&mut self, f: &mut Frame, area: Rect);
    fn workflow_lines(&self) -> Vec<Line<'static>>;
    fn render_workflow_side(&mut self, f: &mut Frame, area: Rect);
    fn workflow_side_lines(&self) -> Vec<Line<'static>>;
    fn render_detail(&mut self, f: &mut Frame, area: Rect);
}

impl<'a> AppRender for App<'a> {
    fn render(&mut self, f: &mut ratatui::Frame) {
        let size = f.area();
        // 每帧重建命中映射：上一帧的坐标在缩放/筛选/滚动后不再成立。
        self.hit_map.clear();

        // ── 密码锁定模式：全屏接管，保护数据安全 ──
        if self.locked {
            self.hit_map.modal();
            self.render_lock_screen(f, size);
            return;
        }

        // ── 番茄专注模式：全屏接管 ──
        {
            let is_active = self.pomo.phase != horae_core::model::pomodoro::Phase::Idle;
            if is_active {
                self.hide_pomo_banner = false; // reset so it shows up next time it becomes Idle
                self.hit_map.modal();
                self.render_focus_mode(f, size);
                return;
            }
        }

        // ── 快速录入纯净专注模式：全屏接管，消除多栏与横幅等背景干扰 ──
        if self.is_zen_capturing() {
            self.hit_map.modal();
            f.render_widget(ratatui::widgets::Clear, size);
            let bg_block = Block::default().style(Style::default().bg(self.theme.bg));
            f.render_widget(bg_block, size);

            // 屏幕最底部单行 Zen 快捷键指示
            let hint_line = Line::from(vec![
                Span::styled(
                    " [Enter] ",
                    Style::default()
                        .fg(self.theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    tr!(self.lang, "保存入库", "save"),
                    Style::default().fg(self.theme.text_dim),
                ),
                Span::styled(
                    "  •  [Tab] ",
                    Style::default()
                        .fg(self.theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    tr!(self.lang, "智能补全", "complete"),
                    Style::default().fg(self.theme.text_dim),
                ),
                Span::styled(
                    "  •  [Ctrl+P] ",
                    Style::default()
                        .fg(self.theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    if self.show_syntax {
                        tr!(self.lang, "收起语法", "hide syntax")
                    } else {
                        tr!(self.lang, "语法速查", "syntax")
                    },
                    Style::default().fg(self.theme.text_dim),
                ),
                Span::styled(
                    "  •  [Esc] ",
                    Style::default()
                        .fg(self.theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    tr!(self.lang, "退出", "exit"),
                    Style::default().fg(self.theme.text_dim),
                ),
            ]);
            let hint_area = Rect {
                x: 0,
                y: size.height.saturating_sub(1),
                width: size.width,
                height: 1,
            };
            f.render_widget(
                Paragraph::new(hint_line).alignment(Alignment::Center),
                hint_area,
            );

            self.render_input_overlay(f, size);

            if self.popup.is_some() {
                self.render_popups(f, size);
            }
            return;
        }

        let main_area = self.render_banners(f, size);

        self.list_state.select(Some(self.selected));
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(main_area);

        // 三栏：引导栏 | 列表 | 详情
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(22),
                Constraint::Percentage(46),
                Constraint::Percentage(32),
            ])
            .split(chunks[0]);

        self.render_guide(f, body[0]);
        self.render_list(f, body[1]);
        self.render_detail(f, body[2]);

        self.render_status_bar(f, chunks[1]);

        if self.show_syntax && !self.mode.is_input() {
            let syntax_area = self.centered_rect(76, 30, size);
            self.hit_map.modal();
            self.render_syntax_drawer(f, syntax_area);
        }

        match self.mode {
            Mode::ConfirmArchive | Mode::ConfirmPurge | Mode::ConfirmProfileDelete => {
                self.hit_map.modal();
                self.render_confirm_overlay(f, size);
            }
            Mode::Normal | Mode::Visual | Mode::ChecklistFocus => {}
            _ => {
                self.hit_map.modal();
                self.render_input_overlay(f, size);
            }
        }

        if self.show_help {
            let help_area = self.centered_rect(72, size.height.saturating_sub(4), size);
            self.hit_map.modal();
            self.render_help_drawer(f, help_area);
        }

        if self.popup.is_some() {
            // 最高弹窗接管整屏，清除低层弹窗的命中目标。
            self.hit_map.modal();
            self.render_popups(f, size);
        }
    }

    fn render_focus_mode(&mut self, f: &mut Frame, area: Rect) {
        self.render_focus_view(f, area);
    }

    fn render_lock_screen(&mut self, f: &mut Frame, size: Rect) {
        f.render_widget(ratatui::widgets::Clear, size);
        let bg_block = Block::default().style(Style::default().bg(self.theme.bg));
        f.render_widget(bg_block, size);

        let area = self.centered_rect(54, 13, size);
        f.render_widget(ratatui::widgets::Clear, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(self.theme.accent))
            .padding(ratatui::widgets::Padding::horizontal(2))
            .title(tr!(
                self.lang,
                " 🔒 密码保护模式 · 已锁定 ",
                " 🔒 Password Protection · Locked "
            ));

        let masked_input: String = "● ".repeat(self.lock_input.chars().count());
        let is_empty = masked_input.is_empty();
        let mut lines = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                tr!(
                    self.lang,
                    "请输入密码以解锁 horae：",
                    "Please enter password to unlock horae:"
                ),
                Style::default()
                    .fg(self.theme.fg)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled(" [ ", Style::default().fg(self.theme.accent)),
                Span::styled(
                    if is_empty {
                        tr!(self.lang, "输入密码...", "enter password...").to_string()
                    } else {
                        masked_input
                    },
                    if is_empty {
                        Style::default().fg(self.theme.text_dim)
                    } else {
                        Style::default()
                            .fg(self.theme.fg)
                            .add_modifier(Modifier::BOLD)
                    },
                ),
                Span::styled(" ]", Style::default().fg(self.theme.accent)),
            ]),
            Line::from(""),
        ];

        if let Some(ref err) = self.lock_error {
            lines.push(Line::from(vec![Span::styled(
                format!(" ✗ {}", err),
                Style::default().fg(self.theme.text_urgent),
            )]));
        } else {
            lines.push(Line::from(""));
        }

        lines.push(Line::from(vec![
            Span::styled(
                " [Enter] ",
                Style::default()
                    .fg(self.theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                tr!(self.lang, "解锁", "Unlock"),
                Style::default().fg(self.theme.text_dim),
            ),
            Span::styled(
                "  •  [Esc] ",
                Style::default()
                    .fg(self.theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                tr!(self.lang, "退出", "Exit"),
                Style::default().fg(self.theme.text_dim),
            ),
            Span::styled(
                "  •  [Ctrl+U] ",
                Style::default()
                    .fg(self.theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                tr!(self.lang, "清空", "Clear"),
                Style::default().fg(self.theme.text_dim),
            ),
        ]));

        let p = Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Center);
        f.render_widget(p, area);
    }

    fn render_help_drawer(&mut self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        f.render_widget(ratatui::widgets::Clear, area);
        let keys_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .padding(ratatui::widgets::Padding::horizontal(1))
            .border_style(Style::default().fg(self.theme.accent))
            .title(tr!(
                self.lang,
                " 快捷键指南 (F1/?) · hjkl 滚动 · Esc 关闭 ",
                " Shortcuts (F1/?) · hjkl scroll · Esc close "
            ));

        let ctx = ctx_of(self);
        let rows = help_rows(&ctx, self.lang);
        let mut lines: Vec<Line> = Vec::new();
        let mut last_group: Option<KeyGroup> = None;
        for (g, k, desc, applicable) in rows {
            if last_group != Some(g) {
                lines.push(Line::from(Span::styled(
                    g.title(self.lang),
                    Style::default()
                        .fg(self.theme.text_dim)
                        .add_modifier(Modifier::BOLD),
                )));
                last_group = Some(g);
            }
            if applicable {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("{:>6} ", k),
                        Style::default()
                            .fg(self.theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(desc),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("{:>6} ", k),
                        Style::default().fg(self.theme.text_dim),
                    ),
                    Span::styled(desc, Style::default().fg(self.theme.text_dim)),
                ]));
            }
        }

        let content_h = area.height.saturating_sub(2) as usize;
        let max = lines.len().saturating_sub(content_h);
        let scroll = self.help_scroll.min(max);
        self.help_scroll = scroll;
        f.render_widget(
            Paragraph::new(lines)
                .scroll((scroll as u16, 0))
                .block(keys_block),
            area,
        );
        // 帮助浮层是模态层：整屏点击不再落到下面三栏；顶部标题行可点击关闭（等价 Esc）。
        self.hit_map.push(
            Rect::new(area.x, area.y, area.width, 1),
            HitTarget::CloseHelp,
        );
        self.hit_map.push_scroll(area, ScrollArea::Help, max);
    }

    fn render_syntax_drawer(&mut self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        f.render_widget(ratatui::widgets::Clear, area);
        let scroll_hint = if self.syntax_scroll > 0 {
            tr!(self.lang, " · PgUp/PgDn 翻卷", " · PgUp/PgDn")
        } else {
            ""
        };
        let title_str = format!(
            "{}{}",
            tr!(
                self.lang,
                " 语法说明指南 (Ctrl+P 收起) ",
                " Syntax guide (Ctrl+P to close) "
            ),
            scroll_hint
        );
        let syntax_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .padding(ratatui::widgets::Padding::horizontal(1))
            .border_style(Style::default().fg(self.theme.accent))
            .title(title_str);
        let syntax = self.syntax_lines();
        let content_width = syntax_block.inner(area).width;
        let para = Paragraph::new(syntax)
            .block(syntax_block)
            .wrap(ratatui::widgets::Wrap { trim: false });
        // 用段落自身的换行结果算最大偏移，边界与渲染真实内容一致。
        let max = para
            .line_count(content_width)
            .saturating_sub(area.height as usize);
        let scroll = self.syntax_scroll.min(max);
        self.syntax_scroll = scroll;
        f.render_widget(para.scroll((scroll as u16, 0)), area);
        self.hit_map.push_scroll(area, ScrollArea::Syntax, max);
        self.hit_map.push(
            Rect::new(area.x, area.y, area.width, 1),
            HitTarget::CloseSyntax,
        );
    }

    fn centered_rect(&self, percent_x: u16, height: u16, r: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(r.height.saturating_sub(height) / 2),
                Constraint::Length(height),
                Constraint::Min(0),
            ])
            .split(r);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }

    fn render_guide(&mut self, f: &mut ratatui::Frame, area: Rect) {
        // 引导栏内容与可点击的视图行在同一次生成中产出（不解析文本）。
        let content = self.guide_content(area);
        let border_color = if self.pane == Pane::Left {
            self.theme.accent
        } else {
            self.theme.text_dim
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .padding(ratatui::widgets::Padding::horizontal(1))
            .border_style(Style::default().fg(border_color))
            .title(tr!(self.lang, " 引导 ", " Guide "));
        let inner = block.inner(area);
        let max = content.lines.len().saturating_sub(inner.height as usize);
        let scroll = self.guide_scroll.min(max);
        self.guide_scroll = scroll;
        f.render_widget(
            Paragraph::new(content.lines)
                .block(block)
                .scroll((scroll as u16, 0)),
            area,
        );

        // 整栏点击聚焦左侧面板；视图行覆盖在它之上（后登记优先命中）。
        self.hit_map.push(area, HitTarget::Pane(Pane::Left));
        self.hit_map.push_scroll(area, ScrollArea::Guide, max);
        for (idx, view) in content.view_rows {
            if idx < scroll {
                continue;
            }
            let row = idx - scroll;
            if row >= inner.height as usize {
                break;
            }
            let y = inner.y + row as u16;
            self.hit_map
                .push(Rect::new(inner.x, y, inner.width, 1), HitTarget::View(view));
        }
    }

    fn render_list(&mut self, f: &mut ratatui::Frame, area: Rect) {
        if self.view == View::Workflow {
            self.render_workflow(f, area);
            return;
        }
        let border_color = if self.pane == Pane::Center {
            self.theme.accent
        } else {
            self.theme.text_dim
        };
        let lunar_suffix = if self.view == View::Today && self.lunar_enabled {
            if let Some(ref cal) = self.calendar_info {
                format!(" [{}]", cal.status_line())
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        let mut filter_suffix = String::new();
        if let Some(ref tf) = self.tag_filter {
            filter_suffix.push_str(&format!(" [@{}]", tf));
        }
        if !self.search_query.is_empty() {
            filter_suffix.push_str(&format!(" [/{}]", self.search_query));
        }
        let title = tr!(
            self.lang,
            " 任务 · {}{}{} ",
            " Tasks · {}{}{} ",
            super::view_label(self.lang, self.view),
            lunar_suffix,
            filter_suffix
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .padding(ratatui::widgets::Padding::horizontal(1))
            .border_style(Style::default().fg(border_color))
            .title(title);
        // List 的实际内容区（已扣除边框与左右内边距），命中矩形以此为准。
        let inner = block.inner(area);
        // 元数据与实际显示行使用相同的高度，超高任务只显示可点击的预览。
        let rows = build_list_items(self, inner.height as usize);
        let heights: Vec<u16> = rows.iter().map(|r| r.item.height() as u16).collect();
        let meta: Vec<(String, u16, u16)> = rows
            .iter()
            .map(|r| (r.task_id.clone(), r.marker_start, r.marker_width))
            .collect();
        let items: Vec<ListItem<'static>> = rows.into_iter().map(|r| r.item).collect();
        let list = List::new(items)
            .block(block)
            .highlight_style(if self.theme.is_dark {
                // 暗色主题：活动行深蓝背景（与标签亮蓝区分），不覆盖任务文本原色。
                if self.pane == Pane::Center {
                    Style::default().bg(self.theme.row_active_bg)
                } else {
                    Style::default().bg(self.theme.hl_bg)
                }
            } else if self.pane == Pane::Center {
                Style::default()
                    .bg(self.theme.hl_bg)
                    .fg(self.theme.hl_fg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().bg(self.theme.hl_bg)
            });
        f.render_stateful_widget(list, area, &mut self.list_state);

        // 渲染后 `ListState` 已写入当帧真实的可见起始行：直接用它做偏移，
        // 与屏幕上看到的行严格一致（含选中行自动滚动、多行行高）。
        let offset = self.list_state.offset();
        self.hit_map.push(area, HitTarget::Pane(Pane::Center));
        self.hit_map.push_scroll(area, ScrollArea::List, 0);
        let completable = self.view.mouse_completable();
        let mut y = inner.y;
        for (i, (id, marker_start, marker_width)) in meta.iter().enumerate().skip(offset) {
            if y >= inner.y.saturating_add(inner.height) {
                break;
            }
            let full_height = heights.get(i).copied().unwrap_or(1).max(1);
            // Ratatui List only paints complete items that fit the viewport.
            // Do not register the unpainted tail as a partially visible task.
            if full_height > inner.y.saturating_add(inner.height) - y {
                break;
            }
            let height = full_height;
            let row_rect = Rect::new(inner.x, y, inner.width, height);
            self.hit_map.push(row_rect, HitTarget::Task(id.clone()));
            if completable {
                let marker_w = (*marker_width).min(row_rect.width.saturating_sub(*marker_start));
                if marker_w > 0 {
                    self.hit_map.push(
                        Rect::new(
                            row_rect.x.saturating_add(*marker_start),
                            row_rect.y,
                            marker_w,
                            1,
                        ),
                        HitTarget::Complete(id.clone()),
                    );
                }
            }
            y = y.saturating_add(full_height);
        }

        if self.items.is_empty() && area.height >= 4 && area.width >= 8 {
            let empty_hint = if self.tag_filter.is_some() || !self.search_query.is_empty() {
                tr!(
                    self.lang,
                    "没有匹配过滤条件的任务 · 按 Esc 清除过滤",
                    "No tasks match filter · Press Esc to clear"
                )
            } else {
                match self.view {
                    View::Inbox => tr!(
                        self.lang,
                        "收件箱为空 · 按 a 捕获新想法",
                        "Inbox is empty · Press a to capture"
                    ),
                    View::Archived => tr!(self.lang, "归档箱为空", "Archive is empty"),
                    View::Quotes => tr!(
                        self.lang,
                        "暂无金句 · 录入时加 @quote 或在任务上按 \"",
                        "No quotes yet · Add @quote or press \" on a task"
                    ),
                    _ => tr!(self.lang, "此处暂无任务", "No tasks here"),
                }
            };
            let hint_rect = Rect {
                x: area.x + 2,
                y: area.y + area.height / 2,
                width: area.width.saturating_sub(4),
                height: 1,
            };
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    empty_hint,
                    Style::default().fg(self.theme.text_dim),
                )))
                .alignment(Alignment::Center),
                hint_rect,
            );
        }
    }

    /// GTD 工作流与决策树视图（中心面板）。
    fn render_workflow(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let border_color = if self.pane == Pane::Center {
            self.theme.accent
        } else {
            self.theme.text_dim
        };
        let title = tr!(
            self.lang,
            " GTD 工作流 · 决策树与五步闭环 ",
            " GTD Workflow · Decision Tree & 5 Steps "
        );
        let lines = self.workflow_lines();
        let content_h = area.height.saturating_sub(2) as usize;
        let top_pad = if content_h > lines.len() {
            ((content_h - lines.len()) / 3).min(3) as u16
        } else {
            0
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .padding(ratatui::widgets::Padding::new(1, 1, top_pad, 0))
            .border_style(Style::default().fg(border_color))
            .title(title);
        let content_width = block.inner(area).width;
        let para = Paragraph::new(lines)
            .block(block)
            .wrap(ratatui::widgets::Wrap { trim: false });
        let max = para
            .line_count(content_width)
            .saturating_sub(area.height as usize);
        let scroll = self.workflow_scroll.min(max);
        self.workflow_scroll = scroll;
        f.render_widget(para.scroll((scroll as u16, 0)), area);
        self.hit_map.push(area, HitTarget::Pane(Pane::Center));
        self.hit_map.push_scroll(area, ScrollArea::Workflow, max);
    }

    /// GTD 决策树与五步闭环说明文本（中英双语，精炼版）。
    fn workflow_lines(&self) -> Vec<Line<'static>> {
        use Icon::*;
        let s_step = Style::default()
            .fg(self.theme.accent)
            .add_modifier(Modifier::BOLD);
        let s_dim = Style::default().fg(self.theme.text_dim);
        let s_map = Style::default().fg(self.theme.hl_fg);
        let s_urgent = Style::default()
            .fg(self.theme.text_urgent)
            .add_modifier(Modifier::BOLD);
        let s_success = Style::default()
            .fg(self.theme.text_success)
            .add_modifier(Modifier::BOLD);
        let s_bold = Style::default().add_modifier(Modifier::BOLD);

        let step = |s: &'static str| Span::styled(s.to_string(), s_step);
        let dim = |s: &'static str| Span::styled(s.to_string(), s_dim);
        let map = |s: &'static str| Span::styled(s.to_string(), s_map);
        let urgent = |s: &'static str| Span::styled(s.to_string(), s_urgent);

        let mut lines: Vec<Line> = Vec::new();

        // 1. 收集
        lines.push(Line::from(vec![
            Span::styled(format!("{} ", self.icon(Inbox)), s_step),
            step(tr!(self.lang, "1. 收集 (Capture)", "1. Capture")),
        ]));
        lines.push(Line::from(vec![
            dim("   "),
            Span::styled(
                tr!(
                    self.lang,
                    "杂事 100% 入收件箱 (",
                    "Capture 100% stuff into Inbox ("
                ),
                s_dim,
            ),
            map(tr!(self.lang, "1 / a", "1 / a")),
            Span::styled(
                tr!(self.lang, ")，清空大脑，不留杂念。", "). Free your mind."),
                s_dim,
            ),
        ]));
        lines.push(Line::from(""));

        // 2. 厘清与决策树
        lines.push(Line::from(vec![
            Span::styled(format!("{} ", self.icon(Next)), s_step),
            step(tr!(self.lang, "2. 厘清 (Clarify)", "2. Clarify")),
            dim("  ──  "),
            Span::styled(
                tr!(self.lang, "逐条判定：是否可行动？", "Is it actionable?"),
                s_bold,
            ),
        ]));
        // 不可行动
        lines.push(Line::from(vec![
            dim(tr!(self.lang, "   ├── 否 (No)  ──→ ", "   ├── No → ")),
            urgent(tr!(self.lang, "🗑 归档", "🗑 Archive")),
            dim(tr!(self.lang, " (8/A)  ·  ", " (8/A)  ·  ")),
            map(tr!(self.lang, "💡 将来也许", "💡 Someday")),
            dim(tr!(self.lang, " (5/s)  ·  ", " (5/s)  ·  ")),
            map(tr!(self.lang, "📚 参考资料", "📚 Reference")),
            dim(tr!(self.lang, " (6/0)", " (6/0)")),
        ]));
        // 可行动
        lines.push(Line::from(vec![
            dim(tr!(self.lang, "   └── 是 (Yes) ──→ ", "   └── Yes → ")),
            Span::styled(
                tr!(
                    self.lang,
                    "多步骤立项目 (Shift+C 检查单)",
                    "Multi-step? Project + Checklist (Shift+C)"
                ),
                s_success,
            ),
        ]));
        lines.push(Line::from(vec![
            dim(tr!(
                self.lang,
                "       ├── 耗时 < 2分钟 → ⚡ ",
                "       ├── < 2 min → ⚡ "
            )),
            urgent(tr!(
                self.lang,
                "立即做完 (两分钟原则)",
                "Do it now (2-Minute Rule)"
            )),
        ]));
        lines.push(Line::from(vec![
            dim(tr!(
                self.lang,
                "       ├── 委派他人 → 👥 ",
                "       ├── Delegate → 👥 "
            )),
            map(tr!(self.lang, "等待中 (3/w)", "Waiting For (3/w)")),
        ]));
        lines.push(Line::from(vec![dim(tr!(
            self.lang,
            "       └── 延迟/我来做 (Defer/Do):",
            "       └── Defer/Do:"
        ))]));
        lines.push(Line::from(vec![
            dim(tr!(
                self.lang,
                "           ├── 固定时间 → 📅 ",
                "           ├── Specific time → 📅 "
            )),
            map(tr!(
                self.lang,
                "已排程 (4/~/* 习惯循环)",
                "Scheduled (4/~/* Habits)"
            )),
        ]));
        lines.push(Line::from(vec![
            dim(tr!(
                self.lang,
                "           └── 尽快执行 → 🎯 ",
                "           └── ASAP → 🎯 "
            )),
            map(tr!(
                self.lang,
                "下一步行动 (2/Enter)",
                "Next Actions (2/Enter)"
            )),
        ]));
        lines.push(Line::from(""));

        // 3. 组织
        lines.push(Line::from(vec![
            Span::styled(format!("{} ", self.icon(Scheduled)), s_step),
            step(tr!(self.lang, "3. 组织 (Organize)", "3. Organize")),
        ]));
        lines.push(Line::from(vec![
            dim("   "),
            Span::styled(
                tr!(
                    self.lang,
                    "情境标签 (@work/@focus) 与 任务优先级 (!high/!medium/!low 或 !1/!2/!3)。",
                    "Context tags (@work/@focus) & task priority (!high/!medium/!low or !1/!2/!3)."
                ),
                s_dim,
            ),
        ]));
        lines.push(Line::from(""));

        // 4. 回顾
        lines.push(Line::from(vec![
            Span::styled(format!("{} ", self.icon(Review)), s_step),
            step(tr!(self.lang, "4. 回顾 (Reflect)", "4. Reflect")),
        ]));
        lines.push(Line::from(vec![
            dim("   "),
            Span::styled(
                tr!(
                    self.lang,
                    "晨间 horae do 智能聚焦  ·  晚间清零 (Inbox Zero)  ·  周末按 r 周回顾。",
                    "Morning horae do  ·  Evening Inbox Zero  ·  Weekly review (r)."
                ),
                s_dim,
            ),
        ]));
        lines.push(Line::from(""));

        // 5. 执行
        lines.push(Line::from(vec![
            Span::styled(format!("{} ", self.icon(Done)), s_step),
            step(tr!(self.lang, "5. 执行 (Engage)", "5. Engage")),
        ]));
        lines.push(Line::from(vec![
            dim("   "),
            Span::styled(
                tr!(
                    self.lang,
                    "四维选事法则 (情境 / 时间 / 精力 / 优先级)  ·  按 P 开启番茄专注钟。",
                    "Pick actions by 4-criteria (Context/Time/Energy/Priority)  ·  P for Pomodoro."
                ),
                s_dim,
            ),
        ]));
        lines.push(Line::from(""));

        lines.push(Line::from(Span::styled(
            tr!(
                self.lang,
                "💡 提示：按 h/l 切换面板，按 j/k 滚动，按 g/G 置顶/置底。",
                "Tip: press h/l to switch panels, j/k to scroll, g/G for top/bottom."
            ),
            s_dim,
        )));

        lines
    }

    /// GTD 哲学与 David Allen 介绍（右侧面板）。
    fn render_workflow_side(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let border_color = if self.pane == Pane::Right {
            self.theme.accent
        } else {
            self.theme.text_dim
        };
        let title = tr!(
            self.lang,
            " David Allen · GTD 核心哲学 ",
            " David Allen · GTD Philosophy "
        );
        let lines = self.workflow_side_lines();
        let content_h = area.height.saturating_sub(2) as usize;
        let top_pad = if content_h > lines.len() {
            ((content_h - lines.len()) / 3).min(3) as u16
        } else {
            0
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .padding(ratatui::widgets::Padding::new(1, 1, top_pad, 0))
            .border_style(Style::default().fg(border_color))
            .title(title);
        let content_width = block.inner(area).width;
        let para = Paragraph::new(lines)
            .block(block)
            .wrap(ratatui::widgets::Wrap { trim: false });
        let max = para
            .line_count(content_width)
            .saturating_sub(area.height as usize);
        let scroll = self.workflow_side_scroll.min(max);
        self.workflow_side_scroll = scroll;
        f.render_widget(para.scroll((scroll as u16, 0)), area);
        self.hit_map.push(area, HitTarget::Pane(Pane::Right));
        self.hit_map
            .push_scroll(area, ScrollArea::WorkflowSide, max);
    }

    /// David Allen 介绍与 GTD 核心哲学文本（中英双语，100 字精炼版）。
    #[allow(clippy::vec_init_then_push)]
    fn workflow_side_lines(&self) -> Vec<Line<'static>> {
        let s_sec = Style::default()
            .fg(self.theme.accent)
            .add_modifier(Modifier::BOLD);
        let s_quote_title = Style::default()
            .fg(self.theme.rrule_fg)
            .add_modifier(Modifier::BOLD);
        let s_quote_text = Style::default()
            .fg(self.theme.text_urgent)
            .add_modifier(Modifier::ITALIC);
        let s_dim = Style::default().fg(self.theme.text_dim);
        let s_bold = Style::default().add_modifier(Modifier::BOLD);

        let mut lines: Vec<Line> = Vec::new();

        // 导师简介
        lines.push(Line::from(Span::styled(
            tr!(self.lang, "◆ 导师简介 (About)", "◆ About David Allen"),
            s_sec,
        )));
        lines.push(Line::from(vec![
            Span::styled(
                tr!(
                    self.lang,
                    "David Allen (1945~)：",
                    "David Allen (born 1945): "
                ),
                s_bold,
            ),
            Span::styled(
                tr!(
                    self.lang,
                    "GTD (Getting Things Done / 尽管去做) 创始人，全球顶尖效能导师，《时代周刊》效率大师。",
                    "Creator of Getting Things Done (GTD) and world-renowned productivity pioneer."
                ),
                s_dim,
            ),
        ]));
        lines.push(Line::from(""));

        // 核心哲学
        lines.push(Line::from(Span::styled(
            tr!(
                self.lang,
                "◆ 核心心法 (Core Principles)",
                "◆ Core Principles"
            ),
            s_sec,
        )));
        lines.push(Line::from(""));

        // 心法 1：大脑是 CPU 不是硬盘
        lines.push(Line::from(vec![
            Span::styled("🧠 ", s_bold),
            Span::styled(
                tr!(
                    self.lang,
                    "大脑是 CPU，不是硬盘",
                    "Mind for ideas, not holding them"
                ),
                s_quote_title,
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("   "),
            Span::styled(
                tr!(
                    self.lang,
                    "“Your mind is for having ideas, not holding them.”",
                    "“Your mind is for having ideas, not holding them.”"
                ),
                s_quote_text,
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("   "),
            Span::styled(
                tr!(
                    self.lang,
                    "将一切杂事 100% 外部化到系统，彻底清空工作记忆与认知负荷。",
                    "Externalize all loops to system to free mental bandwidth and stress."
                ),
                s_dim,
            ),
        ]));
        lines.push(Line::from(""));

        // 心法 2：心如止水
        lines.push(Line::from(vec![
            Span::styled("🌊 ", s_bold),
            Span::styled(
                tr!(self.lang, "心如止水 (Mind Like Water)", "Mind Like Water"),
                s_quote_title,
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("   "),
            Span::styled(
                tr!(
                    self.lang,
                    "对纷至沓来的事务恰如其分地反应，事毕即复归平静与深度专注。",
                    "Respond appropriately to input, then return to calm and deep focus."
                ),
                s_dim,
            ),
        ]));
        lines.push(Line::from(""));

        // 心法 3：两分钟原则
        lines.push(Line::from(vec![
            Span::styled("⚡ ", s_bold),
            Span::styled(
                tr!(self.lang, "两分钟原则 (2-Minute Rule)", "2-Minute Rule"),
                s_quote_title,
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("   "),
            Span::styled(
                tr!(
                    self.lang,
                    "耗时 < 2 分钟的事立即动手做完，不留任何推迟、排程与记录开销。",
                    "If an action takes less than 2 minutes, do it immediately."
                ),
                s_dim,
            ),
        ]));
        lines.push(Line::from(""));

        // 心法 4：聚焦物理下一步
        lines.push(Line::from(vec![
            Span::styled("🎯 ", s_bold),
            Span::styled(
                tr!(
                    self.lang,
                    "明确物理下一步 (Next Physical Action)",
                    "Next Physical Action"
                ),
                s_quote_title,
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("   "),
            Span::styled(
                tr!(
                    self.lang,
                    "拖延往往源于步骤模糊。将复杂任务拆解为无需思考即可执行的第一步。",
                    "Clarify the immediate next visible physical activity to eliminate friction."
                ),
                s_dim,
            ),
        ]));
        lines.push(Line::from(""));

        // 心法 5：多维高度俯瞰
        lines.push(Line::from(vec![
            Span::styled("🔭 ", s_bold),
            Span::styled(
                tr!(
                    self.lang,
                    "多维专注高度 (Horizons of Focus)",
                    "Horizons of Focus"
                ),
                s_quote_title,
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("   "),
            Span::styled(
                tr!(
                    self.lang,
                    "5万英尺(愿景与人生目标) → 跑道(日常微行动)，自上而下对齐，自下而上掌控。",
                    "50,000 ft (Vision) to Runway (Actions) — align top-down, control bottom-up."
                ),
                s_dim,
            ),
        ]));

        lines
    }

    fn render_detail(&mut self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        if self.view == View::Workflow {
            self.render_workflow_side(f, area);
            return;
        }
        let border_color = if self.pane == Pane::Right {
            self.theme.accent
        } else {
            self.theme.text_dim
        };

        if self.view == View::Settings {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_set(border::ROUNDED)
                .padding(ratatui::widgets::Padding::horizontal(1))
                .border_style(Style::default().fg(border_color))
                .title(tr!(self.lang, " 设置与安全 ", " Settings & Security "));

            let selected_profile = self.items.get(self.selected).map(|r| r.title.clone());
            let mut lines = vec![Line::from(vec![
                Span::styled(
                    tr!(self.lang, "当前配置: ", "Current Profile: "),
                    Style::default()
                        .fg(self.theme.text_dim)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    self.profile_name.clone(),
                    Style::default()
                        .fg(self.theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
            ])];

            if let Some(ref prof) = selected_profile {
                if prof != &self.profile_name {
                    lines.push(Line::from(vec![
                        Span::styled(
                            tr!(self.lang, "选中配置: ", "Selected Profile: "),
                            Style::default().fg(self.theme.text_dim),
                        ),
                        Span::styled(prof.clone(), Style::default().fg(self.theme.fg)),
                    ]));
                }
            }

            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                tr!(self.lang, "系统偏好设置", "System Preferences"),
                Style::default()
                    .fg(self.theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]));
            lines.push(Line::from(vec![
                Span::styled(
                    tr!(self.lang, "• 提示音效: ", "• Sound Effects: "),
                    Style::default().fg(self.theme.text_dim),
                ),
                if self.sound_enabled {
                    Span::styled(
                        tr!(self.lang, "已开启 (默认)", "Enabled (Default)"),
                        Style::default()
                            .fg(self.theme.text_success)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::styled(
                        tr!(self.lang, "已关闭 (静音)", "Disabled (Muted)"),
                        Style::default().fg(self.theme.text_dim),
                    )
                },
            ]));

            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                tr!(self.lang, "数据安全保护", "Data Security"),
                Style::default()
                    .fg(self.theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]));
            lines.push(Line::from(vec![
                Span::styled(
                    tr!(self.lang, "• 密码保护模式: ", "• Password Mode: "),
                    Style::default().fg(self.theme.text_dim),
                ),
                if self.password_enabled {
                    Span::styled(
                        tr!(self.lang, "已启用 (受保护)", "Enabled (Protected)"),
                        Style::default()
                            .fg(self.theme.text_success)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::styled(
                        tr!(self.lang, "未启用 (默认)", "Disabled (Default)"),
                        Style::default().fg(self.theme.text_dim),
                    )
                },
            ]));

            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                tr!(self.lang, "快捷操作：", "Actions:"),
                Style::default()
                    .fg(self.theme.text_dim)
                    .add_modifier(Modifier::BOLD),
            )]));
            lines.push(Line::from(vec![
                Span::styled(" [b] ", Style::default().fg(self.theme.accent)),
                Span::styled(
                    if self.sound_enabled {
                        tr!(self.lang, "关闭音效提示", "Disable Sound Effects")
                    } else {
                        tr!(self.lang, "开启音效提示", "Enable Sound Effects")
                    },
                    Style::default().fg(self.theme.fg),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled(" [p] ", Style::default().fg(self.theme.accent)),
                Span::styled(
                    if self.password_enabled {
                        tr!(self.lang, "关闭/修改密码", "Disable/Change Password")
                    } else {
                        tr!(self.lang, "开启密码保护模式", "Enable Password Mode")
                    },
                    Style::default().fg(self.theme.fg),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled(" [n] ", Style::default().fg(self.theme.accent)),
                Span::styled(
                    tr!(self.lang, "新建配置", "New Profile"),
                    Style::default().fg(self.theme.fg),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled(" [r] ", Style::default().fg(self.theme.accent)),
                Span::styled(
                    tr!(self.lang, "重命名配置", "Rename Profile"),
                    Style::default().fg(self.theme.fg),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled(" [d] ", Style::default().fg(self.theme.accent)),
                Span::styled(
                    tr!(self.lang, "删除配置", "Delete Profile"),
                    Style::default().fg(self.theme.fg),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled(" [s] ", Style::default().fg(self.theme.accent)),
                Span::styled(
                    tr!(self.lang, "设为默认配置", "Set as Default"),
                    Style::default().fg(self.theme.fg),
                ),
            ]));

            let para = Paragraph::new(lines).block(block);
            let max = para
                .line_count(area.width)
                .saturating_sub(area.height as usize);
            self.detail_scroll = self.detail_scroll.min(max);
            f.render_widget(para.scroll((self.detail_scroll as u16, 0)), area);
            self.hit_map.push(area, HitTarget::Pane(Pane::Right));
            self.hit_map.push_scroll(area, ScrollArea::Detail, max);
            return;
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .padding(ratatui::widgets::Padding::horizontal(1))
            .border_style(Style::default().fg(border_color))
            .title(tr!(self.lang, " 任务详情 ", " Task Details "));

        // 先取出渲染行（结束对 `self.detail` 的不可变借用），再更新滚动偏移。
        let lines = self
            .detail
            .as_ref()
            .map(|d| self.detail_lines(d, area.width, area.height));
        let Some(lines) = lines else {
            let empty_para = Paragraph::new(tr!(self.lang, " 未选中任务", " No task selected"))
                .style(Style::default().fg(self.theme.text_dim))
                .block(block);
            f.render_widget(empty_para, area);
            self.hit_map.push(area, HitTarget::Pane(Pane::Right));
            return;
        };

        let content_width = block.inner(area).width;
        let para = Paragraph::new(lines)
            .style(Style::default().fg(self.theme.fg))
            .block(block)
            .wrap(ratatui::widgets::Wrap { trim: false });
        // 用段落真实换行结果算最大偏移，避免滚过头出现空白页。
        let max = para
            .line_count(content_width)
            .saturating_sub(area.height as usize);
        let scroll = self.detail_scroll.min(max);
        self.detail_scroll = scroll;
        f.render_widget(para.scroll((scroll as u16, 0)), area);
        self.hit_map.push(area, HitTarget::Pane(Pane::Right));
        self.hit_map.push_scroll(area, ScrollArea::Detail, max);
    }
}

mod banners;
mod detail;
pub(crate) mod focus;
mod help;
mod input;
mod popups;
