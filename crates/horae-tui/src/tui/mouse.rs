//! 鼠标交互核心：渲染期登记「命中区域」，事件期反查并派发动作。
//!
//! 设计要点：
//! - **真实 Rect**：每帧渲染时按 widget 的实际布局（边框、内边距、`ListState` 偏移、
//!   多行行高）登记，窗口缩放、筛选、排序、空列表都自然正确，不缓存跨帧坐标。
//! - **稳定 ID**：任务用任务 id（不是行下标）登记，行序变化/过滤后不会错位。
//! - **不解析文本**：左侧引导栏在生成行的同时记录 `(行号, View)`，命中只查映射。
//! - **模态优先**：`HitMap::modal()` 清空底层目标并开启模态标记；之后登记的弹窗/
//!   帮助/输入层目标进入顶层。命中永远先查顶层，模态存在时不回落到低层，
//!   因此整屏被弹窗屏蔽，底层列表/引导栏不会被误触。
//! - **仅 Left Down 触发**：`Up` 只负责释放；同一个按下期间重复的 `Down` 被忽略。
//!
//! 具体弹窗内部的行映射（如模块开关）由弹窗渲染方接入：
//! `hit_map.modal()` 后 `hit_map.push(rect, HitTarget::ModuleOption(idx))`。
//! 区域滚动的 `max` 偏移也由渲染方按当帧内容算好后随
//! [`ScrollRegion`] 一起登记，鼠标处理只做 `0..=max` 的边界夹紧。

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use horae_core::model::task::Status;
use ratatui::layout::Rect;

use super::app::{App, Mode, Pane, Popup, View};
use super::handlers::AppHandlers;

/// 可独立滚动的区域。每个区域维护自己的偏移字段，互不干扰。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ScrollArea {
    /// 左侧引导栏（视图导航）。
    Guide,
    /// 中间任务列表（滚轮移动选择，边界由 `move_sel` 夹紧）。
    List,
    /// 右侧任务详情。
    Detail,
    /// 快捷键帮助浮层。
    Help,
    /// 语法速查浮层。
    Syntax,
    /// GTD 工作流决策树（中间面板）。
    Workflow,
    /// GTD 哲学侧栏（右侧面板）。
    WorkflowSide,
    /// 弹窗内容（由弹窗渲染方登记）。
    Popup,
    Completion,
}

/// 滚动区域 + 该区域当帧允许的最大偏移（渲染方按真实内容/视口算好）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct ScrollRegion {
    pub area: ScrollArea,
    pub max: usize,
}

impl ScrollRegion {
    pub(crate) fn new(area: ScrollArea, max: usize) -> Self {
        Self { area, max }
    }

    /// 弹窗内容的滚动区域（供弹窗渲染方接入）。
    #[allow(dead_code)] // 由 render/popups.rs 接入，本文件内部暂未构造
    pub(crate) fn popup(max: usize) -> Self {
        Self::new(ScrollArea::Popup, max)
    }
}

/// 可点击目标。全部用稳定的业务标识（任务 id / View / KeyCode）表达，
/// 不依赖渲染文本，因此在中文/英文、不同主题与图标风格下都保持一致。
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum HitTarget {
    /// 等价于按下某个键（用于浮层里的按钮/提示）。
    Key(KeyCode),
    /// 设置类弹窗中的第 n 个可切换项（由弹窗渲染方登记）。
    ModuleOption(usize),
    Completion(usize),
    CloseHelp,
    CloseSyntax,
    /// 切换焦点面板。
    Pane(Pane),
    /// 选中某一行任务（按任务 id）。
    Task(String),
    /// 点击完成标记：未完成 -> Done；已完成 -> 恢复为 Next。
    Complete(String),
    /// 切换视图（引导栏）。
    View(View),
    /// 滚轮滚动区域（点击不触发动作）。
    Scroll(ScrollRegion),
}

#[derive(Clone, Debug)]
struct Hit {
    rect: Rect,
    target: HitTarget,
}

