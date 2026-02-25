use anyhow::{Context, Result};
use k256::ecdsa::SigningKey;
use waku_a2a_core::{topics, A2AEnvelope, AgentCard, Task};
use waku_a2a_transport::sds::SdsTransport;
use waku_a2a_transport::WakuTransport;

/// A2A node: announce, discover, send/receive tasks over Waku.
pub struct WakuA2ANode<T: WakuTransport> {
    pub card: AgentCard,
    transport: SdsTransport<T>,
    signing_key: SigningKey,
}

impl<T: WakuTransport> WakuA2ANode<T> {
    /// Create a new node with a random keypair.
    pub fn new(name: &str, description: &str, capabilities: Vec<String>, transport: T) -> Self {
        let signing_key = SigningKey::random(&mut rand_core());
        let public_key = hex::encode(
            signing_key
                .verifying_key()
                .to_encoded_point(true)
                .as_bytes(),
        );

        let card = AgentCard {
            name: name.to_string(),
            description: description.to_string(),
            version: "0.1.0".to_string(),
            capabilities,
            public_key,
        };

        Self {
            card,
            transport: SdsTransport::new(transport),
            signing_key,
        }
    }

    /// Create a node from an existing signing key.
    pub fn from_key(
        name: &str,
        description: &str,
        capabilities: Vec<String>,
        transport: T,
        signing_key: SigningKey,
    ) -> Self {
        let public_key = hex::encode(
            signing_key
                .verifying_key()
                .to_encoded_point(true)
                .as_bytes(),
        );

        let card = AgentCard {
            name: name.to_string(),
            description: description.to_string(),
            version: "0.1.0".to_string(),
            capabilities,
            public_key,
        };

        Self {
            card,
            transport: SdsTransport::new(transport),
            signing_key,
        }
    }

    /// Get this agent's public key hex string.
    pub fn pubkey(&self) -> &str {
        &self.card.public_key
    }

    /// Get the signing key (for testing or advanced use).
    pub fn signing_key(&self) -> &SigningKey {
        &self.signing_key
    }

    /// Broadcast this agent's card on the discovery topic.
    pub async fn announce(&self) -> Result<()> {
        let envelope = A2AEnvelope::AgentCard(self.card.clone());
        let payload = serde_json::to_vec(&envelope).context("Failed to serialize AgentCard")?;
        self.transport
            .inner()
            .publish(topics::DISCOVERY, &payload)
            .await
            .context("Failed to announce AgentCard")?;
        eprintln!("[node] Announced: {} ({})", self.card.name, self.pubkey());
        Ok(())
    }

    /// Discover agents by polling the discovery topic.
    pub async fn discover(&self) -> Result<Vec<AgentCard>> {
        self.transport
            .inner()
            .subscribe(topics::DISCOVERY)
            .await?;
        let messages = self.transport.inner().poll(topics::DISCOVERY).await?;
        let mut cards = Vec::new();
        for msg in messages {
            if let Ok(A2AEnvelope::AgentCard(card)) = serde_json::from_slice(&msg) {
                // Don't include self
                if card.public_key != self.card.public_key {
                    cards.push(card);
                }
            }
        }
        Ok(cards)
    }

    /// Send a task to another agent. Uses SDS for reliable delivery.
    pub async fn send_task(&self, task: &Task) -> Result<bool> {
        let topic = topics::task_topic(&task.to);
        let envelope = A2AEnvelope::Task(task.clone());
        let payload = serde_json::to_vec(&envelope).context("Failed to serialize task")?;

        self.transport
            .inner()
            .subscribe(&topic)
            .await?;

        let acked = self
            .transport
            .publish_reliable(&topic, &payload, &task.id)
            .await
            .context("SDS publish failed")?;

        if acked {
            eprintln!("[node] Task {} sent and ACKed", task.id);
        } else {
            eprintln!("[node] Task {} sent but no ACK received", task.id);
        }
        Ok(acked)
    }

    /// Poll for incoming tasks addressed to this agent.
    pub async fn poll_tasks(&self) -> Result<Vec<Task>> {
        let topic = topics::task_topic(&self.card.public_key);
        self.transport.inner().subscribe(&topic).await?;
        let messages = self.transport.poll_dedup(&topic).await?;
        let mut tasks = Vec::new();
        for msg in messages {
            if let Ok(A2AEnvelope::Task(task)) = serde_json::from_slice(&msg) {
                // ACK the received task (SDS)
                let _ = self.transport.send_ack(&task.id).await;
                tasks.push(task);
            }
        }
        Ok(tasks)
    }

