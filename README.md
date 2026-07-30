# lwsfh

**L**ibadoxon's **w**indow **s**witcher **f**or **H**yprland - a fast, modern, macOS-inspired Alt-Tab window switcher for [Hyprland](https://hyprland.org/).

Built in Rust on top of [GPUI](https://github.com/zed-industries/zed) (the Zed editor's UI framework) and [gpui-component](https://github.com/longbridge/gpui-component). It renders as a Wayland layer-shell overlay that grabs the keyboard, cycles through your windows in most-recently-used order, and focuses your pick when you release `SUPER`.

## Features

- **Instant** - a warm daemon keeps the overlay ready, so it appears with no cold-start lag.
- **MRU ordering** - windows are sorted by focus history.
- **Hold-and-release** — hold `SUPER`, tap `Tab` to cycle, release to confirm. Feels like macOS ⌘-Tab.
- **Themeable** - Ships with two style options (--macos flag to get the second style variant) as well as a --theme flag to completely customize colors, see ./themes for examples
- **App icons** - resolves icons from your GTK/KDE icon theme via freedesktop lookups.

## Hyprland setup

Add the following to your Hyprland config.
```lua
hl.exec_once("lwsfh")
hl.bind("SUPER + Tab", (hl.dsp.exec_cmd("lwsfh go")), {
  ["repeating"] = true
})
```

Enable blurring add:
```lua
hl.layer_rule({
  ["blur"] = true,
  ["ignore_alpha"] = 0.1,
  ["match"] = {
    ["namespace"] = "lwsfh"
  }
})
```

## Usage

```
Usage: lwsfh [OPTIONS] [COMMAND]

Commands:
  go    Open the switcher (or advance the selection if it is already open)
  quit  Quit the daemon
  help  Print this message or the help of the given subcommand(s)

Options:
      --macos         Use a macOS-style look
      --theme <PATH>  Path to a theme JSON file or either "dark" or "light", see repo for exampl
e theme files
  -h, --help          Print help
  -V, --version       Print version
```

## Disclaimer on AI use
This project has been largely written with the help of [Claude Code](https://claude.ai/)

## License

Licensed under the [GNU General Public License v3.0](LICENSE).
