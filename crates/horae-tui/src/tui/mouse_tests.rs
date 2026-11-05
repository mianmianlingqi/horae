use super::*;
use crate::tui::app::{Mode, Popup};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use horae_core::db::migrate;
use horae_core::model::task::Status;
use horae_core::repo::{settings, tasks};
use ratatui::backend::TestBackend;

fn database() -> Connection {
    horae_core::repo::state::set_test_override();
    let mut conn = Connection::open_in_memory().unwrap();
    migrate::run(&mut conn).unwrap();
    settings::set(&conn, "start_capture", "0").unwrap();
    conn
}

fn capture(conn: &Connection, title: &str) -> String {
    tasks::create_capture(
        conn,
        &tasks::CaptureInput {
            title: title.into(),
            ..Default::default()
        },
    )
    .unwrap()
    .id
}

fn draw(app: &mut App<'_>, term: &mut Terminal<TestBackend>) {
    // Match existing snapshot tests: TestBackend retains hidden continuation
    // cells from prior CJK frames when only a terminal diff is applied.
    term.clear().unwrap();
    term.draw(|f| app.render(f)).unwrap();
}

// Locate actual painted cells, retaining their terminal columns even for CJK.
// This deliberately does not derive expected coordinates from HitMap.
fn painted(term: &Terminal<TestBackend>, label: &str) -> (u16, u16) {
    let label: String = label.chars().filter(|c| !c.is_whitespace()).collect();
    let buf = term.backend().buffer();
    let mut found = None;
    for y in 0..buf.area().height {
        let mut text = String::new();
        let mut columns = Vec::new();
        for x in 0..buf.area().width {
            for c in buf[(x, y)].symbol().chars().filter(|c| !c.is_whitespace()) {
                columns.extend(std::iter::repeat_n(x, c.len_utf8()));
                text.push(c);
            }
        }
        if let Some(index) = text.find(&label) {
            let at = (columns[index], y);
            if found.is_none_or(|previous: (u16, u16)| at.0 < previous.0) {
                found = Some(at);
            }
        }
    }
    found.unwrap_or_else(|| panic!("label not painted: {label}\n{buf:?}"))
}

fn mouse(app: &mut App<'_>, at: (u16, u16), kind: MouseEventKind) {
    app.handle_mouse(MouseEvent {
        kind,
        column: at.0,
        row: at.1,
        modifiers: KeyModifiers::NONE,
    })
    .unwrap();
}

fn click(app: &mut App<'_>, at: (u16, u16)) {
    mouse(app, at, MouseEventKind::Down(MouseButton::Left));
    mouse(app, at, MouseEventKind::Up(MouseButton::Left));
}

#[test]
fn mouse_selects_painted_chinese_task_and_updates_detail() {
    let conn = database();
    capture(&conn, "Mouse甲第一项");
    capture(&conn, "Mouse乙第二项");
    let mut app = App::new(&conn).unwrap();
    let mut term = Terminal::new(TestBackend::new(120, 36)).unwrap();
    draw(&mut app, &mut term);
    let wanted = app
        .items
        .iter()
        .find(|r| r.id != app.items[app.selected].id)
        .unwrap()
        .clone();
    let at = painted(&term, &wanted.title);
    click(&mut app, at);
    assert_eq!(app.items[app.selected].id, wanted.id);
    assert_eq!(app.detail.as_ref().unwrap().task.id, wanted.id);
    assert_eq!(app.pane, Pane::Center);
}

#[test]
fn mouse_completion_uses_business_transition_and_never_repeats_on_up() {
    let conn = database();
    capture(&conn, "MouseComplete甲");
    capture(&conn, "MouseComplete乙");
    let mut app = App::new(&conn).unwrap();
    let mut term = Terminal::new(TestBackend::new(120, 36)).unwrap();
    draw(&mut app, &mut term);
    let row = app.items[0].clone();
    let title_at = painted(&term, &row.title);
    let at = (title_at.0 - 2, title_at.1);
    mouse(&mut app, at, MouseEventKind::Down(MouseButton::Left));
    assert_eq!(tasks::get(&conn, &row.id).unwrap().status, Status::Done);
    let remaining = app.items[0].id.clone();
    draw(&mut app, &mut term);
    mouse(&mut app, at, MouseEventKind::Down(MouseButton::Left));
    mouse(&mut app, at, MouseEventKind::Up(MouseButton::Left));
    assert_eq!(tasks::get(&conn, &remaining).unwrap().status, Status::Inbox);
    app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE))
        .unwrap();
    assert_eq!(tasks::get(&conn, &row.id).unwrap().status, Status::Inbox);
}

