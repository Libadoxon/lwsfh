use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::cli::Command;
use crate::daemon::Event;

fn socket_path() -> PathBuf {
    match std::env::var("XDG_RUNTIME_DIR") {
        Ok(dir) => PathBuf::from(dir).join("lwsfh.sock"),
        Err(_) => {
            let user = std::env::var("USER")
                .expect("neither XDG_RUNTIME_DIR nor USER is set; cannot pick a socket path");
            tracing::warn!(
                "XDG_RUNTIME_DIR is not set; falling back to /tmp. Set XDG_RUNTIME_DIR to a per-user runtime dir to silence this."
            );
            PathBuf::from("/tmp").join(format!("lwsfh-{user}.sock"))
        }
    }
}

pub fn send_command(cmd: &str) -> bool {
    match UnixStream::connect(socket_path()) {
        Ok(mut stream) => stream.write_all(cmd.as_bytes()).is_ok(),
        Err(_) => false,
    }
}

pub fn daemon_running() -> bool {
    UnixStream::connect(socket_path()).is_ok()
}

pub fn serve(events: flume::Sender<Event>) -> Result<()> {
    let path = socket_path();
    if let Err(e) = std::fs::remove_file(&path)
        && e.kind() != ErrorKind::NotFound
    {
        return Err(e)
            .with_context(|| format!("failed to remove stale socket at {}", path.display()));
    }
    let listener = UnixListener::bind(&path)
        .with_context(|| format!("failed to bind socket at {}", path.display()))?;

    std::thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            let mut conn = conn;
            let mut buf = String::new();
            if conn.read_to_string(&mut buf).is_ok()
                && let Some(cmd) = Command::parse(buf.trim())
                && events.send(Event::Command(cmd)).is_err()
            {
                break;
            }
        }
    });

    Ok(())
}

pub fn cleanup() {
    let _ = std::fs::remove_file(socket_path());
}