    /// Respond to a task: send back a completed task with result.
    pub async fn respond(&self, task: &Task, result_text: &str) -> Result<()> {
        let response = task.respond(result_text);
        let topic = topics::task_topic(&response.to);
        let envelope = A2AEnvelope::Task(response.clone());
        let payload = serde_json::to_vec(&envelope)?;

        self.transport
            .inner()
            .publish(&topic, &payload)
            .await
            .context("Failed to send response")?;

        eprintln!("[node] Responded to task {}", task.id);
        Ok(())
    }

    /// Create a task and send it.
    pub async fn send_text(&self, to: &str, text: &str) -> Result<Task> {
        let task = Task::new(self.pubkey(), to, text);
        self.send_task(&task).await?;
        Ok(task)
    }
}

/// Platform-appropriate RNG.
fn rand_core() -> k256::elliptic_curve::rand_core::OsRng {
    k256::elliptic_curve::rand_core::OsRng
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    struct MockTransport {
        published: Arc<Mutex<Vec<(String, Vec<u8>)>>>,
        poll_responses: Arc<Mutex<Vec<(String, Vec<u8>)>>>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                published: Arc::new(Mutex::new(Vec::new())),
                poll_responses: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn inject(&self, topic: &str, payload: Vec<u8>) {
            let mut r = self.poll_responses.lock().unwrap();
            r.push((topic.to_string(), payload));
        }
    }

    #[async_trait]
    impl WakuTransport for MockTransport {
        async fn publish(&self, topic: &str, payload: &[u8]) -> Result<()> {
            let mut p = self.published.lock().unwrap();
            p.push((topic.to_string(), payload.to_vec()));
            Ok(())
        }
        async fn subscribe(&self, _topic: &str) -> Result<()> {
            Ok(())
        }
        async fn poll(&self, topic: &str) -> Result<Vec<Vec<u8>>> {
            let mut r = self.poll_responses.lock().unwrap();
            let mut result = Vec::new();
            let mut remaining = Vec::new();
            for (t, p) in r.drain(..) {
                if t == topic {
                    result.push(p);
                } else {
                    remaining.push((t, p));
                }
            }
            *r = remaining;
            Ok(result)
        }
    }

    #[test]
    fn test_node_creation() {
        let transport = MockTransport::new();
        let node = WakuA2ANode::new("test", "test agent", vec!["text".into()], transport);
        assert_eq!(node.card.name, "test");
        assert!(!node.pubkey().is_empty());
        // secp256k1 compressed pubkey is 33 bytes = 66 hex chars
        assert_eq!(node.pubkey().len(), 66);
    }

    #[tokio::test]
    async fn test_announce() {
        let transport = MockTransport::new();
        let published = transport.published.clone();
        let node = WakuA2ANode::new("echo", "echo agent", vec!["text".into()], transport);

        node.announce().await.unwrap();

        let msgs = published.lock().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].0, topics::DISCOVERY);

        let envelope: A2AEnvelope = serde_json::from_slice(&msgs[0].1).unwrap();
        match envelope {
            A2AEnvelope::AgentCard(card) => {
                assert_eq!(card.name, "echo");
                assert_eq!(card.public_key, node.pubkey());
            }
            _ => panic!("Expected AgentCard envelope"),
        }
    }

    #[tokio::test]
    async fn test_discover() {
        let transport = MockTransport::new();
        let other_card = AgentCard {
            name: "other".to_string(),
            description: "other agent".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec!["code".to_string()],
            public_key: "02deadbeef".to_string(),
        };
        let envelope = A2AEnvelope::AgentCard(other_card.clone());
        let payload = serde_json::to_vec(&envelope).unwrap();
        transport.inject(topics::DISCOVERY, payload);

        let node = WakuA2ANode::new("me", "my agent", vec![], transport);
        let cards = node.discover().await.unwrap();

        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].name, "other");
    }

    #[tokio::test]
    async fn test_poll_tasks() {
        let transport = MockTransport::new();
        let node = WakuA2ANode::new("echo", "echo agent", vec!["text".into()], transport);

        // SdsTransport wraps the mock, so we can't inject after construction.
        // This test validates the API works with an empty inbox.
        let tasks = node.poll_tasks().await.unwrap();
        assert!(tasks.is_empty());
    }
}