#[test]
fn mouse_navigation_empty_lists_focus_and_disabled_mouse() {
    let conn = database();
    let mut app = App::new(&conn).unwrap();
    let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
    draw(&mut app, &mut term);
    let today = painted(&term, "今日");
    click(&mut app, today);
    assert_eq!(app.view, View::Today);
    assert!(app.items.is_empty());
    draw(&mut app, &mut term);
    let tomorrow = painted(&term, "明日");
    app.mouse_enabled = false;
    click(&mut app, tomorrow);
    assert_eq!(app.view, View::Today);
    app.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE))
        .unwrap();
    assert_eq!(app.view, View::Next);
    assert_eq!(app.selected, 0);
}

#[test]
fn mouse_modal_blocks_background_and_input_buttons_submit_once() {
    let conn = database();
    let id = capture(&conn, "MouseBehindModal");
    let mut app = App::new(&conn).unwrap();
    let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
    draw(&mut app, &mut term);
    let background = painted(&term, "MouseBehindModal");
    let nav = painted(&term, "明日");
    app.popup = Some(Popup::ModuleToggles(0));
    draw(&mut app, &mut term);
    click(&mut app, background);
    click(&mut app, nav);
    assert_eq!(app.view, View::Inbox);
    assert_eq!(tasks::get(&conn, &id).unwrap().status, Status::Inbox);
    assert!(app.popup.is_some());
    app.popup = None;
    app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE))
        .unwrap();
    app.input = "MouseCapturedOnce".into();
    app.input_cursor = app.input.len();
    draw(&mut app, &mut term);
    let confirm = painted(&term, "[确认]");
    click(&mut app, confirm);
    draw(&mut app, &mut term);
    assert!(app.items.iter().any(|r| r.title == "MouseCapturedOnce"));
    assert_eq!(
        app.items
            .iter()
            .filter(|r| r.title == "MouseCapturedOnce")
            .count(),
        1
    );
    assert_ne!(app.mode, Mode::ConfirmPurge);
}

#[test]
fn mouse_wheel_regions_are_independent_and_scrolled_rows_remain_clickable() {
    let conn = database();
    for i in 0..45 {
        capture(&conn, &format!("MouseScroll{i:02}中文"));
    }
    let mut app = App::new(&conn).unwrap();
    let id = app.items[0].id.clone();
    tasks::update_notes(&conn, &id, &"中文详情以及宽字符内容\n".repeat(90)).unwrap();
    app.load_detail();
    let mut term = Terminal::new(TestBackend::new(120, 20)).unwrap();
    draw(&mut app, &mut term);
    let detail_title = painted(&term, "任务详情");
    let detail_at = (detail_title.0, detail_title.1 + 1);
    let selected = app.selected;
    mouse(&mut app, detail_at, MouseEventKind::ScrollDown);
    assert!(app.detail_scroll > 0);
    assert_eq!(app.selected, selected);
    assert_eq!(app.guide_scroll, 0);
    draw(&mut app, &mut term);
    let detail_offset = app.detail_scroll;
    let guide = painted(&term, "引导");
    mouse(&mut app, (guide.0, guide.1 + 1), MouseEventKind::ScrollDown);
    assert!(app.guide_scroll > 0);
    assert_eq!(app.detail_scroll, detail_offset);
    assert_eq!(app.selected, selected);
    draw(&mut app, &mut term);
    let first = painted(&term, &app.items[0].title);
    for _ in 0..8 {
        mouse(&mut app, first, MouseEventKind::ScrollDown);
        draw(&mut app, &mut term);
    }
    assert!(app.list_state.offset() > 0);
    let wanted = app.items[app.selected].clone();
    let at = painted(&term, &wanted.title);
    click(&mut app, at);
    assert_eq!(app.detail.as_ref().unwrap().task.id, wanted.id);
    assert_eq!(app.pane, Pane::Center);
}

#[test]
fn mouse_filter_sort_resize_and_multiline_titles_keep_task_identity() {
    let conn = database();
    capture(&conn, "MouseFilter甲\n第二行中文");
    capture(&conn, "MouseFilter乙");
    capture(&conn, "Unrelated");
    let mut app = App::new(&conn).unwrap();
    app.search_query = "MouseFilter".into();
    app.refresh().unwrap();
    assert_eq!(app.items.len(), 2);
    app.items.reverse();
    let wanted = app.items[1].clone();
    let mut term = Terminal::new(TestBackend::new(120, 32)).unwrap();
    draw(&mut app, &mut term);
    let at = painted(&term, wanted.title.lines().next().unwrap());
    click(&mut app, at);
    assert_eq!(app.items[app.selected].id, wanted.id);
    term.backend_mut().resize(90, 24);
    term.autoresize().unwrap();
    draw(&mut app, &mut term);
    let at = painted(&term, wanted.title.lines().next().unwrap());
    click(&mut app, at);
    assert_eq!(app.detail.as_ref().unwrap().task.id, wanted.id);
    for (width, height) in [(20, 6), (1, 1), (0, 0)] {
        term.backend_mut().resize(width, height);
        term.autoresize().unwrap();
        draw(&mut app, &mut term);
        click(&mut app, (0, 0));
    }
}

