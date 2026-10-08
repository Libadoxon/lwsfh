use std::path::PathBuf;

use gpui::{
    App, Context, FocusHandle, InteractiveElement, IntoElement, KeyBinding, ModifiersChangedEvent,
    ParentElement, Render, Styled, Window, actions, div, img, px, relative,
};
use gpui_component::{ActiveTheme, Icon, IconName};

use crate::daemon::Event;
use crate::{hyprland, icon};

const CONTEXT: &str = "SwitcherView";
const TITLE_MAX: usize = 120;
const TITLE_WIDTH: f32 = 320.0;
const TITLE_FONT_SIZE: f32 = 14.0;
const TITLE_PAD_X: f32 = 24.0; // px_3 on both sides

const MAX_BOX: f32 = 100.0;
const MIN_BOX: f32 = 80.0;
const ICON_RATIO: f32 = 0.8;
const MACOS_SELECTED_RATIO: f32 = 1.0; // selected icon fills its box in macOS style
const ITEM_GAP: f32 = 12.0; // matches gap_3
const BAR_PADDING_X: f32 = 32.0; // matches px_4 on both sides
const SCREEN_USABLE: f32 = 0.9;
const BAR_OPACITY: f32 = 0.5;
const ITEM_OPACITY: f32 = 0.35;
const ITEM_SELECTED_OPACITY: f32 = 0.55;
const FALLBACK_SCREEN_WIDTH: f32 = 1920.0;
const SPACE_FROM_TOP: f32 = 0.2;

actions!(lwsfh, [SelectNext, SelectPrev, ConfirmSelection, Cancel]);

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("super-tab", SelectNext, Some(CONTEXT)),
        KeyBinding::new("super-right", SelectNext, Some(CONTEXT)),
        KeyBinding::new("super-shift-tab", SelectPrev, Some(CONTEXT)),
        KeyBinding::new("super-left", SelectPrev, Some(CONTEXT)),
        KeyBinding::new("super-enter", ConfirmSelection, Some(CONTEXT)),
        KeyBinding::new("super-escape", Cancel, Some(CONTEXT)),
    ]);
}

pub struct Entry {
    address: String,
    title: String,
    workspace: i32,
    icon: Option<PathBuf>,
}

impl Entry {
    pub fn from_window(w: &hyprland::Window) -> Self {
        Self {
            address: w.address.clone(),
            title: w.display_title().to_string(),
            workspace: w.workspace,
            icon: icon::resolve(&w.class),
        }
    }
}

pub struct SwitcherView {
    entries: Vec<Entry>,
    selected: usize,
    macos: bool,
    focus_handle: FocusHandle,
    events: flume::Sender<Event>,
}

impl SwitcherView {
    pub fn new(
        entries: Vec<Entry>,
        selected: usize,
        macos: bool,
        events: flume::Sender<Event>,
        cx: &mut Context<Self>,
    ) -> Self {
        let selected = selected.min(entries.len().saturating_sub(1));
        Self {
            entries,
            selected,
            macos,
            focus_handle: cx.focus_handle(),
            events,
        }
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    pub fn cycle(&mut self, forward: bool, cx: &mut Context<Self>) {
        let len = self.entries.len();
        if len == 0 {
            return;
        }
        self.selected = if forward {
            (self.selected + 1) % len
        } else {
            (self.selected + len - 1) % len
        };
        cx.notify();
    }

    fn select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        self.cycle(true, cx);
    }

    fn select_prev(&mut self, _: &SelectPrev, _: &mut Window, cx: &mut Context<Self>) {
        self.cycle(false, cx);
    }

    fn confirm(&mut self, _: &ConfirmSelection, _: &mut Window, _: &mut Context<Self>) {
        self.emit_confirm();
    }

    fn cancel(&mut self, _: &Cancel, _: &mut Window, _: &mut Context<Self>) {
        let _ = self.events.send(Event::Cancel);
    }

    // Wayland reports Super as `platform`, and sends the modifier state on keyboard enter as
    // well as on every change. Any report without Super held ends the release-to-select
    // gesture, whether Super was let go while the overlay was up or already before it grabbed
    // the keyboard. Other modifier changes (e.g. Shift for reverse cycling) keep Super set.
    fn on_modifiers(&mut self, ev: &ModifiersChangedEvent, _: &mut Window, _: &mut Context<Self>) {
        if !ev.modifiers.platform {
            self.emit_confirm();
        }
    }

    fn emit_confirm(&mut self) {
        let event = match self.entries.get(self.selected) {
            Some(entry) => Event::Confirm(entry.address.clone()),
            None => Event::Cancel,
        };
        let _ = self.events.send(event);
    }
}

