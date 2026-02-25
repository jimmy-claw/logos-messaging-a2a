use anyhow::Result;
use async_trait::async_trait;

pub mod nwaku_rest;
pub mod sds;

/// Swappable Waku transport trait.
///
/// Two implementations planned:
/// - `LogosDeliveryTransport`: uses logos-delivery-rust-bindings (waku-bindings FFI)
///   TODO (Issue #1): implement once libwaku build is resolved
/// - `NwakuRestTransport`: uses nwaku REST API as fallback (current default)
#[async_trait]
pub trait WakuTransport: Send + Sync {
    /// Publish a payload to a Waku content topic.
    async fn publish(&self, topic: &str, payload: &[u8]) -> Result<()>;

    /// Subscribe to a Waku content topic.
    async fn subscribe(&self, topic: &str) -> Result<()>;

    /// Poll for messages on a content topic. Returns raw payloads.
    async fn poll(&self, topic: &str) -> Result<Vec<Vec<u8>>>;
}

// TODO (Issue #1): Implement LogosDeliveryTransport using waku-bindings FFI
// This would use the waku-bindings crate from:
//   https://github.com/logos-messaging/logos-delivery-rust-bindings
//
// The FFI approach embeds libwaku directly (no separate nwaku process),
// making it ideal for Logos Core integration. However, it requires:
// - Nim toolchain to compile libwaku.so
// - waku-sys build script to link the native library
//
// pub struct LogosDeliveryTransport { handle: WakuNodeHandle<Running> }
