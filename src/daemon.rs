use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use anyhow::Result;
use gpui::{App, AsyncApp, QuitMode, WindowAppearance};
use gpui_component::theme::{Theme, ThemeConfig, ThemeMode, ThemeSet};
use gpui_platform::application;

use crate::cli::Command;
use crate::overlay::{self, Overlay};
use crate::view::{self, Entry};
use crate::{hyprland, ipc};

pub enum Event {
    Command(Command),
    Confirm(String),
    Cancel,
}

pub fn run(macos: bool, theme: Option<PathBuf>) -> Result<()> {
    init_logging();

    if ipc::daemon_running() {
        anyhow::bail!("another lwsfh daemon is already running");
    }

    let (events_tx, events_rx) = flume::unbounded::<Event>();
    ipc::serve(events_tx.clone())?;

    application()
        .with_assets(gpui_component_assets::Assets)
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx| {
            gpui_component::init(cx);
            view::init(cx);
            apply_theme(theme.as_deref(), cx);

            cx.spawn(async move |cx: &mut AsyncApp| {
                run_loop(events_rx, events_tx, macos, cx).await;
                ipc::cleanup();
            })
            .detach();
        });

    Ok(())
}

async fn run_loop(
    rx: flume::Receiver<Event>,
    events: flume::Sender<Event>,
    macos: bool,
    cx: &mut AsyncApp,
) {
    let mut state = WindowState::new(events, macos);
    while let Ok(event) = rx.recv_async().await {
        match event {
            Event::Command(Command::Go { reverse }) => state.go(reverse, cx),
            Event::Command(Command::Quit) => {
                state.close(cx);
                cx.update(|cx| cx.quit());
                return;
            }
            Event::Confirm(address) => state.confirm(address, cx).await,
            Event::Cancel => state.close(cx),
        }
    }
}

struct WindowState {
    overlay: Option<Overlay>,
    events: flume::Sender<Event>,
    macos: bool,
}

impl WindowState {
    fn new(events: flume::Sender<Event>, macos: bool) -> Self {
        Self {
            overlay: None,
            events,
            macos,
        }
    }

    fn go(&mut self, reverse: bool, cx: &mut AsyncApp) {
        if let Some(overlay) = &self.overlay {
            let _ = overlay.view.update(cx, |view, cx| view.cycle(!reverse, cx));
        } else {
            self.show(reverse, cx);
        }
    }

    fn show(&mut self, reverse: bool, cx: &mut AsyncApp) {
        let windows = match hyprland::list_windows() {
            Ok(windows) => windows,
            Err(e) => {
                tracing::warn!(%e, "failed to list windows");
                return;
            }
        };
        if windows.len() < 2 {
            tracing::debug!(count = windows.len(), "not enough windows to switch");
            return;
        }

        let entries: Vec<Entry> = windows.iter().map(Entry::from_window).collect();
        let selected = if reverse { entries.len() - 1 } else { 1 };

        let macos = self.macos;
        match cx.update(|cx| overlay::open(entries, selected, macos, self.events.clone(), cx)) {
            Ok(overlay) => self.overlay = Some(overlay),
            Err(e) => tracing::error!(%e, "failed to open overlay"),
        }
    }

    fn close(&mut self, cx: &mut AsyncApp) {
        if let Some(overlay) = self.overlay.take() {
            cx.update(|cx| overlay::close(&overlay.window, cx));
        }
    }

    async fn confirm(&mut self, address: String, cx: &mut AsyncApp) {
        self.close(cx);
        // Focus only after the exclusive layer surface is torn down; while it holds the
        // keyboard grab, focusing the target window does not take effect.
        cx.background_executor()
            .timer(Duration::from_millis(60))
            .await;
        if let Err(e) = hyprland::focus_window(&address) {
            tracing::warn!(%e, "failed to focus window");
        }
    }
}

/// Apply the theme: `dark`/`light` select the built-in mode, any other value is a path to a
/// theme JSON file, and no value falls back to the system appearance.
fn apply_theme(theme: Option<&Path>, cx: &mut App) {
    match theme.and_then(|p| p.to_str()) {
        Some("dark") => Theme::change(ThemeMode::Dark, None, cx),
        Some("light") => Theme::change(ThemeMode::Light, None, cx),
        Some(_) => match theme.and_then(load_theme) {
            Some(config) => Theme::global_mut(cx).apply_config(&Rc::new(config)),
            None => Theme::change(system_mode(cx), None, cx),
        },
        None => Theme::change(system_mode(cx), None, cx),
    }
}

fn system_mode(cx: &App) -> ThemeMode {
    match cx.window_appearance() {
        WindowAppearance::Dark | WindowAppearance::VibrantDark => ThemeMode::Dark,
        WindowAppearance::Light | WindowAppearance::VibrantLight => ThemeMode::Light,
    }
}

fn load_theme(path: &Path) -> Option<ThemeConfig> {
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(e) => {
            tracing::error!(%e, path = %path.display(), "failed to read theme file");
            return None;
        }
    };
    let set: ThemeSet = match serde_json::from_str(&contents) {
        Ok(set) => set,
        Err(e) => {
            tracing::error!(%e, "failed to parse theme file");
            return None;
        }
    };
    let mut themes = set.themes;
    if themes.is_empty() {
        tracing::error!("theme file contains no themes");
        return None;
    }
    let index = themes.iter().position(|t| t.is_default).unwrap_or(0);
    Some(themes.swap_remove(index))
}

fn init_logging() {
    use tracing_subscriber::{EnvFilter, fmt, prelude::*};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("lwsfh=info"));
    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false))
        .with(filter)
        .init();
}
