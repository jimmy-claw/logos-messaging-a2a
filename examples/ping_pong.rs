//! Ping-pong example: two agents exchanging tasks.
//!
//! This example creates two in-process agents using an in-memory mock transport,
//! demonstrating the A2A task flow without requiring a running nwaku node.
//!
//! Usage:
//!   cargo run --example ping_pong

use anyhow::Result;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use waku_a2a::{Task, WakuA2ANode, WakuTransport};

/// Simple in-memory transport for demo purposes.
struct InMemoryTransport {
    messages: Arc<Mutex<Vec<(String, Vec<u8>)>>>,
}

impl InMemoryTransport {
    fn new(store: Arc<Mutex<Vec<(String, Vec<u8>)>>>) -> Self {
        Self { messages: store }
    }
}

#[async_trait]
impl WakuTransport for InMemoryTransport {
    async fn publish(&self, topic: &str, payload: &[u8]) -> Result<()> {
        let mut msgs = self.messages.lock().unwrap();
        msgs.push((topic.to_string(), payload.to_vec()));
        Ok(())
    }

    async fn subscribe(&self, _topic: &str) -> Result<()> {
        Ok(())
    }

    async fn poll(&self, topic: &str) -> Result<Vec<Vec<u8>>> {
        let mut msgs = self.messages.lock().unwrap();
        let mut result = Vec::new();
        let mut remaining = Vec::new();
        for (t, p) in msgs.drain(..) {
            if t == topic {
                result.push(p);
            } else {
                remaining.push((t, p));
            }
        }
        *msgs = remaining;
        Ok(result)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Ping-Pong Demo ===\n");

    // Shared message store (simulates Waku relay network)
    let store = Arc::new(Mutex::new(Vec::new()));

    let ping_transport = InMemoryTransport::new(store.clone());
    let pong_transport = InMemoryTransport::new(store.clone());

    let ping = WakuA2ANode::new(
        "ping",
        "Sends ping messages",
        vec!["text".to_string()],
        ping_transport,
    );
    let pong = WakuA2ANode::new(
        "pong",
        "Responds to pings with pongs",
        vec!["text".to_string()],
        pong_transport,
    );

    println!("Ping agent: {} ({}...)", ping.card.name, &ping.pubkey()[..16]);
    println!("Pong agent: {} ({}...)\n", pong.card.name, &pong.pubkey()[..16]);

    // Both announce
    ping.announce().await?;
    pong.announce().await?;

    // Ping discovers Pong
    let discovered = ping.discover().await?;
    println!("Ping discovered {} agent(s)", discovered.len());
    for card in &discovered {
        println!("  -> {} ({}...)", card.name, &card.public_key[..16]);
    }
    println!();

    // Ping sends a task to Pong
    let task = Task::new(ping.pubkey(), pong.pubkey(), "Ping!");
    println!("[ping] Sending: \"Ping!\" (task {})", &task.id[..8]);
    // Use direct publish (no SDS ACK wait for in-memory demo)
    let envelope = waku_a2a_core::A2AEnvelope::Task(task.clone());
    let payload = serde_json::to_vec(&envelope)?;
    let topic = waku_a2a_core::topics::task_topic(pong.pubkey());
    pong.poll_tasks().await?; // clear any stale
    // Re-inject the task
    {
        let mut msgs = store.lock().unwrap();
        msgs.push((topic, payload));
    }

    // Pong polls and gets the task
    let tasks = pong.poll_tasks().await?;
    for t in &tasks {
        let text = t.text().unwrap_or("?");
        println!("[pong] Received: \"{}\" (task {})", text, &t.id[..8]);

        // Pong responds
        let response = format!("Pong! (reply to: {})", text);
        pong.respond(t, &response).await?;
        println!("[pong] Replied: \"{}\"", response);
    }

    // Ping polls for response
    let responses = ping.poll_tasks().await?;
    for r in &responses {
        if let Some(text) = r.result_text() {
            println!("[ping] Got response: \"{}\"", text);
        }
    }

    println!("\nDone! Both agents communicated via in-memory Waku transport.");
    Ok(())
}
