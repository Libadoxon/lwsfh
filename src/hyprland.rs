use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub address: String,
    pub class: String,
    pub title: String,
    pub workspace: i32,
    pub focus_history_id: i32,
}

impl Window {
    pub fn display_title(&self) -> &str {
        if self.title.is_empty() {
            &self.class
        } else {
            &self.title
        }
    }
}

fn socket_path() -> Result<PathBuf> {
    let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
        .context("HYPRLAND_INSTANCE_SIGNATURE not set (is Hyprland running?)")?;
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    Ok(PathBuf::from(runtime)
        .join("hypr")
        .join(sig)
        .join(".socket.sock"))
}

fn request(cmd: &str) -> Result<String> {
    let path = socket_path()?;
    let mut stream = UnixStream::connect(&path)
        .with_context(|| format!("failed to connect to Hyprland at {}", path.display()))?;
    stream.write_all(cmd.as_bytes())?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

pub fn list_windows() -> Result<Vec<Window>> {
    let json = request("j/clients")?;
    let clients: Vec<Client> =
        serde_json::from_str(&json).context("failed to parse Hyprland j/clients JSON")?;
    Ok(order(clients))
}

pub fn focus_window(address: &str) -> Result<()> {
    request(&format!(
        "dispatch hl.dsp.focus({{ window = \"address:{address}\" }})"
    ))?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct Client {
    address: String,
    class: String,
    title: String,
    workspace: ClientWorkspace,
    /// 0 is the currently focused window; larger values are further back in focus history.
    #[serde(rename = "focusHistoryID")]
    focus_history_id: i32,
    #[serde(default)]
    mapped: bool,
    #[serde(default)]
    hidden: bool,
}

#[derive(Debug, Deserialize)]
struct ClientWorkspace {
    id: i32,
}

fn order(clients: Vec<Client>) -> Vec<Window> {
    let mut windows: Vec<Window> = clients
        .into_iter()
        .filter(|c| {
            c.mapped && !c.hidden && !c.class.is_empty() && !c.class.eq_ignore_ascii_case("lwsfh")
        })
        .map(|c| Window {
            address: c.address,
            class: c.class,
            title: c.title,
            workspace: c.workspace.id,
            focus_history_id: c.focus_history_id,
        })
        .collect();
    windows.sort_by_key(|w| w.focus_history_id);
    windows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(class: &str, mapped: bool, hidden: bool, fhid: i32) -> Client {
        Client {
            address: format!("0x{class}"),
            class: class.to_string(),
            title: format!("{class} title"),
            workspace: ClientWorkspace { id: 1 },
            focus_history_id: fhid,
            mapped,
            hidden,
        }
    }

    #[test]
    fn orders_by_focus_history_and_filters() {
        let clients = vec![
            client("firefox", true, false, 2),
            client("alacritty", true, false, 0),
            client("hidden-app", true, true, 1),
            client("unmapped-app", false, false, 3),
            client("", true, false, 4),
            client("lwsfh", true, false, 5),
            client("code", true, false, 1),
        ];

        let classes: Vec<String> = order(clients).into_iter().map(|w| w.class).collect();
        assert_eq!(classes, ["alacritty", "code", "firefox"]);
    }

    #[test]
    fn display_title_falls_back_to_class() {
        let w = Window {
            address: "0x1".into(),
            class: "Alacritty".into(),
            title: String::new(),
            workspace: 1,
            focus_history_id: 0,
        };
        assert_eq!(w.display_title(), "Alacritty");
    }
}