impl Render for SwitcherView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let row_width = usable_row_width(screen_width(window));
        let count = self.entries.len();
        let box_size = box_size(count, row_width);
        let icon_size = (box_size * ICON_RATIO).floor();
        let capacity = capacity(row_width, box_size);

        let selected_title = self
            .entries
            .get(self.selected)
            .map(|e| clip(&e.title, TITLE_MAX))
            .unwrap_or_default();

        let slots = paged_slots(count, self.selected, capacity);
        let slot_count = slots.len();
        let content_width =
            slot_count as f32 * box_size + slot_count.saturating_sub(1) as f32 * ITEM_GAP;
        let selected_slot = slots
            .iter()
            .position(|s| matches!(s, Slot::Item(i) if *i == self.selected))
            .unwrap_or(0);
        let icon_center = selected_slot as f32 * (box_size + ITEM_GAP) + box_size / 2.0;
        // Measure the title so its box hugs the text: centre it under the icon, clamp to the row
        // edges (so edge items start flush and run outward rather than truncating), and only cap
        // the width — truncating — when the text is longer than the max.
        let run = window.text_style().to_run(selected_title.len());
        let text_width = window
            .text_system()
            .layout_line(&selected_title, px(TITLE_FONT_SIZE), &[run], None)
            .width
            .to_f64() as f32;
        let title_width = (text_width + TITLE_PAD_X)
            .min(TITLE_WIDTH)
            .min(content_width);
        let title_left =
            (icon_center - title_width / 2.0).clamp(0.0, (content_width - title_width).max(0.0));

        let macos = self.macos;
        let boxes = slots.into_iter().map(|slot| match slot {
            Slot::Item(i) => item(
                &self.entries[i],
                i == self.selected,
                box_size,
                icon_size,
                macos,
                cx,
            )
            .into_any_element(),
            Slot::ArrowLeft => arrow_box(true, box_size, icon_size, cx).into_any_element(),
            Slot::ArrowRight => arrow_box(false, box_size, icon_size, cx).into_any_element(),
        });

        div()
            .track_focus(&self.focus_handle)
            .key_context(CONTEXT)
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_prev))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::cancel))
            .on_modifiers_changed(cx.listener(Self::on_modifiers))
            .size_full()
            .relative()
            .child(
                div()
                    .absolute()
                    .top(relative(SPACE_FROM_TOP))
                    .left_0()
                    .right_0()
                    .flex()
                    .flex_row()
                    .justify_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_0p5()
                            .pt_3()
                            .pb_0p5()
                            .px_4()
                            .bg(theme.popover.opacity(BAR_OPACITY))
                            .border_1()
                            .border_color(theme.border)
                            .rounded_xl()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_3()
                                    .children(boxes),
                            )
                            .child(
                                div().w(px(content_width)).flex().flex_row().child(
                                    div()
                                        .ml(px(title_left))
                                        .w(px(title_width))
                                        .px_3()
                                        .py_1()
                                        .rounded_md()
                                        .bg(if macos {
                                            theme.secondary.opacity(0.0)
                                        } else {
                                            theme.secondary.opacity(ITEM_OPACITY)
                                        })
                                        .text_size(px(TITLE_FONT_SIZE))
                                        .text_center()
                                        .text_color(theme.foreground)
                                        .truncate()
                                        .child(selected_title),
                                ),
                            ),
                    ),
            )
    }
}

fn screen_width(window: &Window) -> f32 {
    let width = window.viewport_size().width.to_f64() as f32;
    if width >= 1.0 {
        width
    } else {
        FALLBACK_SCREEN_WIDTH
    }
}

fn usable_row_width(screen_width: f32) -> f32 {
    (screen_width * SCREEN_USABLE - BAR_PADDING_X).max(MIN_BOX)
}

fn box_size(count: usize, row_width: f32) -> f32 {
    if count == 0 {
        return MAX_BOX;
    }
    let slot = row_width / count as f32;
    (slot - ITEM_GAP).clamp(MIN_BOX, MAX_BOX).floor()
}

fn capacity(row_width: f32, box_size: f32) -> usize {
    (((row_width + ITEM_GAP) / (box_size + ITEM_GAP)).floor() as usize).max(1)
}

enum Slot {
    ArrowLeft,
    Item(usize),
    ArrowRight,
}

/// Choose which boxes fill a bar `capacity` slots wide. The selection is kept centred; the
/// window only slides toward an edge once there is nothing more to scroll that way. Each
/// truncated edge becomes an arrow slot, so the arrow sits in a real box position.
fn paged_slots(count: usize, selected: usize, capacity: usize) -> Vec<Slot> {
    if count <= capacity {
        return (0..count).map(Slot::Item).collect();
    }
    if capacity < 3 {
        let start = selected.min(count - capacity);
        return (start..start + capacity).map(Slot::Item).collect();
    }

    let center = (capacity - 1) / 2;
    let first = selected.saturating_sub(center).min(count - capacity);
    let has_left = first > 0;
    let has_right = first + capacity < count;

    (0..capacity)
        .map(|p| {
            if p == 0 && has_left {
                Slot::ArrowLeft
            } else if p == capacity - 1 && has_right {
                Slot::ArrowRight
            } else {
                Slot::Item(first + p)
            }
        })
        .collect()
}

