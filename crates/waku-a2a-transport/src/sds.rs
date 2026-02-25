//! Minimal SDS (Scalable Data Sync) — reliable delivery layer.
//!
//! Waku is fire-and-forget. For agent tasks we need delivery guarantees.
//! This implements a minimal SDS-inspired protocol:
//!
//! - Each message has a UUID
//! - Sender publishes and then polls for ACK on /waku-a2a/1/ack/{message_id}/proto
//! - Receiver sends ACK after processing
//! - If no ACK within timeout: retransmit up to MAX_RETRIES times
//!
//! TODO (Issue #2): Replace with the full SDS protocol spec.
//! Reference: https://blog.waku.org/explanation-series-a-unified-stack-for-scalable-and-reliable-p2p-communication/

use crate::WakuTransport;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;

const ACK_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RETRIES: u32 = 3;
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Minimal SDS layer wrapping any WakuTransport.
pub struct SdsTransport<T: WakuTransport> {
    inner: T,
    /// Bloom filter substitute: set of seen message IDs for deduplication.
    /// TODO (Issue #2): Replace with proper bloom filter from SDS spec.
    seen_ids: Mutex<HashSet<String>>,
}

impl<T: WakuTransport> SdsTransport<T> {
    pub fn new(transport: T) -> Self {
        Self {
            inner: transport,
            seen_ids: Mutex::new(HashSet::new()),
        }
    }

    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Publish with SDS reliability: retransmit up to MAX_RETRIES times
    /// if no ACK is received within ACK_TIMEOUT.
    pub async fn publish_reliable(
        &self,
        topic: &str,
        payload: &[u8],
        message_id: &str,
    ) -> Result<bool> {
        let ack_topic = format!("/waku-a2a/1/ack/{}/proto", message_id);
        self.inner.subscribe(&ack_topic).await?;

        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                tracing_log(
                    &format!("SDS: retransmit attempt {}/{} for {}", attempt, MAX_RETRIES, message_id),
                );
            }

            self.inner
                .publish(topic, payload)
                .await
                .context("SDS publish failed")?;

            // Poll for ACK
            if self.wait_for_ack(&ack_topic, message_id).await? {
                return Ok(true);
            }
        }

        tracing_log(&format!(
            "SDS: no ACK after {} retries for {}",
            MAX_RETRIES, message_id
        ));
        Ok(false)
    }

    /// Send an ACK for a received message.
    pub async fn send_ack(&self, message_id: &str) -> Result<()> {
        let ack_topic = format!("/waku-a2a/1/ack/{}/proto", message_id);
        let ack_payload = serde_json::to_vec(&serde_json::json!({
            "type": "ack",
            "message_id": message_id,
        }))?;
        self.inner.publish(&ack_topic, &ack_payload).await
    }

    /// Check if a message ID has been seen before (deduplication).
    pub fn is_duplicate(&self, message_id: &str) -> bool {
        let seen = self.seen_ids.lock().unwrap();
        seen.contains(message_id)
    }

    /// Mark a message ID as seen.
    pub fn mark_seen(&self, message_id: &str) {
        let mut seen = self.seen_ids.lock().unwrap();
        seen.insert(message_id.to_string());
    }

    /// Poll the inner transport, filtering duplicates.
    pub async fn poll_dedup(&self, topic: &str) -> Result<Vec<Vec<u8>>> {
        let messages = self.inner.poll(topic).await?;
        let mut result = Vec::new();
        for msg in messages {
            // Try to extract message_id for dedup
            if let Ok(envelope) =
                serde_json::from_slice::<serde_json::Value>(&msg)
            {
                if let Some(id) = envelope.get("id").and_then(|v| v.as_str()) {
                    if self.is_duplicate(id) {
                        continue;
                    }
                    self.mark_seen(id);
                }
            }
            result.push(msg);
        }
        Ok(result)
    }

    async fn wait_for_ack(&self, ack_topic: &str, message_id: &str) -> Result<bool> {
        let deadline = tokio::time::Instant::now() + ACK_TIMEOUT;
        while tokio::time::Instant::now() < deadline {
            let messages = self.inner.poll(ack_topic).await.unwrap_or_default();
            for msg in messages {
                if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&msg) {
                    if val.get("message_id").and_then(|v| v.as_str()) == Some(message_id) {
                        return Ok(true);
                    }
                }
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
        Ok(false)
    }
}

