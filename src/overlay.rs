use anyhow::Result;
use gpui::{
    App, AppContext, Bounds, Entity, WindowBackgroundAppearance, WindowBounds, WindowHandle,
    WindowKind, WindowOptions,
    layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions},
    point, px, size,
};

use crate::daemon::Event;
use crate::view::{Entry, SwitcherView};

pub struct Overlay {
    pub window: WindowHandle<SwitcherView>,
    pub view: Entity<SwitcherView>,
}

pub fn open(
    entries: Vec<Entry>,
    selected: usize,
    macos: bool,
    events: flume::Sender<Event>,
    cx: &mut App,
) -> Result<Overlay> {
    // A zero size makes gpui call `layer_surface.set_size(0, 0)`; anchored to all edges, the
    // compositor then sizes the surface to the output (a non-zero size would override that and
    // ignore the monitor's real dimensions and scale).
    let bounds = Bounds {
        origin: point(px(0.0), px(0.0)),
        size: size(px(0.0), px(0.0)),
    };

    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        focus: true,
        show: true,
        app_id: Some("lwsfh".to_string()),
        window_background: WindowBackgroundAppearance::Transparent,
        kind: WindowKind::LayerShell(LayerShellOptions {
            namespace: "lwsfh".to_string(),
            layer: Layer::Overlay,
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            keyboard_interactivity: KeyboardInteractivity::Exclusive,
            ..Default::default()
        }),
        ..Default::default()
    };

    let view_cell = std::cell::RefCell::new(None);
    let window = cx.open_window(options, |window, cx| {
        let view = cx.new(|cx| SwitcherView::new(entries, selected, macos, events, cx));
        let handle = view.read(cx).focus_handle();
        window.focus(&handle, cx);
        *view_cell.borrow_mut() = Some(view.clone());
        view
    })?;

    window
        .update(cx, |_, window, _| window.activate_window())
        .ok();

    let view = view_cell
        .into_inner()
        .expect("open_window builder always runs");

    Ok(Overlay { window, view })
}

pub fn close(window: &WindowHandle<SwitcherView>, cx: &mut App) {
    let _ = window.update(cx, |_, window, _| window.remove_window());
}
