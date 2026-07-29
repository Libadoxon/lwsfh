use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

// Request a large icon and let GPUI downscale it to the render size; upscaling a smaller
// source (and HiDPI output) is what makes icons look blurry.
const ICON_SIZE: u16 = 256;

fn cache() -> &'static Mutex<HashMap<String, Option<PathBuf>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<PathBuf>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn configured_theme() -> Option<&'static str> {
    static THEME: OnceLock<Option<String>> = OnceLock::new();
    THEME
        .get_or_init(|| {
            if let Some(theme) = gtk_icon_theme() {
                tracing::info!(theme = %theme, "using GTK icon theme");
                Some(theme)
            } else if let Some(theme) = kde_icon_theme() {
                tracing::info!(theme = %theme, "using KDE icon theme");
                Some(theme)
            } else {
                tracing::info!("no GTK or KDE icon theme configured; using freedesktop defaults");
                None
            }
        })
        .as_deref()
}

fn gtk_icon_theme() -> Option<String> {
    for rel in ["gtk-4.0/settings.ini", "gtk-3.0/settings.ini"] {
        let path = dirs::config_dir()?.join(rel);
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        if let Some(theme) = content
            .lines()
            .find_map(|l| l.trim().strip_prefix("gtk-icon-theme-name="))
        {
            return Some(theme.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn kde_icon_theme() -> Option<String> {
    let content = std::fs::read_to_string(dirs::config_dir()?.join("kdeglobals")).ok()?;
    let mut in_icons = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_icons = line == "[Icons]";
        } else if in_icons && let Some(theme) = line.strip_prefix("Theme=") {
            return Some(theme.to_string());
        }
    }
    None
}

pub fn resolve(class: &str) -> Option<PathBuf> {
    if let Some(cached) = cache().lock().unwrap().get(class) {
        return cached.clone();
    }
    let resolved = lookup(class);
    cache()
        .lock()
        .unwrap()
        .insert(class.to_string(), resolved.clone());
    resolved
}

fn lookup(class: &str) -> Option<PathBuf> {
    candidates(class).iter().find_map(|name| lookup_name(name))
}

fn candidates(class: &str) -> Vec<String> {
    let mut names = vec![class.to_string(), class.to_lowercase()];
    if let Some(last) = class.rsplit('.').next().filter(|s| *s != class) {
        names.push(last.to_string());
        names.push(last.to_lowercase());
    }
    names.dedup();
    names
}

fn lookup_name(name: &str) -> Option<PathBuf> {
    let mut lookup = freedesktop_icons::lookup(name)
        .with_size(ICON_SIZE)
        .force_svg()
        .with_cache();
    if let Some(theme) = configured_theme() {
        lookup = lookup.with_theme(theme);
    }
    lookup.find()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_include_reverse_dns_tail() {
        assert_eq!(
            candidates("org.kde.Dolphin"),
            ["org.kde.Dolphin", "org.kde.dolphin", "Dolphin", "dolphin"]
        );
    }

    #[test]
    fn candidates_without_dots() {
        assert_eq!(candidates("Alacritty"), ["Alacritty", "alacritty"]);
    }
}