fn tracing_log(msg: &str) {
    eprintln!("[sds] {}", msg);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WakuTransport;
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex as StdMutex};

    /// In-memory transport for testing.
    struct MockTransport {
        published: Arc<StdMutex<Vec<(String, Vec<u8>)>>>,
        /// Messages to return on poll, keyed by topic.
        poll_responses: Arc<StdMutex<Vec<(String, Vec<u8>)>>>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                published: Arc::new(StdMutex::new(Vec::new())),
                poll_responses: Arc::new(StdMutex::new(Vec::new())),
            }
        }

        fn inject_message(&self, topic: &str, payload: Vec<u8>) {
            let mut responses = self.poll_responses.lock().unwrap();
            responses.push((topic.to_string(), payload));
        }
    }

    #[async_trait]
    impl WakuTransport for MockTransport {
        async fn publish(&self, topic: &str, payload: &[u8]) -> Result<()> {
            let mut published = self.published.lock().unwrap();
            published.push((topic.to_string(), payload.to_vec()));
            Ok(())
        }

        async fn subscribe(&self, _topic: &str) -> Result<()> {
            Ok(())
        }

        async fn poll(&self, topic: &str) -> Result<Vec<Vec<u8>>> {
            let mut responses = self.poll_responses.lock().unwrap();
            let mut result = Vec::new();
            let mut remaining = Vec::new();
            for (t, payload) in responses.drain(..) {
                if t == topic {
                    result.push(payload);
                } else {
                    remaining.push((t, payload));
                }
            }
            *responses = remaining;
            Ok(result)
        }
    }

    #[test]
    fn test_deduplication() {
        let transport = MockTransport::new();
        let sds = SdsTransport::new(transport);

        assert!(!sds.is_duplicate("msg-1"));
        sds.mark_seen("msg-1");
        assert!(sds.is_duplicate("msg-1"));
        assert!(!sds.is_duplicate("msg-2"));
    }

    #[tokio::test]
    async fn test_send_ack() {
        let transport = MockTransport::new();
        let published = transport.published.clone();
        let sds = SdsTransport::new(transport);

        sds.send_ack("task-123").await.unwrap();

        let msgs = published.lock().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].0, "/waku-a2a/1/ack/task-123/proto");
        let val: serde_json::Value = serde_json::from_slice(&msgs[0].1).unwrap();
        assert_eq!(val["message_id"], "task-123");
    }

    #[tokio::test]
    async fn test_publish_reliable_with_ack() {
        let transport = MockTransport::new();
        // Pre-inject an ACK response
        let ack_payload = serde_json::to_vec(&serde_json::json!({
            "type": "ack",
            "message_id": "msg-1",
        }))
        .unwrap();
        transport.inject_message("/waku-a2a/1/ack/msg-1/proto", ack_payload);

        let sds = SdsTransport::new(transport);

        let acked = sds
            .publish_reliable("/waku-a2a/1/task/somepubkey/proto", b"hello", "msg-1")
            .await
            .unwrap();
        assert!(acked);
    }

    #[tokio::test]
    async fn test_poll_dedup() {
        let transport = MockTransport::new();

        // Inject same message twice
        let msg = serde_json::to_vec(&serde_json::json!({
            "id": "task-1",
            "data": "hello"
        }))
        .unwrap();
        transport.inject_message("topic-a", msg.clone());
        transport.inject_message("topic-a", msg);

        let sds = SdsTransport::new(transport);

        let result = sds.poll_dedup("topic-a").await.unwrap();
        // Second message should be deduped
        assert_eq!(result.len(), 1);
    }
}
