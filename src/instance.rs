//! Single-instance control socket in `$XDG_RUNTIME_DIR/tuple-ui.sock`.
//!
//! A second `tuple-ui --toggle-mute` (or a compositor keybinding) sends one
//! line to the running instance instead of starting another one.

use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_channel::Receiver;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::Semaphore;

/// A command is one short line; anything longer is dropped.
const MAX_COMMAND_LEN: u64 = 128;
/// How long a client gets to send its line before it is dropped.
const READ_DEADLINE: Duration = Duration::from_secs(2);
const MAX_CLIENTS: usize = 8;

/// `$XDG_RUNTIME_DIR/tuple-ui.sock`, or a private `~/.cache/tuple-ui` directory when the
/// session has no runtime dir (never a shared, predictable `/tmp` path).
pub fn default_socket_path() -> PathBuf {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).filter(|p| p.is_dir());
    let dir = runtime.unwrap_or_else(|| {
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
        home.join(".cache").join("tuple-ui")
    });
    dir.join("tuple-ui.sock")
}

fn ensure_private_dir(dir: &std::path::Path) -> std::io::Result<()> {
    if !dir.exists() {
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    }
    Ok(())
}

/// Deliver `command` to a running instance. Returns false when none is listening.
pub fn send_to_running_instance(command: &str, socket_path: &PathBuf) -> bool {
    let Ok(mut stream) = UnixStream::connect(socket_path) else { return false };
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    if stream.write_all(format!("{command}\n").as_bytes()).is_err() {
        return false;
    }
    let mut ack = [0u8; 8];
    let _ = stream.read(&mut ack);
    true
}

/// Listen on the socket forever (on `runtime`), delivering each received command line.
/// Clients are served concurrently with a read deadline and a length cap, so one idle
/// connection cannot block the others.
pub fn listen(socket_path: PathBuf, runtime: &tokio::runtime::Handle) -> Receiver<String> {
    let (sender, receiver) = async_channel::bounded(16);
    runtime.spawn(async move {
        if socket_path.exists() && UnixStream::connect(&socket_path).is_err() {
            let _ = std::fs::remove_file(&socket_path);
        }
        if let Some(parent) = socket_path.parent() {
            if let Err(err) = ensure_private_dir(parent) {
                eprintln!("tuple-ui: cannot create {}: {err}", parent.display());
                return;
            }
        }
        let listener = match UnixListener::bind(&socket_path) {
            Ok(listener) => listener,
            Err(err) => {
                eprintln!("tuple-ui: cannot listen on {}: {err}", socket_path.display());
                return;
            }
        };
        let _ = std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600));
        let clients = Arc::new(Semaphore::new(MAX_CLIENTS));
        while let Ok((stream, _addr)) = listener.accept().await {
            let Ok(permit) = clients.clone().acquire_owned().await else { break };
            let sender = sender.clone();
            tokio::spawn(async move {
                let _permit = permit;
                let (reader, mut writer) = stream.into_split();
                let mut line = String::new();
                let mut limited = BufReader::new(reader.take(MAX_COMMAND_LEN));
                if tokio::time::timeout(READ_DEADLINE, limited.read_line(&mut line)).await.is_err() {
                    return;
                }
                let command = line.trim().to_string();
                if command.is_empty() || !line.ends_with('\n') {
                    return;
                }
                let _ = writer.write_all(b"ok\n").await;
                let _ = sender.send(command).await;
            });
        }
    });
    receiver
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_socket(name: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("tuple-ui-instance-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        (dir.clone(), dir.join("tuple-ui.sock"))
    }

    async fn recv(commands: &Receiver<String>) -> String {
        tokio::time::timeout(Duration::from_secs(5), commands.recv()).await.unwrap().unwrap()
    }

    #[tokio::test]
    async fn socket_delivers_commands_to_the_listener() {
        let (dir, path) = fresh_socket("basic");
        assert!(!send_to_running_instance("show", &path));

        let commands = listen(path.clone(), &tokio::runtime::Handle::current());
        let client_path = path.clone();
        let sender = tokio::task::spawn_blocking(move || {
            let mut sent = false;
            for _ in 0..50 {
                if send_to_running_instance("toggle-mute", &client_path) {
                    sent = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            assert!(sent);
            assert!(send_to_running_instance("show", &client_path));
        });
        assert_eq!(recv(&commands).await, "toggle-mute");
        assert_eq!(recv(&commands).await, "show");
        sender.await.unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn an_idle_client_does_not_block_others_and_long_lines_are_dropped() {
        let (dir, path) = fresh_socket("idle");
        let commands = listen(path.clone(), &tokio::runtime::Handle::current());
        let mut idle = None;
        for _ in 0..50 {
            if let Ok(stream) = UnixStream::connect(&path) {
                idle = Some(stream);
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let idle = idle.expect("listener up"); // connected, never sends a line

        let client_path = path.clone();
        tokio::task::spawn_blocking(move || {
            let mut flood = UnixStream::connect(&client_path).unwrap();
            let _ = flood.write_all(&vec![b'a'; 4096]); // no newline, far too long
            assert!(send_to_running_instance("toggle-mute", &client_path));
        })
        .await
        .unwrap();
        assert_eq!(recv(&commands).await, "toggle-mute");
        assert!(tokio::time::timeout(Duration::from_millis(300), commands.recv()).await.is_err());
        drop(idle);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
