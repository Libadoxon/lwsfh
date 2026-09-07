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

/// Build the caches that back [`resolve`] — a filesystem scan of the desktop-entry
/// database and icon-theme detection — ahead of time, off-thread, so the first switch
/// doesn't pay for them.
pub fn prewarm() {
    std::thread::spawn(|| {
        configured_theme();
        desktop_icons();
    });
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
    // A window's class often isn't the icon's name (e.g. class "Spotify" ships icon
    // "spotify-client"). The .desktop entry's `Icon=` key is the authoritative mapping,
    // so try it before falling back to guessing from the class.
    desktop_icon(class)
        .into_iter()
        .chain(candidates(class))
        .find_map(|name| lookup_name(&name))
}

fn desktop_icon(class: &str) -> Option<String> {
    desktop_icons().get(&class.to_lowercase()).cloned()
}

fn desktop_icons() -> &'static HashMap<String, String> {
    static MAP: OnceLock<HashMap<String, String>> = OnceLock::new();
    MAP.get_or_init(build_desktop_icon_map)
}

fn build_desktop_icon_map() -> HashMap<String, String> {
    let mut map = HashMap::new();
    for dir in application_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Some(icon) = desktop_entry_field(&content, "Icon") else {
                continue;
            };
            // Match the window class against both the desktop file's stem and its
            // StartupWMClass. Earlier data dirs win, per XDG precedence.
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                map.entry(stem.to_lowercase())
                    .or_insert_with(|| icon.clone());
            }
            if let Some(wm) = desktop_entry_field(&content, "StartupWMClass") {
                map.entry(wm.to_lowercase()).or_insert(icon);
            }
        }
    }
    map
}

fn desktop_entry_field(content: &str, key: &str) -> Option<String> {
    let mut in_entry = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
        } else if in_entry
            && let Some(value) = line
                .strip_prefix(key)
                .map(str::trim_start)
                .and_then(|r| r.strip_prefix('='))
        {
            return Some(value.trim().to_string());
        }
    }
    None
}

fn application_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = dirs::data_dir().into_iter().collect();
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());
    dirs.extend(
        data_dirs
            .split(':')
            .filter(|s| !s.is_empty())
            .map(PathBuf::from),
    );
    dirs.into_iter().map(|d| d.join("applications")).collect()
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

    #[test]
    fn desktop_entry_field_reads_from_entry_group_only() {
        let content = "[Desktop Entry]\nName=Spotify\nIcon=spotify-client\nStartupWMClass=spotify\n\n[Desktop Action Next]\nIcon=media-skip-forward\n";
        assert_eq!(
            desktop_entry_field(content, "Icon").as_deref(),
            Some("spotify-client")
        );
        assert_eq!(
            desktop_entry_field(content, "StartupWMClass").as_deref(),
            Some("spotify")
        );
        assert_eq!(desktop_entry_field(content, "Exec"), None);
    }

    #[test]
    fn desktop_entry_field_ignores_localized_keys() {
        let content = "[Desktop Entry]\nIcon[de]=falsch\nIcon=firefox\n";
        assert_eq!(
            desktop_entry_field(content, "Icon").as_deref(),
            Some("firefox")
        );
    }
}