fn rect_contains(rect: &Rect, column: u16, row: u16) -> bool {
    column >= rect.x
        && column < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

fn find_target(hits: &[Hit], column: u16, row: u16) -> Option<HitTarget> {
    hits.iter()
        .rev()
        .find(|h| !matches!(h.target, HitTarget::Scroll(_)) && rect_contains(&h.rect, column, row))
        .map(|h| h.target.clone())
}

/// 一帧的命中映射。`render` 开头 [`HitMap::clear`]，之后每次绘制 widget 时登记。
#[derive(Default, Clone, Debug)]
pub(crate) struct HitMap {
    /// 底层（三栏主界面）目标，模态出现时被丢弃。
    base: Vec<Hit>,
    /// 顶层（弹窗/帮助/输入层）目标，命中优先于底层。
    overlay: Vec<Hit>,
    /// 本帧是否处于模态（弹窗/帮助/输入层已屏蔽整屏）。
    modal: bool,
}

impl HitMap {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// 开始新一帧：清空上一帧的所有目标。
    pub(crate) fn clear(&mut self) {
        self.base.clear();
        self.overlay.clear();
        self.modal = false;
    }

    /// 登记一个可点击区域。
    pub(crate) fn push(&mut self, rect: Rect, target: HitTarget) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let hit = Hit { rect, target };
        if self.modal {
            self.overlay.push(hit);
        } else {
            self.base.push(hit);
        }
    }

    /// 登记一个滚动区域（携带当帧最大偏移）。
    pub(crate) fn push_scroll(&mut self, rect: Rect, area: ScrollArea, max: usize) {
        self.push(rect, HitTarget::Scroll(ScrollRegion::new(area, max)));
    }

    /// 进入模态：丢弃底层目标，之后登记的目标全部位于顶层。
    /// 新模态层取代前一层，确保较低弹层也无法接收事件。
    pub(crate) fn modal(&mut self) {
        self.modal = true;
        self.base.clear();
        self.overlay.clear();
    }

    /// 本帧是否存在模态层（供弹窗渲染方判断是否需要继续登记顶层目标）。
    #[allow(dead_code)]
    pub(crate) fn is_modal(&self) -> bool {
        self.modal
    }

    /// 查询某坐标点击到的目标（顶层优先；模态时不回落到低层）。
    pub(crate) fn hit(&self, column: u16, row: u16) -> Option<HitTarget> {
        find_target(&self.overlay, column, row).or_else(|| {
            if self.modal {
                None
            } else {
                find_target(&self.base, column, row)
            }
        })
    }

    /// 查询某坐标的滚轮作用区域：只匹配滚动目标（行/面板目标在顶层也可被滚轮穿透），
    /// 顶层优先，模态时不回落到低层。
    pub(crate) fn scroll_at(&self, column: u16, row: u16) -> Option<ScrollRegion> {
        let find = |hits: &[Hit]| {
            hits.iter().rev().find_map(|h| match &h.target {
                HitTarget::Scroll(region) if rect_contains(&h.rect, column, row) => Some(*region),
                _ => None,
            })
        };
        find(&self.overlay).or_else(|| if self.modal { None } else { find(&self.base) })
    }
}

impl View {
    /// 该视图的行是否可以点击完成标记：归档箱 / 标签库 / 设置页是只读或伪行，
    /// 点击不改变任务状态。
    pub(crate) fn mouse_completable(self) -> bool {
        !matches!(
            self,
            View::Archived | View::Tags | View::Settings | View::Workflow
        )
    }
}

