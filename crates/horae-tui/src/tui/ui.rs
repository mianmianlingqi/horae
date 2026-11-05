use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::ListItem,
};

use crate::tui::{App, View};

use horae_core::model::task::Status;
use horae_core::time;

/// 状态缩写（窄字符，用于列表前缀，保持列对齐）。
pub fn status_letter(s: &Status) -> &'static str {
    match s {
        Status::Inbox => ".",
        Status::Next => ">",
        Status::Waiting => "W",
        Status::Scheduled => "#",
        Status::Someday => "?",
        Status::Reference => "*",
        Status::Done => "x",
    }
}

/// 状态语义色：优先通过 Theme 获取各主题调校配色；提供向后兼容的双参签名。
pub fn status_color(s: &Status, is_dark: bool) -> Color {
    if is_dark {
        crate::tui::theme::Theme::catppuccin_mocha().status_color(s)
    } else {
        crate::tui::theme::Theme::catppuccin_latte().status_color(s)
    }
}

pub fn status_color_with_theme(s: &Status, theme: &crate::tui::theme::Theme) -> Color {
    theme.status_color(s)
}

/// 优先级的配色：high 红 / medium 黄 / low 蓝（各主题语义统一）。
pub fn priority_color(priority: &str) -> Option<Color> {
    crate::tui::theme::Theme::catppuccin_mocha().priority_color(priority)
}

pub fn priority_color_with_theme(
    priority: &str,
    theme: &crate::tui::theme::Theme,
) -> Option<Color> {
    theme.priority_color(priority)
}

/// 优先级的展示名 (zh, en)。用于列表/详情/预览的徽标文案。
pub fn priority_label(priority: &str) -> Option<(&'static str, &'static str)> {
    match priority {
        "high" => Some(("高", "High")),
        "medium" => Some(("中", "Medium")),
        "low" => Some(("低", "Low")),
        _ => None,
    }
}

/// 列表行内的迷你微进度条（如 [■■■□□ 3/5]），用于项目/检查单。
fn append_progress_spans(
    spans: &mut Vec<Span<'static>>,
    done: Option<usize>,
    total: Option<usize>,
    theme: &crate::tui::theme::Theme,
    is_ascii: bool,
) {
    if let (Some(d), Some(t)) = (done, total) {
        if let Some(div) = (d * 5).checked_div(t) {
            let filled = div.min(5);
            let empty = 5 - filled;
            let (fill_char, empty_char) = if is_ascii { ("=", "-") } else { ("■", "□") };
            let bar_filled = fill_char.repeat(filled);
            let bar_empty = empty_char.repeat(empty);
            let is_all_done = d == t;
            let bar_color = if is_all_done {
                theme.text_success
            } else {
                theme.accent
            };

            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("[{}{}", bar_filled, bar_empty),
                Style::default().fg(bar_color).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                format!(" {}/{}]", d, t),
                Style::default().fg(if is_all_done {
                    theme.text_success
                } else {
                    theme.text_dim
                }),
            ));
        }
    }
}

/// 列表行：渲染用的 [`ListItem`] 与鼠标命中所需的几何信息。
///
/// 行高由 `ListItem::height()`（多行行的真实高度）决定；完成标记列宽用
/// ratatui 的 unicode 宽度计算（`Line::width` 同源），中文/emoji 宽字符按 2 列计。
#[derive(Debug)]
pub(crate) struct ListRow {
    pub(crate) item: ListItem<'static>,
    /// 完成标记本身的显示宽度，不包含缩进、选择前缀或标题。
    pub(crate) marker_width: u16,
    pub(crate) marker_start: u16,
    /// 稳定任务 id（Tags/Settings 视图为伪行 id），用于命中反查，避免下标错位。
    pub(crate) task_id: String,
}

