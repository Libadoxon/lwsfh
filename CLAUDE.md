# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

`lwsfh` is a configurable, fast, stylish file explorer written in Rust on top of [GPUI](https://github.com/zed-industries/zed) (the Zed editor's UI framework) and [gpui-component](https://github.com/longbridge/gpui-component). Both are pulled as git dependencies, so changes there flow in directly and pinned commits live in `Cargo.lock`.

The crate uses Rust edition 2024 — pinned via `rust-toolchain.toml` (channel `stable`).

## Development environment

The project is built and tested through Nix. `.envrc` runs `use flake`, so direnv drops you into a shell with the right toolchain, `rust-analyzer`, and the native libs GPUI needs (Wayland, Vulkan loader, fontconfig, libxkbcommon). `LD_LIBRARY_PATH` is set inside that shell — running `cargo run` outside of it will likely fail to load Vulkan/Wayland at runtime.

## Common commands

Inside the dev shell:

- `cargo run` — launch the app. Themes are read from `$XDG_CONFIG_HOME/lwsfh/themes/` (auto-created on first launch); drop JSON theme files there.
- `cargo build` / `cargo build --release`
- `cargo clippy --all-targets -- --deny warnings` — same invocation CI runs.
- `cargo nextest run` — test runner used by CI (plain `cargo test` also works).
- `cargo nextest run <test_name>` — run a single test by substring match.
- `nix fmt` — format Rust (`rustfmt`, edition 2024), TOML (`taplo`, with `dependencies` reordered only in `Cargo.toml`), and Nix (`nixfmt --strict`). Configured in `treefmt.nix`.
- `nix flake check` — runs the full CI matrix locally: build, formatting check, clippy (deny warnings), `cargo-audit`, `cargo-nextest`.
- `nix build` — build the wrapped binary (sets `LD_LIBRARY_PATH` via `postFixup` so the resulting `./result/bin/lwsfh` runs standalone).

CI (`.github/workflows/check-nix.yml`) only runs `nix build` and `nix flake check`, so `nix flake check` locally reproduces CI exactly.

`.cargo/audit.toml` disables the yanked-crate check and crates.io index updates because the sandboxed Nix build has no network — keep that in mind if adjusting audit behavior.

## Architecture notes

The codebase is still small (`src/main.rs` is the only module at present), so most "architecture" here is about the framework conventions rather than internal layering:

- **GPUI app entry**: `gpui_platform::application()` is constructed with `assets::Assets` (defined in `src/assets.rs` — a `RustEmbed` source over `./assets/icons/**.svg`). `gpui_component::init(cx)` must be called before any component is used.
- **Root view**: every window's top-level view must be wrapped in `gpui_component::Root::new(view, window, cx)` — `main.rs` shows the pattern. Do not put a bare view at the window root.
- **Theme loading**: `assets::init_themes("…", cx)` ensures `$XDG_CONFIG_HOME/lwsfh/themes/` exists and registers `ThemeRegistry::watch_dir` against it. The callback re-applies the named theme via `Theme::global_mut(cx).apply_config(&theme)` and forces `font_family = "Inter"` (the gpui-component default `.SystemUIFont` lacks bold faces on Linux). New themes dropped into that directory are picked up live.
- **Bundled icons**: `src/assets.rs` defines an in-crate `IconName` enum + `IconNamed` impl pointing each variant at `icons/<file>.svg`. Add a variant when a new SVG lands under `./assets/icons/`.
- **Layout**: GPUI uses a Tailwind-inspired fluent builder API (`div().p_4().size_full().child(...)`). Resizable splits come from `gpui_component::resizable::{h_resizable, resizable_panel}` with explicit `size()` and `size_range()` per panel.
