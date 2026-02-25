//! Echo agent example.
//!
//! Starts an A2A agent that echoes back any text messages it receives.
//!
//! Usage:
//!   cargo run --example echo_agent
//!   cargo run --example echo_agent -- --waku http://localhost:8645

use anyhow::Result;
use waku_a2a::{NwakuRestTransport, WakuA2ANode};

#[tokio::main]
async fn main() -> Result<()> {
    let waku_url = std::env::args()
        .skip_while(|a| a != "--waku")
        .nth(1)
        .unwrap_or_else(|| "http://localhost:8645".to_string());

    let transport = NwakuRestTransport::new(&waku_url);
    let node = WakuA2ANode::new(
        "echo",
        "Echoes back any text message",
        vec!["text".to_string()],
        transport,
    );

    println!("=== Echo Agent ===");
    println!("Name:   {}", node.card.name);
    println!("Pubkey: {}", node.pubkey());
    println!();

    // Announce presence
    match node.announce().await {
        Ok(()) => println!("Announced on discovery topic."),
        Err(e) => eprintln!("Warning: could not announce (nwaku not running?): {}", e),
    }

    println!("Listening for tasks... (Ctrl+C to stop)\n");

    loop {
        match node.poll_tasks().await {
            Ok(tasks) => {
                for task in &tasks {
                    let text = task.text().unwrap_or("<no text>");
                    println!("[recv] Task {} from {}", task.id, &task.from[..12.min(task.from.len())]);
                    println!("       Text: {}", text);

                    let response = format!("Echo: {}", text);
                    match node.respond(task, &response).await {
                        Ok(()) => println!("       Replied: {}\n", response),
                        Err(e) => eprintln!("       Reply failed: {}\n", e),
                    }
                }
            }
            Err(e) => {
                // Silently retry — nwaku might not be running
                eprintln!("[poll error] {}", e);
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}