#[test]
fn mouse_help_scroll_is_bounded_and_blocks_list_wheel() {
    let conn = database();
    capture(&conn, "MouseHelp");
    let mut app = App::new(&conn).unwrap();
    let mut term = Terminal::new(TestBackend::new(120, 20)).unwrap();
    app.show_help = true;
    draw(&mut app, &mut term);
    let help = painted(&term, "快捷键指南");
    let at = (help.0, help.1 + 1);
    for _ in 0..100 {
        mouse(&mut app, at, MouseEventKind::ScrollDown);
        draw(&mut app, &mut term);
    }
    let bottom = app.help_scroll;
    mouse(&mut app, at, MouseEventKind::ScrollDown);
    assert_eq!(app.help_scroll, bottom);
    assert_eq!(app.selected, 0);
    assert!(bottom > 0 && bottom < 500);
    for _ in 0..100 {
        mouse(&mut app, at, MouseEventKind::ScrollUp);
        draw(&mut app, &mut term);
    }
    assert_eq!(app.help_scroll, 0);
}

#[test]
fn mouse_single_completion_does_not_apply_to_keyboard_multiselection() {
    let conn = database();
    capture(&conn, "MouseSingle甲");
    capture(&conn, "MouseSingle乙");
    let mut app = App::new(&conn).unwrap();
    app.select_all();
    let target = app.items[0].clone();
    let other = app.items[1].id.clone();
    let mut term = Terminal::new(TestBackend::new(120, 36)).unwrap();
    draw(&mut app, &mut term);
    let at = painted(&term, &target.title);
    click(&mut app, (at.0 - 2, at.1));
    assert_eq!(tasks::get(&conn, &target.id).unwrap().status, Status::Done);
    assert_eq!(tasks::get(&conn, &other).unwrap().status, Status::Inbox);
    assert!(app.selected_ids.is_empty());
    assert_eq!(app.mode, Mode::Normal);
}

#[test]
fn mouse_settings_defaults_invalid_values_and_keyboard_toggle_are_compatible() {
    let conn = database();
    let app = App::new(&conn).unwrap();
    assert!(app.mouse_enabled);
    assert_eq!(app.mouse_scroll_step, 3);
    settings::set(&conn, "mouse_scroll_step", "invalid").unwrap();
    assert_eq!(App::new(&conn).unwrap().mouse_scroll_step, 3);
    settings::set(&conn, "mouse_scroll_step", "0").unwrap();
    assert_eq!(App::new(&conn).unwrap().mouse_scroll_step, 1);
    settings::set(&conn, "mouse_scroll_step", "10000").unwrap();
    assert_eq!(App::new(&conn).unwrap().mouse_scroll_step, 100);
    settings::set(&conn, "mouse_enabled", "0").unwrap();
    let mut app = App::new(&conn).unwrap();
    assert!(!app.mouse_enabled);
    app.popup = Some(Popup::ModuleToggles(super::handlers::MODULE_OPTION_MOUSE));
    app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE))
        .unwrap();
    assert!(app.mouse_enabled);
    assert!(App::new(&conn).unwrap().mouse_enabled);
}

#[test]
fn mouse_confirmation_requires_button_and_preserves_cancel() {
    let conn = database();
    let id = capture(&conn, "MouseArchiveConfirm");
    let mut app = App::new(&conn).unwrap();
    let mut term = Terminal::new(TestBackend::new(120, 36)).unwrap();
    app.handle_key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::NONE))
        .unwrap();
    assert_eq!(app.mode, Mode::ConfirmArchive);
    draw(&mut app, &mut term);
    click(&mut app, (0, 0));
    assert_eq!(app.mode, Mode::ConfirmArchive);
    assert!(tasks::get(&conn, &id).unwrap().archived_at.is_none());
    let cancel = painted(&term, "[n/Esc]");
    click(&mut app, cancel);
    assert_eq!(app.mode, Mode::Normal);
    assert!(tasks::get(&conn, &id).unwrap().archived_at.is_none());
    app.handle_key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::NONE))
        .unwrap();
    draw(&mut app, &mut term);
    let confirm = painted(&term, "[y/Enter]");
    click(&mut app, confirm);
    assert!(tasks::get(&conn, &id).unwrap().archived_at.is_some());
}

