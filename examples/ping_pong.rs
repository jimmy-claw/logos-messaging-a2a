//! Ping-pong example: two agents exchanging tasks.
//!
//! This example creates two in-process agents using an in-memory mock transport,
//! demonstrating the A2A task flow without requiring a running nwaku node.
//!
//! Usage:
//!   cargo run --example ping_pong
//!   cargo run --example ping_pong -- --encrypt

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
    let encrypt = std::env::args().any(|a| a == "--encrypt");

    if encrypt {
        run_encrypted().await
    } else {
        run_plaintext().await
    }
}

async fn run_plaintext() -> Result<()> {
    println!("=== Ping-Pong Demo (plaintext) ===\n");

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

    println!(
        "Ping agent: {} ({}...)",
        ping.card.name,
        &ping.pubkey()[..16]
    );
    println!(
        "Pong agent: {} ({}...)\n",
        pong.card.name,
        &pong.pubkey()[..16]
    );

    ping.announce().await?;
    pong.announce().await?;

    let discovered = ping.discover().await?;
    println!("Ping discovered {} agent(s)", discovered.len());
    for card in &discovered {
        println!("  -> {} ({}...)", card.name, &card.public_key[..16]);
    }
    println!();

    let task = Task::new(ping.pubkey(), pong.pubkey(), "Ping!");
    println!("[ping] Sending: \"Ping!\" (task {})", &task.id[..8]);
    let envelope = waku_a2a::A2AEnvelope::Task(task.clone());
    let payload = serde_json::to_vec(&envelope)?;
    let topic = waku_a2a::topics::task_topic(pong.pubkey());
    pong.poll_tasks().await?;
    {
        let mut msgs = store.lock().unwrap();
        msgs.push((topic, payload));
    }

    let tasks = pong.poll_tasks().await?;
    for t in &tasks {
        let text = t.text().unwrap_or("?");
        println!("[pong] Received: \"{}\" (task {})", text, &t.id[..8]);
        let response = format!("Pong! (reply to: {})", text);
        pong.respond(t, &response).await?;
        println!("[pong] Replied: \"{}\"", response);
    }

    let responses = ping.poll_tasks().await?;
    for r in &responses {
        if let Some(text) = r.result_text() {
            println!("[ping] Got response: \"{}\"", text);
        }
    }

    println!("\nDone! Both agents communicated via in-memory Waku transport.");
    Ok(())
}

async fn run_encrypted() -> Result<()> {
    println!("=== Ping-Pong Demo (encrypted: X25519+ChaCha20-Poly1305) ===\n");

    let store = Arc::new(Mutex::new(Vec::new()));

    let ping_transport = InMemoryTransport::new(store.clone());
    let pong_transport = InMemoryTransport::new(store.clone());

    let ping = WakuA2ANode::new_encrypted(
        "ping",
        "Sends encrypted ping messages",
        vec!["text".to_string()],
        ping_transport,
    );
    let pong = WakuA2ANode::new_encrypted(
        "pong",
        "Responds to encrypted pings",
        vec!["text".to_string()],
        pong_transport,
    );

    let ping_bundle = ping.card.intro_bundle.as_ref().unwrap();
    let pong_bundle = pong.card.intro_bundle.as_ref().unwrap();
    println!(
        "Ping agent: {} (X25519: {}...)",
        ping.card.name,
        &ping_bundle.agent_pubkey[..16]
    );
    println!(
        "Pong agent: {} (X25519: {}...)\n",
        pong.card.name,
        &pong_bundle.agent_pubkey[..16]
    );

    ping.announce().await?;
    pong.announce().await?;

    let discovered = ping.discover().await?;
    println!("Ping discovered {} agent(s)", discovered.len());
    for card in &discovered {
        let enc = if card.intro_bundle.is_some() {
            "encrypted"
        } else {
            "plaintext"
        };
        println!(
            "  -> {} ({}...) [{}]",
            card.name,
            &card.public_key[..16],
            enc
        );
    }
    println!();

    // Ping sends encrypted task to Pong (using Pong's card for key agreement)
    let task = Task::new(ping.pubkey(), pong.pubkey(), "Ping! (encrypted)");
    println!(
        "[ping] Sending encrypted: \"Ping! (encrypted)\" (task {})",
        &task.id[..8]
    );
    // Encrypt using pong's card
    let envelope = {
        // We need to manually encrypt here since send_task_to requires the card
        let pong_card = &pong.card;
        let identity = ping.identity().unwrap();
        let their_pubkey = waku_a2a::AgentIdentity::parse_public_key(
            &pong_card.intro_bundle.as_ref().unwrap().agent_pubkey,
        )?;
        let session_key = identity.shared_key(&their_pubkey);
        let task_json = serde_json::to_vec(&task)?;
        let encrypted = session_key.encrypt(&task_json)?;
        waku_a2a::A2AEnvelope::EncryptedTask {
            encrypted,
            sender_pubkey: identity.public_key_hex(),
        }
    };
    let payload = serde_json::to_vec(&envelope)?;
    let topic = waku_a2a::topics::task_topic(pong.pubkey());
    pong.poll_tasks().await?;
    {
        let mut msgs = store.lock().unwrap();
        msgs.push((topic, payload));
    }

    // Pong polls and decrypts
    let tasks = pong.poll_tasks().await?;
    for t in &tasks {
        let text = t.text().unwrap_or("?");
        println!("[pong] Decrypted: \"{}\" (task {})", text, &t.id[..8]);
        let response = format!("Pong! (reply to: {})", text);
        // Respond encrypted using ping's card
        pong.respond_to(t, &response, Some(&ping.card)).await?;
        println!("[pong] Replied (encrypted): \"{}\"", response);
    }

    // Ping polls for encrypted response
    let responses = ping.poll_tasks().await?;
    for r in &responses {
        if let Some(text) = r.result_text() {
            println!("[ping] Decrypted response: \"{}\"", text);
        }
    }

    println!("\nDone! Both agents communicated with end-to-end encryption.");
    Ok(())
}
