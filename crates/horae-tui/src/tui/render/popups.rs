use super::AppRender;
use crate::tui::app::{App, Mode};
use crate::tui::handlers::{MODULE_OPTION_COUNT, MODULE_OPTION_SCROLL_STEP};
use crate::tui::mouse::{HitTarget, ScrollArea};
use crossterm::event::KeyCode;
use ratatui::symbols::border;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

impl<'a> App<'a> {
    /// 组装一行含可点击片段的文本，并把每个片段的真实矩形登记进 `hit_map`。
    ///
    /// `inner` 是弹窗内容区（已扣除边框与内边距），`row` 是该行在内容区内的
    /// 行号（0 起），`centered` 必须与 Paragraph 的对齐方式一致。中文宽字符
    /// 的列宽统一用 `Span::width()` 计算，不手数字符，避免热区错位。
    fn popup_click_line(
        &mut self,
        inner: Rect,
        row: usize,
        segments: Vec<(Vec<Span<'static>>, Option<KeyCode>)>,
        centered: bool,
    ) -> Line<'static> {
        let widths: Vec<usize> = segments
            .iter()
            .map(|(spans, _)| spans.iter().map(Span::width).sum::<usize>())
            .collect();
        let line_width: usize = widths.iter().sum();
        let pad = (inner.width as usize).saturating_sub(line_width) / 2;
        let x0 = if centered {
            inner.x.saturating_add(pad as u16)
        } else {
            inner.x
        };

        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut col = 0usize;
        for ((seg_spans, target), seg_w) in segments.into_iter().zip(widths) {
            if let Some(code) = target {
                let visible_w = seg_w.min((inner.width as usize).saturating_sub(col));
                if visible_w > 0 && row < inner.height as usize {
                    self.hit_map.push(
                        Rect {
                            x: x0.saturating_add(col as u16),
                            y: inner.y + row as u16,
                            width: visible_w as u16,
                            height: 1,
                        },
                        HitTarget::Key(code),
                    );
                }
            }
            col += seg_w;
            spans.extend(seg_spans);
        }
        Line::from(spans)
    }

    /// 批量操作确认弹层：归档 / 永久删除的居中醒目确认框。
    pub(super) fn render_confirm_overlay(&mut self, f: &mut Frame, size: Rect) {
        // 确认弹层是模态的：先清空底层命中目标，只保留本弹层的按钮热区。
        self.hit_map.modal();

        let (title, desc) = match self.mode {
            Mode::ConfirmPurge => (
                tr!(
                    self.lang,
                    " ⚠ 确认永久删除 ",
                    " ⚠ Confirm permanent delete "
                ),
                tr!(
                    self.lang,
                    "将永久删除 {} 项，不可恢复。",
                    "Permanently delete {} item(s). This cannot be undone.",
                    self.pending_purge_ids.len()
                ),
            ),
            Mode::ConfirmProfileDelete => (
                tr!(
                    self.lang,
                    " ⚠ 确认删除 profile ",
                    " ⚠ Confirm delete profile "
                ),
                tr!(
                    self.lang,
                    "从配置移除 profile `{}`（数据库文件保留）。",
                    "Remove profile `{}` from config (db file kept).",
                    self.pending_profile_delete.as_deref().unwrap_or("")
                ),
            ),
            _ => (
                tr!(self.lang, " ⚠ 确认归档 ", " ⚠ Confirm archive "),
                tr!(
                    self.lang,
                    "将归档 {} 项。",
                    "Archive {} item(s).",
                    self.pending_archive_ids.len()
                ),
            ),
        };

        let mut lines = vec![
            Line::from(Span::styled(
                format!(" {}", desc),
                Style::default().fg(self.theme.fg),
            )),
            Line::from(""),
        ];
        let height = (lines.len() + 1) as u16 + 2;
        let area = self.centered_rect(58, height, size);

        f.render_widget(ratatui::widgets::Clear, area);
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .padding(ratatui::widgets::Padding::horizontal(1))
            .border_style(
                Style::default()
                    .fg(self.theme.text_urgent)
                    .add_modifier(Modifier::BOLD),
            );
        let inner = block.inner(area);

        let key_style = Style::default()
            .fg(self.theme.text_success)
            .add_modifier(Modifier::BOLD);
        let cancel_style = Style::default()
            .fg(self.theme.text_urgent)
            .add_modifier(Modifier::BOLD);
        let dim = Style::default().fg(self.theme.text_dim);
        let buttons = self.popup_click_line(
            inner,
            lines.len(),
            vec![
                (
                    vec![
                        Span::styled(tr!(self.lang, "  [y/Enter] ", "  [y/Enter] "), key_style),
                        Span::styled(
                            tr!(self.lang, "确认  ", "Confirm  "),
                            Style::default().fg(self.theme.fg),
                        ),
                    ],
                    Some(KeyCode::Enter),
                ),
                (
                    vec![
                        Span::styled(tr!(self.lang, "[n/Esc] ", "[n/Esc] "), cancel_style),
                        Span::styled(tr!(self.lang, "取消", "Cancel"), dim),
                    ],
                    Some(KeyCode::Esc),
                ),
            ],
            false,
        );
        lines.push(buttons);

        f.render_widget(Paragraph::new(lines).block(block), area);
    }