impl<'a> App<'a> {
    /// 鼠标事件统一入口（由事件循环调用）。只处理左键 Down、Up 与滚轮，
    /// 其余事件（移动、拖拽、其他按键）忽略。
    pub(crate) fn handle_mouse(&mut self, m: MouseEvent) -> Result<()> {
        if !self.mouse_enabled || self.locked {
            self.mouse_down = false;
            return Ok(());
        }
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // 同一次按下期间重复的 Down 直接忽略，直到收到 Up。
                if self.mouse_down {
                    return Ok(());
                }
                self.mouse_down = true;
                if let Some(target) = self.hit_map.hit(m.column, m.row) {
                    self.dispatch_mouse_target(target)?;
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.mouse_down = false;
            }
            MouseEventKind::ScrollUp => {
                if let Some(region) = self.hit_map.scroll_at(m.column, m.row) {
                    self.scroll_region(region, true);
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(region) = self.hit_map.scroll_at(m.column, m.row) {
                    self.scroll_region(region, false);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// 派发一次点击。业务动作全部复用现有键盘/命令路径，不新增数据库直写。
    fn dispatch_mouse_target(&mut self, target: HitTarget) -> Result<()> {
        match target {
            HitTarget::Key(code) => {
                self.handle_key(KeyEvent::new(code, KeyModifiers::NONE))?;
            }
            HitTarget::ModuleOption(idx) => {
                // 复用按键路径：先把光标放到该项，再发一个空格切换（保持弹窗打开）。
                self.popup = Some(Popup::ModuleToggles(idx));
                self.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE))?;
            }
            HitTarget::Completion(index) => {
                if self.completion_active() && index < self.completion_candidates.len() {
                    self.completion_index = index;
                    self.apply_current_completion();
                }
            }
            HitTarget::CloseHelp => self.show_help = false,
            HitTarget::CloseSyntax => self.show_syntax = false,
            HitTarget::Pane(pane) => {
                if pane != Pane::Right {
                    self.leave_checklist_focus();
                }
                self.pane = pane;
            }
            HitTarget::Task(id) => {
                self.select_task_by_id(&id);
            }
            HitTarget::Complete(id) => {
                self.toggle_complete(&id)?;
            }
            HitTarget::View(view) => {
                self.leave_checklist_focus();
                // 侧栏点击同时把焦点交给左侧面板。
                self.pane = Pane::Left;
                self.set_view(view);
            }
            // 点击滚动区域本身不产生动作（滚轮由 ScrollUp/ScrollDown 处理）。
            HitTarget::Scroll(_) => {}
        }
        Ok(())
    }

    /// 按任务 id 选中对应行（筛选/排序后仍准确），并聚焦中间面板、刷新详情。
    fn select_task_by_id(&mut self, id: &str) -> bool {
        let Some(idx) = self.items.iter().position(|r| r.id == id) else {
            return false;
        };
        self.leave_checklist_focus();
        self.selected = idx;
        if self.mode == Mode::Visual {
            self.update_visual_selection();
        }
        self.pane = Pane::Center;
        self.load_detail();
        true
    }

    fn leave_checklist_focus(&mut self) {
        if self.mode == Mode::ChecklistFocus {
            self.checklist_cursor = None;
            self.set_mode(Mode::Normal);
        }
    }

    /// 点击完成标记：复用 [`App::act_on_selected`]，因此会写事件流、压撤销栈并刷新。
    /// 已完成的行点击后恢复为 Next（保留原状态流），归档/标签/设置伪行不可完成。
    fn toggle_complete(&mut self, id: &str) -> Result<()> {
        if !self.view.mouse_completable() {
            return Ok(());
        }
        if !self.select_task_by_id(id) {
            return Ok(());
        }
        // 视觉多选下先退出多选，避免把选择集里其他任务一起改状态。
        if self.mode == Mode::Visual {
            self.set_mode(Mode::Normal);
        }
        self.selected_ids.clear();
        self.visual_start_idx = None;
        let is_done = self
            .items
            .get(self.selected)
            .and_then(|r| r.status.parse::<Status>().ok())
            == Some(Status::Done);
        let target = if is_done { Status::Next } else { Status::Done };
        self.act_on_selected(target)
    }

    /// 按区域滚动：每个区域独立偏移，并用渲染方给的 `max` 做边界夹紧。
    /// 列表滚轮等价于移动选择（由 `move_sel` 保证边界与详情刷新）。
    fn scroll_region(&mut self, region: ScrollRegion, up: bool) {
        let step = self.mouse_scroll_step.clamp(1, 100) as isize;
        let delta = if up { -step } else { step };
        let clamp = |current: usize, max: usize| -> usize {
            let current = current.min(max);
            if up {
                current.saturating_sub(step as usize)
            } else {
                current.saturating_add(step as usize).min(max)
            }
        };
        match region.area {
            ScrollArea::List => {
                self.leave_checklist_focus();
                self.move_sel(delta);
            }
            ScrollArea::Guide => self.guide_scroll = clamp(self.guide_scroll, region.max),
            ScrollArea::Detail => self.detail_scroll = clamp(self.detail_scroll, region.max),
            ScrollArea::Help => self.help_scroll = clamp(self.help_scroll, region.max),
            ScrollArea::Syntax => self.syntax_scroll = clamp(self.syntax_scroll, region.max),
            ScrollArea::Completion => {
                self.completion_index = clamp(self.completion_index, region.max);
            }
            ScrollArea::Workflow => self.workflow_scroll = clamp(self.workflow_scroll, region.max),
            ScrollArea::WorkflowSide => {
                self.workflow_side_scroll = clamp(self.workflow_side_scroll, region.max)
            }
            ScrollArea::Popup => {
                self.popup_scroll = clamp(self.popup_scroll, region.max);
                if let Some(Popup::ModuleToggles(index)) = self.popup.as_mut() {
                    *index = (*index as isize + delta)
                        .clamp(0, super::handlers::MODULE_OPTION_COUNT as isize - 1)
                        as usize;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: u16, y: u16, width: u16, height: u16) -> Rect {
        Rect::new(x, y, width, height)
    }

    #[test]
    fn later_targets_win_within_the_same_layer() {
        let mut map = HitMap::new();
        map.push(rect(0, 0, 10, 2), HitTarget::Task("a".into()));
        map.push(rect(0, 0, 2, 1), HitTarget::Complete("a".into()));
        assert_eq!(
            map.hit(1, 0),
            Some(HitTarget::Complete("a".into())),
            "完成标记压在整行之上"
        );
        assert_eq!(
            map.hit(5, 0),
            Some(HitTarget::Task("a".into())),
            "标记区域之外仍是整行"
        );
    }

    #[test]
    fn out_of_bounds_is_a_miss() {
        let mut map = HitMap::new();
        map.push(rect(3, 4, 2, 2), HitTarget::Pane(Pane::Center));
        assert_eq!(map.hit(2, 4), None);
        assert_eq!(map.hit(5, 4), None);
        assert_eq!(map.hit(3, 6), None);
        assert_eq!(map.hit(3, 4), Some(HitTarget::Pane(Pane::Center)));
    }

    #[test]
    fn modal_drops_underlying_targets() {
        let mut map = HitMap::new();
        map.push(rect(0, 0, 100, 100), HitTarget::View(View::Inbox));
        map.modal();
        assert!(map.is_modal());
        assert_eq!(map.hit(10, 10), None, "模态屏蔽整屏底层目标");
        map.push(rect(10, 10, 5, 5), HitTarget::ModuleOption(3));
        assert_eq!(map.hit(11, 11), Some(HitTarget::ModuleOption(3)));
        // 新弹层必须屏蔽之前的弹层。
        map.modal();
        assert_eq!(map.hit(11, 11), None);
    }

    #[test]
    fn scrolling_prefers_scroll_targets_and_respects_modal() {
        let mut map = HitMap::new();
        map.push(
            rect(0, 0, 40, 10),
            HitTarget::Scroll(ScrollRegion::new(ScrollArea::List, 0)),
        );
        map.push(rect(0, 1, 40, 1), HitTarget::Task("a".into()));
        assert_eq!(
            map.scroll_at(5, 1),
            Some(ScrollRegion::new(ScrollArea::List, 0)),
            "行目标不应吞掉滚轮"
        );
        map.modal();
        assert_eq!(map.scroll_at(5, 1), None, "模态下底层不接收滚轮");
        map.push(
            rect(10, 10, 20, 5),
            HitTarget::Scroll(ScrollRegion::popup(7)),
        );
        assert_eq!(map.scroll_at(11, 11), Some(ScrollRegion::popup(7)));
    }

    #[test]
    fn key_target_carries_keycode() {
        let mut map = HitMap::new();
        map.push(rect(0, 0, 4, 1), HitTarget::Key(KeyCode::Esc));
        assert_eq!(map.hit(0, 0), Some(HitTarget::Key(KeyCode::Esc)));
    }
}