fn item(
    entry: &Entry,
    selected: bool,
    box_size: f32,
    icon_size: f32,
    macos: bool,
    cx: &Context<SwitcherView>,
) -> impl IntoElement {
    let theme = cx.theme();
    // macOS style shows selection by growing the icon instead of a background highlight.
    let background = if selected && !macos {
        theme.secondary.opacity(ITEM_SELECTED_OPACITY)
    } else {
        theme.secondary.opacity(0.0)
    };
    let icon_size = if selected && macos {
        box_size * MACOS_SELECTED_RATIO
    } else {
        icon_size
    };

    div()
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .size(px(box_size))
        .rounded_lg()
        .bg(background)
        .child(icon_element(entry, icon_size))
        .child(
            div()
                .absolute()
                .bottom_2()
                .right_2()
                .min_w(px(16.0))
                .h(px(16.0))
                .flex()
                .items_center()
                .justify_center()
                .px_1()
                .rounded_md()
                .bg(theme.secondary)
                .text_xs()
                .text_color(theme.foreground)
                .child(workspace_label(entry.workspace)),
        )
}

fn workspace_label(workspace: i32) -> String {
    if workspace < 0 {
        "S".to_string()
    } else {
        workspace.to_string()
    }
}

fn arrow_box(
    left: bool,
    box_size: f32,
    icon_size: f32,
    cx: &Context<SwitcherView>,
) -> impl IntoElement {
    let theme = cx.theme();
    let icon = if left {
        IconName::ChevronLeft
    } else {
        IconName::ChevronRight
    };
    div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(box_size))
        .rounded_lg()
        .child(
            Icon::new(icon)
                .size(px(icon_size))
                .text_color(theme.foreground),
        )
}

fn icon_element(entry: &Entry, size: f32) -> impl IntoElement {
    let size = px(size);
    match &entry.icon {
        Some(path) => img(path.clone()).w(size).h(size).into_any_element(),
        None => div().size(size).into_any_element(),
    }
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{truncated}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item_indices(slots: &[Slot]) -> Vec<usize> {
        slots
            .iter()
            .filter_map(|s| match s {
                Slot::Item(i) => Some(*i),
                _ => None,
            })
            .collect()
    }

    fn has_left(slots: &[Slot]) -> bool {
        matches!(slots.first(), Some(Slot::ArrowLeft))
    }

    fn has_right(slots: &[Slot]) -> bool {
        matches!(slots.last(), Some(Slot::ArrowRight))
    }

    #[test]
    fn shows_everything_when_it_fits() {
        let slots = paged_slots(4, 2, 8);
        assert_eq!(item_indices(&slots), [0, 1, 2, 3]);
        assert!(!has_left(&slots) && !has_right(&slots));
    }

    #[test]
    fn right_arrow_only_near_left_edge() {
        let slots = paged_slots(20, 0, 5);
        assert_eq!(slots.len(), 5);
        assert!(!has_left(&slots) && has_right(&slots));
        assert!(item_indices(&slots).contains(&0));
    }

    #[test]
    fn left_arrow_only_near_right_edge() {
        let slots = paged_slots(20, 19, 5);
        assert_eq!(slots.len(), 5);
        assert!(has_left(&slots) && !has_right(&slots));
        assert!(item_indices(&slots).contains(&19));
    }

    #[test]
    fn both_arrows_in_the_middle() {
        let slots = paged_slots(20, 10, 5);
        assert_eq!(slots.len(), 5);
        assert!(has_left(&slots) && has_right(&slots));
        assert!(item_indices(&slots).contains(&10));
    }

    #[test]
    fn selection_stays_centered_while_hidden_both_sides() {
        let capacity = 7;
        let center = (capacity - 1) / 2;
        for selected in center..(20 - center - 1) {
            let slots = paged_slots(20, selected, capacity);
            let pos = slots
                .iter()
                .position(|s| matches!(s, Slot::Item(i) if *i == selected))
                .unwrap();
            assert_eq!(pos, center, "selected {selected} should be centered");
        }
    }

    #[test]
    fn clip_truncates_with_ellipsis() {
        assert_eq!(clip("abcdef", 4), "abc…");
    }

    #[test]
    fn special_workspaces_show_s() {
        assert_eq!(workspace_label(3), "3");
        assert_eq!(workspace_label(-98), "S");
    }

    #[test]
    fn box_size_shrinks_with_count_down_to_min() {
        assert_eq!(box_size(2, 2000.0), MAX_BOX);
        assert_eq!(box_size(500, 2000.0), MIN_BOX);
    }
}