pub(crate) fn build_list_items(app: &App, max_height: usize) -> Vec<ListRow> {
    let active_pomo_task_id = if app.pomo.phase != horae_core::model::pomodoro::Phase::Idle {
        app.pomo.task_id.clone()
    } else {
        None
    };
    let (today_start, today_end) = horae_core::time::local_day_bounds(0);

    app.items
        .iter()
        .map(|r| {
            let status_enum = r.status.parse::<Status>().unwrap_or(Status::Inbox);
            let is_selected = app.selected_ids.contains(&r.id);
            let is_focus_task = active_pomo_task_id.as_deref() == Some(&r.id);
            let is_done = status_enum == Status::Done;
            // 归档箱：用归档原因取代状态语义，不再显示"已完成/逾期"。
            let is_archived = r.archive_reason.is_some();
            // 循环任务今日已打卡：✓ 标记 + 下一次执行时间。
            let is_checked_in = r.checked_in_today;
            // 金句视图：@quote 行显示为引用（前缀 "、创建时间），不显示逾期。
            let is_quote = app.view == View::Quotes
                && r.tags
                    .iter()
                    .any(|t| t == horae_core::repo::tasks::QUOTE_TAG);

            let indent = "  ".repeat(r.indent);
            let sel_prefix = if is_selected {
                " [v]"
            } else if is_focus_task {
                " 🎯"
            } else {
                ""
            };

            // 到期：相对时间 + 逾期红色强调 + 今日暖黄强调
            let reason_cn = if is_archived {
                match r.archive_reason.as_deref() {
                    Some("completed") => tr!(app.lang, "[完成]", "[Done]"),
                    Some("deleted") => tr!(app.lang, "[删除]", "[Deleted]"),
                    _ => tr!(app.lang, "[归档]", "[Archived]"),
                }
            } else {
                ""
            };
            let due_text = if is_archived || is_done || is_quote {
                time::relative_past(app.lang, r.due)
                    .map(|s| format!("~{}", s))
                    .unwrap_or_default()
            } else if is_checked_in {
                // 已打卡：重点展示下一次执行时间。
                time::relative_schedule(app.lang, r.due, r.is_deadline)
                    .map(|s| tr!(app.lang, "已打卡·下次:{}", "checked in·next:{}", s))
                    .unwrap_or_default()
            } else {
                time::relative_schedule(app.lang, r.due, r.is_deadline)
                    .map(|s| format!("~{}", s))
                    .unwrap_or_default()
            };

            let is_overdue = !is_archived
                && !is_done
                && !is_checked_in
                && !is_quote
                && r.is_deadline
                && time::is_overdue(r.due);
            let is_past_start = !is_archived
                && !is_done
                && !is_checked_in
                && !is_quote
                && !r.is_deadline
                && time::is_overdue(r.due);
            let is_today = !is_overdue
                && !is_past_start
                && !is_archived
                && !is_done
                && !is_checked_in
                && !is_quote
                && r.due
                    .map(|d| d >= today_start && d <= today_end)
                    .unwrap_or(false);

            let due_color = if is_archived || is_done || is_checked_in || is_quote {
                app.theme.text_dim
            } else if is_overdue {
                app.theme.text_urgent
            } else if is_past_start || is_today {
                app.theme.text_warn
            } else {
                app.theme.text_dim
            };

            let (letter, color) = if is_archived {
                match r.archive_reason.as_deref() {
                    Some("completed") => ("√", app.theme.text_dim),
                    Some("deleted") => ("×", app.theme.text_dim),
                    _ => ("?", app.theme.text_dim),
                }
            } else if is_checked_in {
                ("✓", app.theme.text_success)
            } else if is_quote {
                ("\"", app.theme.accent)
            } else {
                (
                    status_letter(&status_enum),
                    status_color_with_theme(&status_enum, &app.theme),
                )
            };

            let mut spans = vec![Span::styled(
                format!("{}{}{} ", sel_prefix, indent, letter),
                Style::default()
                    .fg(if is_focus_task {
                        app.theme.text_urgent
                    } else if is_selected {
                        app.theme.accent
                    } else {
                        color
                    })
                    .add_modifier(if is_focus_task {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            )];

            // 标题（选中时加粗，专注任务特殊高亮）
            let title_style = if is_focus_task {
                Style::default()
                    .fg(app.theme.text_urgent)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            spans.push(Span::styled(r.title.clone(), title_style));

            // 进度微进度条
            append_progress_spans(
                &mut spans,
                r.done,
                r.total,
                &app.theme,
                app.icon_style == crate::tui::icons::IconStyle::Ascii,
            );

            // 优先级徽标（独立字段，不再作为标签渲染）
            if let Some(ref p) = r.priority {
                if let Some((zh, en)) = priority_label(p) {
                    let c = priority_color_with_theme(p, &app.theme).unwrap_or(app.theme.hl_fg);
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        format!("!{}", app.lang.tr(zh, en)),
                        Style::default().fg(c).add_modifier(Modifier::BOLD),
                    ));
                }
            }

            // 标签（金句视图隐藏冗余的 @quote）
            let shown_tags: Vec<&String> = r
                .tags
                .iter()
                .filter(|t| !(is_quote && t.as_str() == horae_core::repo::tasks::QUOTE_TAG))
                .collect();
            if !shown_tags.is_empty() {
                spans.push(Span::raw(" "));
                for (i, t) in shown_tags.iter().enumerate() {
                    if i > 0 {
                        spans.push(Span::raw(","));
                    }
                    let c = priority_color_with_theme(t, &app.theme).unwrap_or(app.theme.hl_fg);
                    spans.push(Span::styled(format!("@{}", t), Style::default().fg(c)));
                }
            }

            // 到期 / 归档时间
            if !due_text.is_empty() || !reason_cn.is_empty() {
                spans.push(Span::styled(
                    format!("  {}{}", reason_cn, due_text),
                    Style::default()
                        .fg(due_color)
                        .add_modifier(if is_overdue || is_today {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ));
            }

            let marker_start = Span::raw(format!("{}{}", sel_prefix, indent)).width() as u16;
            let marker_width = Span::raw(letter).width() as u16;
            // Explicit line breaks belong to the same task. Preserve span styling
            // while letting List calculate its real multi-line height.
            let mut lines = Vec::new();
            let mut line = Line::default();
            for span in spans {
                for (index, part) in span.content.split('\n').enumerate() {
                    if index > 0 {
                        lines.push(line);
                        line = Line::default();
                    }
                    line.spans.push(Span::styled(part.to_owned(), span.style));
                }
            }
            lines.push(line);
            // Keep a clickable preview for items taller than the viewport.
            // The full task title remains available in the detail panel.
            lines.truncate(max_height.max(1));
            let mut item = ListItem::new(ratatui::text::Text::from(lines));
            if is_selected {
                item = item.style(Style::default().bg(app.theme.hl_bg));
            }
            ListRow {
                item,
                marker_width,
                marker_start,
                task_id: r.id.clone(),
            }
        })
        .collect()
}