#[test]
fn mouse_small_module_popup_scrolls_and_clicks_visible_option() {
    let conn = database();
    let mut app = App::new(&conn).unwrap();
    app.popup = Some(Popup::ModuleToggles(0));
    let mut term = Terminal::new(TestBackend::new(120, 14)).unwrap();
    draw(&mut app, &mut term);
    let title = painted(&term, "模块显示设置");
    for _ in 0..7 {
        mouse(&mut app, (title.0, title.1 + 2), MouseEventKind::ScrollDown);
        draw(&mut app, &mut term);
    }
    assert!(app.popup_scroll > 0);
    let step = painted(&term, "滚轮步长");
    let before = app.mouse_scroll_step;
    click(&mut app, step);
    assert_ne!(app.mouse_scroll_step, before);
    assert!(app.popup.is_some());
}

#[test]
fn mouse_new_popup_masks_help_buttons_and_all_background_scroll() {
    let conn = database();
    capture(&conn, "MouseLayered");
    let mut app = App::new(&conn).unwrap();
    let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
    app.show_help = true;
    draw(&mut app, &mut term);
    let help = painted(&term, "快捷键指南");
    app.popup = Some(Popup::ModuleToggles(0));
    draw(&mut app, &mut term);
    click(&mut app, help);
    mouse(&mut app, help, MouseEventKind::ScrollDown);
    assert!(app.popup.is_some());
    assert!(app.show_help);
    assert_eq!(app.help_scroll, 0);
}

#[test]
fn mouse_clicks_empty_panel_border_to_focus_without_changing_task() {
    let conn = database();
    let mut app = App::new(&conn).unwrap();
    let mut term = Terminal::new(TestBackend::new(120, 36)).unwrap();
    draw(&mut app, &mut term);
    let right = painted(&term, "任务详情");
    click(&mut app, right);
    assert_eq!(app.pane, Pane::Right);
    let left = painted(&term, "引导");
    click(&mut app, left);
    assert_eq!(app.pane, Pane::Left);
    let center = painted(&term, "任务·收件箱");
    click(&mut app, center);
    assert_eq!(app.pane, Pane::Center);
    assert!(app.items.is_empty());
}

