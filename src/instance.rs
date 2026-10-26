//! Single-instance control socket in `$XDG_RUNTIME_DIR/tuple-ui.sock`.
//!
//! A second `tuple-ui --toggle-mute` (or a compositor keybinding) sends one
//! line to the running instance instead of starting another one.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use async_channel::Receiver;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;


pub fn default_socket_path() -> PathBuf {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join(format!("tuple-ui-{}", uid())));
    runtime.join("tuple-ui.sock")
}

fn uid() -> u32 {
    std::fs::metadata("/proc/self").map(|m| std::os::unix::fs::MetadataExt::uid(&m)).unwrap_or(0)
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
pub fn listen(socket_path: PathBuf, runtime: &tokio::runtime::Handle) -> Receiver<String> {
    let (sender, receiver) = async_channel::bounded(16);
    runtime.spawn(async move {
        if socket_path.exists() && UnixStream::connect(&socket_path).is_err() {
            let _ = std::fs::remove_file(&socket_path);
        }
        if let Some(parent) = socket_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let listener = match UnixListener::bind(&socket_path) {
            Ok(listener) => listener,
            Err(err) => {
                eprintln!("tuple-ui: cannot listen on {}: {err}", socket_path.display());
                return;
            }
        };
        while let Ok((stream, _addr)) = listener.accept().await {
            let (reader, mut writer) = stream.into_split();
            let mut lines = BufReader::new(reader).lines();
            if let Ok(Some(line)) = lines.next_line().await {
                let command = line.trim().to_string();
                if !command.is_empty() {
                    let _ = writer.write_all(b"ok\n").await;
                    if sender.send(command).await.is_err() {
                        break;
                    }
                }
            }
        }
    });
    receiver
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn socket_delivers_commands_to_the_listener() {
        let dir = std::env::temp_dir().join(format!("tuple-ui-instance-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("tuple-ui.sock");
        assert!(!send_to_running_instance("show", &path));

        let commands = listen(path.clone(), &tokio::runtime::Handle::current());
        let path_for_client = path.clone();
        let sender = tokio::task::spawn_blocking(move || {
            let mut sent = false;
            for _ in 0..50 {
                if send_to_running_instance("toggle-mute", &path_for_client) {
                    sent = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            assert!(sent);
            assert!(send_to_running_instance("show", &path_for_client));
        });
        let first = tokio::time::timeout(Duration::from_secs(5), commands.recv()).await.unwrap().unwrap();
        assert_eq!(first, "toggle-mute");
        let second = tokio::time::timeout(Duration::from_secs(5), commands.recv()).await.unwrap().unwrap();
        assert_eq!(second, "show");
        sender.await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
