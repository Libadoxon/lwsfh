# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

`lwsfh` ("Libadoxon's window switcher for Hyprland") is a fast, stylish Alt-Tab–style window switcher for [Hyprland](https://hyprland.org/), written in Rust on top of [GPUI](https://github.com/zed-industries/zed) (the Zed editor's UI framework) and [gpui-component](https://github.com/longbridge/gpui-component). Both are pulled as git dependencies, so changes there flow in directly and pinned commits live in `Cargo.lock`.

The crate uses Rust edition 2024 — pinned via `rust-toolchain.toml` (channel `stable`).

## How it works

A single binary runs in two modes:

- **Daemon** (`lwsfh` with no subcommand): a long-running GPUI application. It stays warm so the overlay appears instantly, listens on a Unix socket for commands, and hosts the switcher overlay.
- **Client** (`lwsfh go [--reverse]`, `lwsfh quit`): a thin invocation that writes one line to the daemon's socket. Hyprland keybinds exec these.

Interaction: bind `SUPER+TAB` to `lwsfh go`. The first press opens a Wayland **layer-shell overlay** with `KeyboardInteractivity::Exclusive`, so it grabs the keyboard. From then on Tab / Shift-Tab cycle the selection **in-process** (they no longer reach Hyprland binds), and **releasing SUPER** confirms and focuses the selected window. So there is exactly one client spawn per switching session.

Window order is Hyprland's own MRU: `j/clients` returns each window's `focusHistoryID` (0 = current), and the list is sorted by it. The overlay pre-selects index 1 (the previously focused window).

Suggested Hyprland config:

```ini
exec-once = lwsfh
bind = SUPER, TAB, exec, lwsfh go
bind = SUPER SHIFT, TAB, exec, lwsfh go --reverse
```

## Development environment

The project is built and tested through Nix. `.envrc` runs `use flake`, so direnv drops you into a shell with the right toolchain, `rust-analyzer`, and the native libs GPUI needs (Wayland, Vulkan loader, fontconfig, libxkbcommon). `LD_LIBRARY_PATH` is set inside that shell — running `cargo run` outside of it will likely fail to load Vulkan/Wayland at runtime. Layer-shell also requires an actual Wayland compositor at runtime.

## Common commands

Inside the dev shell:

- `cargo run` — start the daemon. `cargo run -- go` / `cargo run -- quit` act as the client.
- `cargo build` / `cargo build --release`
- `cargo clippy --all-targets -- --deny warnings` — same invocation CI runs.
- `cargo nextest run` — test runner used by CI (plain `cargo test` also works).
- `cargo nextest run <test_name>` — run a single test by substring match.
- `nix fmt` — format Rust (`rustfmt`, edition 2024), TOML (`taplo`, with `dependencies` reordered only in `Cargo.toml`), and Nix (`nixfmt --strict`). Configured in `treefmt.nix`.
- `nix flake check` — runs the full CI matrix locally: build, formatting check, clippy (deny warnings), `cargo-audit`, `cargo-nextest`.
- `nix build` — build the wrapped binary (sets `LD_LIBRARY_PATH` via `postFixup` so the resulting `./result/bin/lwsfh` runs standalone).

CI (`.github/workflows/check-nix.yml`) only runs `nix build` and `nix flake check`, so `nix flake check` locally reproduces CI exactly.

`.cargo/audit.toml` disables the yanked-crate check and crates.io index updates because the sandboxed Nix build has no network — keep that in mind if adjusting audit behavior.

## Module layout (`src/`)

- `main.rs` — clap parse; dispatch daemon vs. client.
- `cli.rs` — the `Command` enum and the one-word socket protocol (`wire`/`parse`), plus the client that sends it.
- `ipc.rs` — the daemon's `$XDG_RUNTIME_DIR/lwsfh.sock` listener (a thread forwarding parsed commands onto a `flume` channel) and the client `send_command`.
- `hyprland.rs` — raw Hyprland IPC: `list_windows` (`j/clients`, filtered and sorted by `focusHistoryID`) and `focus_window`. Note: dispatchers use the current Lua syntax (`hl.dsp.focus({ window = "address:0x…" })`), not the old `focuswindow` form.
- `daemon.rs` — the GPUI `Application`, the `Event` enum, and the async event loop (`WindowState`) that shows/cycles/closes the overlay and focuses the target window after the overlay tears down.
- `overlay.rs` — creates the layer-shell window (`WindowKind::LayerShell`, `Layer::Overlay`, anchored to all edges, exclusive keyboard) wrapped in `gpui_component::Root`.
- `view.rs` — `SwitcherView`: the horizontal card row, keyboard actions (Tab/Shift-Tab/Enter/Escape via `actions!` + `KeyBinding`), and `on_modifiers_changed` to confirm on SUPER (`Modifiers.platform`) release.
- `icon.rs` — resolves app icons from window class via `freedesktop-icons`, honoring the GTK/KDE icon theme, with a cache.

## Framework conventions

- **GPUI app entry**: `gpui::Application::new().with_assets(gpui_component_assets::Assets).with_quit_mode(QuitMode::Explicit).run(...)`. Call `gpui_component::init(cx)` before using any component.
- **Root view**: a window's top-level view must be wrapped in `gpui_component::Root::new(view, window, cx)` — see `overlay.rs`. Do not put a bare view at the window root.
- **Layer shell**: this pinned gpui exposes `gpui::layer_shell::{LayerShellOptions, Layer, Anchor, KeyboardInteractivity}` and `WindowKind::LayerShell`.
- **Layout**: GPUI uses a Tailwind-inspired fluent builder API (`div().p_4().size_full().child(...)`); theme colors come from `cx.theme()` (`gpui_component::ActiveTheme`).