#[test]
fn mouse_done_restore_and_recurring_completion_keep_existing_semantics() {
    let conn = database();
    let done = capture(&conn, "MouseDoneRestore");
    tasks::transition(&conn, &done, Status::Done).unwrap();
    let habit = tasks::create_capture(
        &conn,
        &tasks::CaptureInput {
            title: "MouseHabit".into(),
            status: Status::Scheduled,
            due_at: Some(horae_core::time::now_ms()),
            rrule: Some("FREQ=DAILY".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let mut app = App::new(&conn).unwrap();
    let mut term = Terminal::new(TestBackend::new(120, 36)).unwrap();
    app.set_view(View::Done);
    draw(&mut app, &mut term);
    let at = painted(&term, "MouseDoneRestore");
    click(&mut app, (at.0 - 2, at.1));
    assert_eq!(tasks::get(&conn, &done).unwrap().status, Status::Next);
    app.set_view(View::Scheduled);
    draw(&mut app, &mut term);
    let at = painted(&term, "MouseHabit");
    click(&mut app, (at.0 - 2, at.1));
    let rescheduled = tasks::get(&conn, &habit.id).unwrap();
    assert_eq!(rescheduled.status, Status::Scheduled);
    assert!(rescheduled.due_at > habit.due_at);
}

#[test]
fn mouse_completion_candidate_uses_current_input_completion() {
    let conn = database();
    let mut app = App::new(&conn).unwrap();
    app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE))
        .unwrap();
    app.input = "MouseCapture @".into();
    app.input_cursor = app.input.len();
    app.refresh_completion();
    assert!(app.completion_active());
    let wanted = app.completion_candidates[1].clone();
    let mut term = Terminal::new(TestBackend::new(140, 50)).unwrap();
    draw(&mut app, &mut term);
    let at = painted(&term, &format!("@{wanted}"));
    click(&mut app, at);
    assert!(app.input.contains(&format!("@{wanted}")));
    assert!(app.input.starts_with("MouseCapture"));
}

#[test]
fn mouse_multiline_continuation_selects_task_without_completing_it() {
    let conn = database();
    let id = capture(&conn, "Mouse多行标题\n第二行中文内容");
    capture(&conn, "MouseAnother");
    let mut app = App::new(&conn).unwrap();
    let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
    draw(&mut app, &mut term);
    let at = painted(&term, "第二行中文内容");
    click(&mut app, at);
    assert_eq!(app.detail.as_ref().unwrap().task.id, id);
    assert_eq!(tasks::get(&conn, &id).unwrap().status, Status::Inbox);
    let first = painted(&term, "Mouse多行标题");
    click(&mut app, (first.0 - 2, at.1));
    assert_eq!(tasks::get(&conn, &id).unwrap().status, Status::Inbox);
}

#[test]
fn mouse_wrapped_detail_can_reach_the_last_line() {
    let conn = database();
    let id = capture(&conn, "MouseWrappedDetails");
    let notes = format!("{}\nMouseDetailsEnd", "中".repeat(18).repeat(100));
    tasks::update_notes(&conn, &id, &notes).unwrap();
    let mut app = App::new(&conn).unwrap();
    let mut term = Terminal::new(TestBackend::new(120, 20)).unwrap();
    draw(&mut app, &mut term);
    let title = painted(&term, "任务详情");
    let at = (title.0, title.1 + 1);
    app.mouse_scroll_step = 100;
    for _ in 0..10 {
        mouse(&mut app, at, MouseEventKind::ScrollDown);
        draw(&mut app, &mut term);
    }
    painted(&term, "MouseDetailsEnd");
    let bottom = app.detail_scroll;
    mouse(&mut app, at, MouseEventKind::ScrollDown);
    assert_eq!(app.detail_scroll, bottom);
}

#[test]
fn mouse_task_navigation_leaves_checklist_focus() {
    let conn = database();
    let first = capture(&conn, "MouseChecklist甲");
    let second = capture(&conn, "MouseChecklist乙");
    tasks::add_checklist_item(&conn, &first, "第一项").unwrap();
    tasks::add_checklist_item(&conn, &second, "第二项").unwrap();
    let mut app = App::new(&conn).unwrap();
    app.mode = Mode::ChecklistFocus;
    app.checklist_cursor = Some(0);
    let wanted = app
        .items
        .iter()
        .find(|r| r.id != app.items[app.selected].id)
        .unwrap()
        .clone();
    let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
    draw(&mut app, &mut term);
    let at = painted(&term, &wanted.title);
    click(&mut app, at);
    assert_eq!(app.mode, Mode::Normal);
    assert_eq!(app.checklist_cursor, None);
    app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE))
        .unwrap();
    assert!(!tasks::get(&conn, &wanted.id).unwrap().checklist[0].done);
}

#[test]
fn mouse_help_after_keyboard_bottom_scrolls_up_by_one_step() {
    let conn = database();
    let mut app = App::new(&conn).unwrap();
    let mut term = Terminal::new(TestBackend::new(120, 20)).unwrap();
    app.show_help = true;
    app.handle_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE))
        .unwrap();
    draw(&mut app, &mut term);
    let bottom = app.help_scroll;
    assert!(bottom > app.mouse_scroll_step);
    let title = painted(&term, "快捷键指南");
    mouse(&mut app, (title.0, title.1 + 1), MouseEventKind::ScrollUp);
    assert_eq!(app.help_scroll, bottom - app.mouse_scroll_step);
}

#[test]
fn mouse_help_close_never_cancels_underlying_input() {
    let conn = database();
    let mut app = App::new(&conn).unwrap();
    app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE))
        .unwrap();
    app.zen_capture = false;
    app.input = "MouseUnsavedInput".into();
    app.input_cursor = app.input.len();
    app.show_help = true;
    let mut term = Terminal::new(TestBackend::new(120, 36)).unwrap();
    draw(&mut app, &mut term);
    let title = painted(&term, "快捷键指南");
    click(&mut app, title);
    assert!(!app.show_help);
    assert_eq!(app.mode, Mode::Capturing);
    assert_eq!(app.input, "MouseUnsavedInput");
}

#[test]
fn mouse_task_taller_than_viewport_keeps_clickable_preview() {
    let conn = database();
    let id = capture(
        &conn,
        &format!("MouseTallTask\n{}", "更多中文行\n".repeat(40)),
    );
    let mut app = App::new(&conn).unwrap();
    let mut term = Terminal::new(TestBackend::new(120, 10)).unwrap();
    draw(&mut app, &mut term);
    let at = painted(&term, "MouseTallTask");
    click(&mut app, at);
    assert_eq!(app.detail.as_ref().unwrap().task.id, id);
    assert_eq!(app.pane, Pane::Center);
    assert_eq!(tasks::get(&conn, &id).unwrap().status, Status::Inbox);
}