    /// 弹出框：模块开关 / 任务到期提醒 / 密码设置。
    pub(super) fn render_popups(&mut self, f: &mut Frame, size: Rect) {
        let Some(popup) = self.popup.clone() else {
            return;
        };
        // 弹窗模态：清空底层（列表/详情/输入层）的命中目标，
        // 之后只登记本弹窗内真实渲染出来的区域。
        self.hit_map.modal();
        match popup {
            crate::tui::app::Popup::ModuleToggles(idx) => {
                self.render_module_toggles(f, size, idx);
            }
            crate::tui::app::Popup::TaskDueNow(_, title) => {
                let area = self.centered_rect(50, 10, size);
                f.render_widget(ratatui::widgets::Clear, area);
                let block = Block::default()
                    .title(tr!(self.lang, " ⏰ 任务提醒! ", " ⏰ Task due! "))
                    .borders(Borders::ALL)
                    .border_set(border::ROUNDED)
                    .border_style(Style::default().fg(self.theme.text_urgent));
                let inner = block.inner(area);

                let mut lines = vec![
                    Line::from(tr!(
                        self.lang,
                        "有任务到期或已到开始时间，需要立即处理：",
                        "A task is due or starting now, handle it now:"
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        format!(" 「{}」 ", title),
                        Style::default()
                            .fg(self.theme.text_urgent)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                ];

                let key_style = Style::default()
                    .fg(self.theme.accent)
                    .add_modifier(Modifier::BOLD);
                let dim = Style::default().fg(self.theme.text_dim);
                let hint = self.popup_click_line(
                    inner,
                    lines.len(),
                    vec![
                        (
                            vec![
                                Span::styled(" [Enter] ", key_style),
                                Span::styled(
                                    tr!(self.lang, "一键进入番茄钟", "start pomodoro"),
                                    dim,
                                ),
                            ],
                            Some(KeyCode::Enter),
                        ),
                        (vec![Span::styled("  |  ", dim)], None),
                        (
                            vec![
                                Span::styled("[Esc] ", key_style),
                                Span::styled(tr!(self.lang, "忽略", "dismiss"), dim),
                            ],
                            Some(KeyCode::Esc),
                        ),
                    ],
                    true,
                );
                lines.push(hint);

                f.render_widget(
                    Paragraph::new(lines)
                        .block(block)
                        .alignment(Alignment::Center),
                    area,
                );
            }
            crate::tui::app::Popup::PasswordSet {
                is_confirm,
                input,
                error,
                ..
            } => {
                let area = self.centered_rect(54, 11, size);
                f.render_widget(ratatui::widgets::Clear, area);
                let block = Block::default()
                    .title(tr!(
                        self.lang,
                        " 🔐 设置密码保护 ",
                        " 🔐 Set Password Protection "
                    ))
                    .borders(Borders::ALL)
                    .border_set(border::ROUNDED)
                    .border_style(Style::default().fg(self.theme.accent));
                let inner = block.inner(area);

                let prompt = if is_confirm {
                    tr!(
                        self.lang,
                        "请再次输入以确认新密码：",
                        "Please confirm new password:"
                    )
                } else {
                    tr!(self.lang, "请输入新密码：", "Please enter new password:")
                };

                let masked = "● ".repeat(input.chars().count());
                let is_empty = masked.is_empty();
                let mut lines = vec![
                    Line::from(""),
                    Line::from(Span::styled(prompt, Style::default().fg(self.theme.fg))),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(" [ ", Style::default().fg(self.theme.accent)),
                        Span::styled(
                            if is_empty {
                                tr!(self.lang, "输入密码...", "enter password...").to_string()
                            } else {
                                masked
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

                if let Some(err) = error {
                    lines.push(Line::from(Span::styled(
                        format!(" ✗ {}", err),
                        Style::default().fg(self.theme.text_urgent),
                    )));
                } else {
                    lines.push(Line::from(""));
                }

                let hint = self.password_popup_hint(
                    inner,
                    lines.len(),
                    &[KeyCode::Enter, KeyCode::Esc],
                    true,
                );
                lines.push(hint);

                f.render_widget(
                    Paragraph::new(lines)
                        .block(block)
                        .alignment(Alignment::Center),
                    area,
                );
            }
            crate::tui::app::Popup::PasswordDisable { input, error } => {
                let area = self.centered_rect(54, 11, size);
                f.render_widget(ratatui::widgets::Clear, area);
                let block = Block::default()
                    .title(tr!(
                        self.lang,
                        " 🔓 关闭密码保护 ",
                        " 🔓 Disable Password Protection "
                    ))
                    .borders(Borders::ALL)
                    .border_set(border::ROUNDED)
                    .border_style(Style::default().fg(self.theme.accent));
                let inner = block.inner(area);

                let prompt = tr!(
                    self.lang,
                    "请输入当前密码以关闭保护：",
                    "Enter current password to disable:"
                );

                let masked = "● ".repeat(input.chars().count());
                let is_empty = masked.is_empty();
                let mut lines = vec![
                    Line::from(""),
                    Line::from(Span::styled(prompt, Style::default().fg(self.theme.fg))),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(" [ ", Style::default().fg(self.theme.accent)),
                        Span::styled(
                            if is_empty {
                                tr!(self.lang, "输入密码...", "enter password...").to_string()
                            } else {
                                masked
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

                if let Some(err) = error {
                    lines.push(Line::from(Span::styled(
                        format!(" ✗ {}", err),
                        Style::default().fg(self.theme.text_urgent),
                    )));
                } else {
                    lines.push(Line::from(""));
                }

                let hint = self.password_popup_hint(
                    inner,
                    lines.len(),
                    &[KeyCode::Enter, KeyCode::Tab, KeyCode::Esc],
                    true,
                );
                lines.push(hint);

                f.render_widget(
                    Paragraph::new(lines)
                        .block(block)
                        .alignment(Alignment::Center),
                    area,
                );
            }
            crate::tui::app::Popup::PasswordChange {
                step, input, error, ..
            } => {
                let area = self.centered_rect(54, 11, size);
                f.render_widget(ratatui::widgets::Clear, area);
                let block = Block::default()
                    .title(tr!(self.lang, " 🔑 修改保护密码 ", " 🔑 Change Password "))
                    .borders(Borders::ALL)
                    .border_set(border::ROUNDED)
                    .border_style(Style::default().fg(self.theme.accent));
                let inner = block.inner(area);

                let prompt = match step {
                    0 => tr!(self.lang, "请输入当前原密码：", "Enter current password:"),
                    1 => tr!(self.lang, "请输入新密码：", "Enter new password:"),
                    _ => tr!(
                        self.lang,
                        "请再次输入以确认新密码：",
                        "Confirm new password:"
                    ),
                };

                let masked = "● ".repeat(input.chars().count());
                let is_empty = masked.is_empty();
                let mut lines = vec![
                    Line::from(""),
                    Line::from(Span::styled(prompt, Style::default().fg(self.theme.fg))),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(" [ ", Style::default().fg(self.theme.accent)),
                        Span::styled(
                            if is_empty {
                                tr!(self.lang, "输入密码...", "enter password...").to_string()
                            } else {
                                masked
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

                if let Some(err) = error {
                    lines.push(Line::from(Span::styled(
                        format!(" ✗ {}", err),
                        Style::default().fg(self.theme.text_urgent),
                    )));
                } else {
                    lines.push(Line::from(""));
                }

                let hint = self.password_popup_hint(
                    inner,
                    lines.len(),
                    &[KeyCode::Enter, KeyCode::Esc],
                    true,
                );
                lines.push(hint);

                f.render_widget(
                    Paragraph::new(lines)
                        .block(block)
                        .alignment(Alignment::Center),
                    area,
                );
            }
        }
    }

    /// 密码弹层底部的可点击操作行：按传入的键顺序生成「按键 + 说明」片段，
    /// 每个片段都登记成 `HitTarget::Key`，点击等价于按下该键。
    fn password_popup_hint(
        &mut self,
        inner: Rect,
        row: usize,
        keys: &[KeyCode],
        centered: bool,
    ) -> Line<'static> {
        let key_style = Style::default()
            .fg(self.theme.accent)
            .add_modifier(Modifier::BOLD);
        let dim = Style::default().fg(self.theme.text_dim);

        let mut segments: Vec<(Vec<Span<'static>>, Option<KeyCode>)> = Vec::new();
        for (i, code) in keys.iter().enumerate() {
            if i > 0 {
                segments.push((vec![Span::styled("  •  ", dim)], None));
            }
            let (key_label, action) = match code {
                KeyCode::Enter => {
                    let label = if keys.len() > 2 {
                        tr!(self.lang, "确认关闭", "Confirm")
                    } else {
                        tr!(self.lang, "下一步/确认", "Next/Confirm")
                    };
                    (" [Enter] ".to_string(), label)
                }
                KeyCode::Tab => (" [Tab] ".to_string(), tr!(self.lang, "修改密码", "Change")),
                _ => (" [Esc] ".to_string(), tr!(self.lang, "取消", "Cancel")),
            };
            segments.push((
                vec![
                    Span::styled(key_label, key_style),
                    Span::styled(action, dim),
                ],
                Some(*code),
            ));
        }
        self.popup_click_line(inner, row, segments, centered)
    }

    /// F7 模块显示设置弹层。
    ///
    /// 每个可见选项行都按真实矩形登记 `HitTarget::ModuleOption(i)`；选项多于
    /// 可见高度时按「选中项居中」推导滚动偏移，因此不需要新增 App 状态字段。
    fn render_module_toggles(&mut self, f: &mut Frame, size: Rect, selected: usize) {
        // 宽度下限 40：小终端也要放得下选项文本与底部按钮；高度随窗口收缩。
        let width = ((size.width as u32 * 46 / 100) as u16)
            .clamp(40, 60)
            .min(size.width);
        let height = 22.min(size.height);
        let area = Rect::new(
            size.x + (size.width - width) / 2,
            size.y + (size.height - height) / 2,
            width,
            height,
        );
        f.render_widget(ratatui::widgets::Clear, area);

        let nerd = matches!(self.icon_style, crate::tui::icons::IconStyle::Nerd);
        let icons_label = if nerd {
            tr!(self.lang, "图标 (Nerd Font)", "Icons (Nerd Font)")
        } else {
            tr!(self.lang, "图标 (ASCII 回退)", "Icons (ASCII fallback)")
        };
        let is_ref = matches!(
            self.completion_style,
            crate::tui::app::completion::CompletionStyle::Reference
        );
        let completion_label = self.completion_style.label(self.lang);
        let opts: [(bool, &str); MODULE_OPTION_COUNT] = [
            (
                self.modules.splash,
                tr!(self.lang, "开屏页 (Splash)", "Splash screen"),
            ),
            (
                self.modules.reference,
                tr!(self.lang, "6 参考资料 (Reference)", "6 Reference"),
            ),
            (
                self.modules.done,
                tr!(self.lang, "7 已完成 (Done)", "7 Done"),
            ),
            (
                self.modules.archived,
                tr!(self.lang, "8 归档箱 (Archived)", "8 Archived"),
            ),
            (
                self.modules.tags,
                tr!(self.lang, "9 标签库 (Tags)", "9 Tags"),
            ),
            (
                self.quotes.enabled,
                tr!(self.lang, "0 金句 (Quotes)", "0 Quotes"),
            ),
            (
                self.modules.review,
                tr!(self.lang, "r 周回顾 (Review)", "r Review"),
            ),
            (
                self.modules.settings,
                tr!(self.lang, "M 设置 (Settings)", "M Settings"),
            ),
            (nerd, icons_label),
            (
                self.start_in_capture,
                tr!(
                    self.lang,
                    "启动即快速录入 (Capture)",
                    "Start in capture mode"
                ),
            ),
            (is_ref, completion_label),
            (
                self.zen_capture,
                tr!(
                    self.lang,
                    "纯净录入无干扰 (Zen)",
                    "Zen capture (no distractions)"
                ),
            ),
            (
                self.flash_mode,
                tr!(
                    self.lang,
                    "闪念录入即退出 (Flash)",
                    "Flash mode (exit on capture)"
                ),
            ),
            (
                self.lunar_enabled,
                tr!(
                    self.lang,
                    "农历与节气提醒 (Lunar)",
                    "Lunar & holiday reminders (Lunar)"
                ),
            ),
            (
                self.sound_enabled,
                tr!(self.lang, "提示音效 (Sound)", "Sound effects (Sound)"),
            ),
            (
                self.password_enabled,
                tr!(
                    self.lang,
                    "密码保护模式 (Password)",
                    "Password protection (Password)"
                ),
            ),
            (
                self.mouse_enabled,
                tr!(self.lang, "鼠标交互 (Mouse)", "Mouse interaction (Mouse)"),
            ),
            (
                true,
                tr!(self.lang, "滚轮步长 (Scroll)", "Scroll step (Scroll)"),
            ),
        ];

        // 行文本：勾选项显示 [x]/[ ]，滚轮步长行直接显示当前步长值。
        let rows: Vec<String> = opts
            .iter()
            .enumerate()
            .map(|(i, (enabled, name))| {
                if i == MODULE_OPTION_SCROLL_STEP {
                    format!("     {}  < {} > ", name, self.mouse_scroll_step)
                } else {
                    let checkbox = if *enabled { "[x]" } else { "[ ]" };
                    format!(" {} {} ", checkbox, name)
                }
            })
            .collect();

        let base_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(self.theme.accent));
        let inner = base_block.inner(area);

        // 底部提示行（点击「切换」= Space，点击「关闭」= Esc）。
        let footer_h: u16 = if inner.height >= 4 { 1 } else { 0 };
        let visible = inner.height.saturating_sub(footer_h) as usize;
        let selected = selected.min(rows.len().saturating_sub(1));

        let title = if visible > 0 && rows.len() > visible {
            format!(
                "{}{}/{} ",
                tr!(self.lang, " 模块显示设置 ", " Module Visibility "),
                selected + 1,
                rows.len()
            )
        } else {
            tr!(self.lang, " 模块显示设置 ", " Module Visibility ").to_string()
        };
        let block = base_block.title(title);

        if visible == 0 {
            f.render_widget(List::new(Vec::<ListItem>::new()).block(block), area);
            return;
        }

        // 滚轮偏移（`popup_scroll`）由鼠标层维护，这里夹紧到合法范围，
        // 并保证键盘移动后的选中项始终可见（渲染期回写，不新增字段）。
        let max_offset = rows.len().saturating_sub(visible);
        let mut offset = self.popup_scroll.min(max_offset);
        if selected < offset {
            offset = selected;
        } else if selected >= offset + visible {
            offset = selected + 1 - visible;
        }
        self.popup_scroll = offset;

        // 列表主体可滚：鼠标滚轮按 `mouse_scroll_step` 移动可见窗口。
        self.hit_map.push_scroll(
            Rect {
                x: inner.x,
                y: inner.y,
                width: inner.width,
                height: visible as u16,
            },
            ScrollArea::Popup,
            max_offset,
        );

        let mut items: Vec<ListItem> = Vec::with_capacity(visible);
        for (i, row) in rows.iter().enumerate().skip(offset).take(visible) {
            let mut style = Style::default();
            if i == selected {
                style = style
                    .fg(self.theme.bg)
                    .bg(self.theme.accent)
                    .add_modifier(Modifier::BOLD);
            } else if !opts[i].0 {
                style = style.fg(self.theme.text_dim);
            }
            items.push(ListItem::new(Line::from(Span::styled(row.clone(), style))));

            // 整行可点：点击后由鼠标层设置弹窗下标再复用 Space 的切换逻辑。
            self.hit_map.push(
                Rect {
                    x: inner.x,
                    y: inner.y + (i - offset) as u16,
                    width: inner.width,
                    height: 1,
                },
                HitTarget::ModuleOption(i),
            );
        }
        f.render_widget(List::new(items).block(block), area);

        if footer_h == 1 {
            let key_style = Style::default()
                .fg(self.theme.accent)
                .add_modifier(Modifier::BOLD);
            let dim = Style::default().fg(self.theme.text_dim);
            let full = vec![
                (
                    vec![
                        Span::styled(" [Space] ", key_style),
                        Span::styled(tr!(self.lang, "切换", "toggle"), dim),
                    ],
                    Some(KeyCode::Char(' ')),
                ),
                (vec![Span::styled("  •  ", dim)], None),
                (
                    vec![
                        Span::styled("[Enter/Esc] ", key_style),
                        Span::styled(tr!(self.lang, "关闭", "close"), dim),
                    ],
                    Some(KeyCode::Esc),
                ),
            ];
            // 窄弹窗退化为只保留「关闭」，避免按钮被裁切到点不到。
            let compact = vec![(
                vec![
                    Span::styled("[Esc] ", key_style),
                    Span::styled(tr!(self.lang, "关闭", "close"), dim),
                ],
                Some(KeyCode::Esc),
            )];
            let width_of = |segs: &Vec<(Vec<Span<'static>>, Option<KeyCode>)>| -> usize {
                segs.iter()
                    .map(|(spans, _)| spans.iter().map(Span::width).sum::<usize>())
                    .sum()
            };
            let segments = if width_of(&full) <= inner.width as usize {
                full
            } else {
                compact
            };
            let hint = self.popup_click_line(inner, visible, segments, false);
            f.render_widget(
                Paragraph::new(hint),
                Rect {
                    x: inner.x,
                    y: inner.y + visible as u16,
                    width: inner.width,
                    height: 1,
                },
            );
        }
    }
}
