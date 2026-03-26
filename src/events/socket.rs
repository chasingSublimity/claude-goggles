use std::path::PathBuf;
use tokio::sync::mpsc;

use super::HookEvent;

pub struct SocketListener {
    path: PathBuf,
}

impl SocketListener {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Remove stale socket file if it exists
    fn cleanup_stale(&self) {
        if self.path.exists() {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    /// Start listening. Sends parsed HookEvents through the channel.
    /// Runs until the channel is closed.
    pub async fn listen(&self, tx: mpsc::Sender<HookEvent>) -> std::io::Result<()> {
        self.cleanup_stale();
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let listener = tokio::net::UnixListener::bind(&self.path)?;

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        let _ = handle_connection(stream, tx).await;
                    });
                }
                Err(_) => continue,
            }
        }
    }
}

impl Drop for SocketListener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

async fn handle_connection(
    mut stream: tokio::net::UnixStream,
    tx: mpsc::Sender<HookEvent>,
) -> std::io::Result<()> {
    use tokio::io::AsyncReadExt;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await?;
    let json = String::from_utf8_lossy(&buf);
    if let Some(event) = super::parse_hook_event(&json) {
        let event = resolve_transcript_usage(event).await;
        let _ = tx.send(event).await;
    }
    Ok(())
}

/// Resolve token_usage from transcript files without blocking the async executor.
async fn resolve_transcript_usage(event: HookEvent) -> HookEvent {
    match event {
        HookEvent::SubagentStop {
            session_id,
            agent_id,
            agent_type,
            transcript_path,
            ..
        } => {
            let token_usage = if let Some(ref path) = transcript_path {
                let path = std::path::PathBuf::from(path);
                tokio::task::spawn_blocking(move || {
                    super::transcript::parse_transcript_usage(&path)
                })
                .await
                .ok()
                .flatten()
            } else {
                None
            };
            HookEvent::SubagentStop {
                session_id,
                agent_id,
                agent_type,
                token_usage,
                transcript_path,
            }
        }
        HookEvent::Stop {
            session_id,
            transcript_path,
            ..
        } => {
            let token_usage = if let Some(ref path) = transcript_path {
                let path = std::path::PathBuf::from(path);
                tokio::task::spawn_blocking(move || {
                    super::transcript::parse_transcript_usage(&path)
                })
                .await
                .ok()
                .flatten()
            } else {
                None
            };
            HookEvent::Stop {
                session_id,
                token_usage,
                transcript_path,
            }
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    use tokio::net::UnixStream;

    #[tokio::test]
    async fn test_socket_receives_event() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("test.sock");
        let listener = SocketListener::new(sock_path.clone());
        let (tx, mut rx) = mpsc::channel(100);

        let handle = tokio::spawn(async move {
            listener.listen(tx).await.unwrap();
        });

        // Give the listener time to bind
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Send a valid event
        let json = r#"{"session_id":"s1","hook_event_name":"PreToolUse","tool_name":"Read","tool_input":{"file_path":"test.rs"},"tool_use_id":"t1"}"#;
        let mut stream = UnixStream::connect(&sock_path).await.unwrap();
        stream.write_all(json.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let event = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            rx.recv(),
        ).await.unwrap().unwrap();

        assert!(matches!(event, HookEvent::PreToolUse { .. }));

        handle.abort();
    }

    #[tokio::test]
    async fn test_socket_drops_malformed_json() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("test2.sock");
        let listener = SocketListener::new(sock_path.clone());
        let (tx, mut rx) = mpsc::channel(100);

        let handle = tokio::spawn(async move {
            listener.listen(tx).await.unwrap();
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Send garbage
        let mut stream = UnixStream::connect(&sock_path).await.unwrap();
        stream.write_all(b"not json").await.unwrap();
        stream.shutdown().await.unwrap();

        // Small delay to let it process
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Send valid event after
        let json = r#"{"session_id":"s1","hook_event_name":"Stop","transcript_path":"/tmp/t.jsonl"}"#;
        let mut stream2 = UnixStream::connect(&sock_path).await.unwrap();
        stream2.write_all(json.as_bytes()).await.unwrap();
        stream2.shutdown().await.unwrap();

        let event = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            rx.recv(),
        ).await.unwrap().unwrap();

        // Should get Stop, not the malformed event
        assert!(matches!(event, HookEvent::Stop { .. }));

        handle.abort();
    }
}
