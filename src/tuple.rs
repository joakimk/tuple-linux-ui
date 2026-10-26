//! Asynchronous wrapper around the `tuple` executable: one-shot commands and the
//! long-running `tuple state follow --raw` stream.

use std::process::Stdio;
use std::time::Duration;

use async_channel::{Receiver, Sender};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

pub const FOLLOW_ARGS: [&str; 5] = ["state", "follow", "--raw", "--format", "json"];
const RESTART_DELAYS_MS: [u64; 5] = [1000, 2000, 4000, 8000, 10000];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TupleCli {
    pub executable: String,
    pub env: Vec<(String, String)>,
}

impl Default for TupleCli {
    fn default() -> Self {
        TupleCli { executable: "tuple".into(), env: Vec::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliResult {
    pub args: Vec<String>,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub spawn_error: Option<String>,
}

impl CliResult {
    pub fn ok(&self) -> bool {
        self.exit_code == Some(0) && self.spawn_error.is_none()
    }

    pub fn json(&self) -> Option<Value> {
        serde_json::from_str(&self.stdout).ok()
    }

    /// The command line for messages, with secrets (the `auth` sign-in code) redacted.
    pub fn display_command(&self) -> String {
        format!("tuple {}", redact_args(&self.args).join(" "))
    }

    /// Best human-readable error text for a failed command. Never includes secrets.
    pub fn error(&self) -> String {
        if let Some(err) = &self.spawn_error {
            return format!("Could not start `{}`: {err}", self.display_command());
        }
        if let Some(message) = self.json().and_then(|v| v.get("error").and_then(Value::as_str).map(String::from)) {
            return redact_text(&self.args, message);
        }
        let text = if self.stderr.trim().is_empty() { &self.stdout } else { &self.stderr };
        match text.trim().lines().last() {
            Some(line) if !line.is_empty() => redact_text(&self.args, line.to_string()),
            _ => format!("`{}` failed ({:?})", self.display_command(), self.exit_code),
        }
    }
}

/// Arguments that must never appear in UI text or notifications.
fn secret_args(args: &[String]) -> Vec<&String> {
    match args.first().map(String::as_str) {
        Some("auth") => args.iter().skip(1).filter(|a| !a.starts_with("--")).collect(),
        _ => Vec::new(),
    }
}

fn redact_args(args: &[String]) -> Vec<String> {
    let secrets = secret_args(args);
    args.iter().map(|a| if secrets.contains(&a) { "••••".to_string() } else { a.clone() }).collect()
}

fn redact_text(args: &[String], mut text: String) -> String {
    for secret in secret_args(args) {
        if !secret.is_empty() {
            text = text.replace(secret.as_str(), "••••");
        }
    }
    text
}

/// One event from `tuple state follow --raw`, or a lifecycle notice about the stream itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FollowEvent {
    Event { kind: String, data: Value },
    /// The stream could not start at all (executable missing).
    Unavailable(String),
    /// The stream ended; it will be restarted after a delay.
    Ended { error: String, had_state: bool },
}

impl TupleCli {
    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(&self.executable);
        command.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        for (key, value) in &self.env {
            command.env(key, value);
        }
        command.kill_on_drop(true);
        command
    }

    /// Run `tuple <args>` and wait for it (30 s timeout).
    pub async fn run(&self, args: Vec<String>) -> CliResult {
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let child = self.command(&arg_refs).spawn();
        let mut result = CliResult { args: args.clone(), exit_code: None, stdout: String::new(), stderr: String::new(), spawn_error: None };
        let child = match child {
            Ok(child) => child,
            Err(err) => {
                result.spawn_error = Some(err.to_string());
                return result;
            }
        };
        match tokio::time::timeout(Duration::from_secs(30), child.wait_with_output()).await {
            Ok(Ok(output)) => {
                result.exit_code = output.status.code();
                result.stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                result.stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            }
            Ok(Err(err)) => result.spawn_error = Some(err.to_string()),
            Err(_) => result.spawn_error = Some("timed out after 30s".into()),
        }
        result
    }

    /// Stream `tuple state follow --raw` forever on `runtime`, restarting with backoff when it
    /// exits. Dropping the receiver stops the stream.
    pub fn follow(self, runtime: &tokio::runtime::Handle) -> Receiver<FollowEvent> {
        let (sender, receiver) = async_channel::bounded(100);
        runtime.spawn(async move {
            let mut restarts = 0usize;
            loop {
                let had_state = self.follow_once(&sender).await;
                if sender.is_closed() {
                    return;
                }
                if had_state {
                    restarts = 0;
                }
                let delay = RESTART_DELAYS_MS[restarts.min(RESTART_DELAYS_MS.len() - 1)];
                restarts += 1;
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
        });
        receiver
    }

    /// One run of the follow process. Returns whether any event was delivered.
    async fn follow_once(&self, output: &Sender<FollowEvent>) -> bool {
        let mut child = match self.command(&FOLLOW_ARGS).spawn() {
            Ok(child) => child,
            Err(err) => {
                let _ = output.send(FollowEvent::Unavailable(format!("`{}` not found on PATH ({err})", self.executable))).await;
                return false;
            }
        };
        let stdout = child.stdout.take().expect("stdout piped");
        let stderr = child.stderr.take().expect("stderr piped");
        // Drain stderr concurrently so a chatty daemon can never block on a full pipe;
        // only the last non-empty line is kept for diagnostics.
        let stderr_task = tokio::spawn(async move {
            let mut last = String::new();
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if !line.trim().is_empty() {
                    last = line.chars().take(500).collect();
                }
            }
            last
        });
        let mut lines = BufReader::new(stdout).lines();
        let mut had_state = false;
        while let Ok(Some(line)) = lines.next_line().await {
            let Ok(event) = serde_json::from_str::<Value>(&line) else { continue };
            let Some(kind) = event.get("type").and_then(Value::as_str) else { continue };
            had_state = true;
            let data = event.get("data").cloned().unwrap_or(Value::Null);
            if output.send(FollowEvent::Event { kind: kind.to_string(), data }).await.is_err() {
                let _ = child.kill().await;
                stderr_task.abort();
                return had_state;
            }
        }
        let status = child.wait().await.ok();
        let mut error = stderr_task.await.unwrap_or_default();
        if error.is_empty() {
            error = format!("`tuple state follow` exited ({})", status.map(|s| s.to_string()).unwrap_or_default());
        }
        let _ = output.send(FollowEvent::Ended { error, had_state }).await;
        had_state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::FakeTuple;

    fn cli(fake: &FakeTuple) -> TupleCli {
        TupleCli { executable: crate::test_support::fake_tuple_path().to_string_lossy().into_owned(), env: fake.env() }
    }

    async fn next(receiver: &Receiver<FollowEvent>) -> FollowEvent {
        tokio::time::timeout(Duration::from_secs(5), receiver.recv()).await.unwrap().unwrap()
    }

    #[tokio::test]
    async fn runs_commands_and_records_them() {
        let fake = FakeTuple::new();
        let result = cli(&fake).run(vec!["call".into(), "mute".into(), "--call".into(), "current".into()]).await;
        assert!(result.ok());
        assert_eq!(result.stdout.trim(), "Muted");
        assert_eq!(fake.calls(), vec![vec!["call", "mute", "--call", "current"]]);
    }

    #[tokio::test]
    async fn reports_errors_from_stderr_and_json() {
        let fake = FakeTuple::new();
        fake.fail_with("dial unix /run/user/1000/tuple.sock: connect: no such file");
        let result = cli(&fake).run(vec!["state".into()]).await;
        assert!(!result.ok());
        assert!(result.error().contains("tuple.sock"));

        let missing = TupleCli { executable: "/nonexistent/tuple".into(), env: vec![] };
        let result = missing.run(vec!["version".into()]).await;
        assert!(result.spawn_error.is_some());
        assert!(result.error().contains("Could not start"));
    }

    #[tokio::test]
    async fn auth_codes_never_appear_in_error_text() {
        let missing = TupleCli { executable: "/nonexistent/tuple".into(), env: vec![] };
        let result = missing.run(vec!["auth".into(), "SECRET-1234".into()]).await;
        assert!(!result.error().contains("SECRET-1234"), "{}", result.error());
        assert_eq!(result.display_command(), "tuple auth ••••");

        let fake = FakeTuple::new();
        fake.fail_with("auth failed for code SECRET-1234");
        let result = cli(&fake).run(vec!["auth".into(), "SECRET-1234".into()]).await;
        assert!(!result.ok());
        assert_eq!(result.error(), "auth failed for code ••••");
    }

    #[tokio::test]
    async fn follow_survives_a_flood_of_stderr() {
        let fake = FakeTuple::new();
        std::fs::write(fake.dir.join("stderr_flood"), "1048576").unwrap();
        let events = cli(&fake).follow(&tokio::runtime::Handle::current());
        match next(&events).await {
            FollowEvent::Event { kind, .. } => assert_eq!(kind, "state"),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[tokio::test]
    async fn follow_streams_initial_state_and_live_events() {
        let fake = FakeTuple::new();
        let events = cli(&fake).follow(&tokio::runtime::Handle::current());
        match next(&events).await {
            FollowEvent::Event { kind, data } => {
                assert_eq!(kind, "state");
                assert_eq!(data["current_user"]["full_name"], "Jo Example");
            }
            other => panic!("unexpected {other:?}"),
        }
        fake.emit("current_call", serde_json::json!({"id": "abc", "local": {"audio_enabled": false}}));
        let second = next(&events).await;
        assert_eq!(second, FollowEvent::Event { kind: "current_call".into(), data: serde_json::json!({"id": "abc", "local": {"audio_enabled": false}}) });
    }

    #[tokio::test]
    async fn follow_reports_failures_and_missing_executable() {
        let fake = FakeTuple::new();
        fake.fail_with("daemon exploded");
        let events = cli(&fake).follow(&tokio::runtime::Handle::current());
        assert_eq!(next(&events).await, FollowEvent::Ended { error: "daemon exploded".into(), had_state: false });

        let missing = TupleCli { executable: "/nonexistent/tuple".into(), env: vec![] };
        let events = missing.follow(&tokio::runtime::Handle::current());
        match next(&events).await {
            FollowEvent::Unavailable(message) => assert!(message.contains("/nonexistent/tuple")),
            other => panic!("unexpected {other:?}"),
        }
    }
}
