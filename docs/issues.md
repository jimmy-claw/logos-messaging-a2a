# Follow-up Issues

Pre-written GitHub issues for the logos-messaging-a2a project. File these after the v0.1 prototype is stable.

---

## Issue #1: Use logos-delivery-rust-bindings as primary transport

**Labels:** `enhancement`, `transport`, `priority:high`

Replace the `NwakuRestTransport` fallback with a proper `LogosDeliveryTransport` using the libwaku FFI bindings.

**Context:**
The v0.1 prototype uses the nwaku REST API (`http://localhost:8645`) because the logos-delivery-rust-bindings require:
- Nim toolchain to compile `libwaku.so`
- `waku-sys` build script to link the native library
- Platform-specific build configuration

**Tasks:**
- [ ] Add `waku-bindings` as a git dependency from https://github.com/logos-messaging/logos-delivery-rust-bindings
- [ ] Implement `WakuTransport` trait for `LogosDeliveryTransport` using `WakuNodeHandle<Running>`
- [ ] Set up CI with Nim toolchain for building `libwaku.so`
- [ ] Make `LogosDeliveryTransport` the default, with `NwakuRestTransport` as opt-in fallback
- [ ] Document build prerequisites in README

**Link:** https://github.com/logos-messaging/logos-delivery-rust-bindings

---

## Issue #2: Implement proper SDS for reliable task delivery

**Labels:** `enhancement`, `reliability`, `priority:high`

Replace the minimal-SDS implementation with the full Scalable Data Sync protocol.

**Context:**
The current "minimal-SDS" provides:
- UUID-based message deduplication (HashSet, not bloom filter)
- Simple ACK/retransmit (10s timeout, 3 retries)
- No ordering guarantees

**The full SDS spec adds:**
- Bloom filter-based deduplication (space-efficient)
- Causal ordering of messages
- Negotiated sync windows
- Efficient batch acknowledgements

**Tasks:**
- [ ] Study the SDS specification
- [ ] Implement bloom filter deduplication
- [ ] Add causal ordering for task state transitions
- [ ] Implement batch ACK for multiple messages
- [ ] Benchmark throughput vs minimal-SDS

**Reference:** https://blog.waku.org/explanation-series-a-unified-stack-for-scalable-and-reliable-p2p-communication/

---

## Issue #3: Waku symmetric encryption for task privacy

**Labels:** `enhancement`, `security`, `privacy`

Encrypt task payloads with per-conversation symmetric keys derived via ECDH.

**Context:**
Currently, all messages are plaintext JSON on Waku content topics. Anyone monitoring the relay network can read task contents.

**Approach:**
1. When Agent A wants to communicate with Agent B, perform ECDH key agreement:
   - A uses their secp256k1 private key + B's public key → shared secret
   - Derive a symmetric key via HKDF
2. Encrypt task payloads with AES-256-GCM using the derived symmetric key
3. Include a nonce/IV in the envelope
4. Only the sender and recipient can decrypt

**Tasks:**
- [ ] Implement ECDH key derivation using `k256` crate
- [ ] Add AES-256-GCM encryption/decryption for task payloads
- [ ] Update `A2AEnvelope` to support encrypted variants
- [ ] Key rotation strategy for long-running conversations

---

## Issue #4: LEZ agent registry (on-chain AgentCards)

**Labels:** `enhancement`, `discovery`, `blockchain`

Store AgentCards in a LEZ program for permanent, censorship-resistant discovery.

**Context:**
Current discovery relies on ephemeral Waku messages — if no agents are broadcasting, discovery returns nothing. An on-chain registry provides:
- Permanent agent registration
- Verifiable identity (pubkey on-chain)
- Rich metadata (capabilities, version, endpoints)
- Decentralized — no single point of failure

**Tasks:**
- [ ] Define LEZ program interface for agent registration
- [ ] Implement `LezRegistryTransport` that reads AgentCards from chain
- [ ] Hybrid discovery: check LEZ first, fall back to Waku broadcast
- [ ] CLI command: `logos-messaging-a2a agent register` (writes to LEZ)

**Link:** https://github.com/jimmy-claw/lez-registry

---

## Issue #5: Logos Core module (.so)

**Labels:** `enhancement`, `integration`, `logos-core`

Package logos-messaging-a2a-node as a Logos Core `IComponent` plugin for fleet management via the desktop app.

**Context:**
Logos Core uses a plugin architecture where `.so` modules implement `IComponent`. A logos-messaging-a2a plugin would:
- Manage a fleet of agents from the desktop UI
- Expose agent status, task history, discovery via QML
- Integrate with Logos Core's existing Waku stack

**Tasks:**
- [ ] Define `IComponent` interface for logos-messaging-a2a
- [ ] Expose `WakuA2ANode` lifecycle via C FFI
- [ ] Create QML UI for agent management
- [ ] Integration tests with Logos Core runtime

---

## Issue #6: MCP bridge

**Labels:** `enhancement`, `interop`, `mcp`

Expose Waku-connected agents as MCP (Model Context Protocol) tools.

**Context:**
MCP is the standard for connecting AI models to external tools. An MCP bridge would make logos-messaging-a2a agents accessible from:
- Claude (via Claude Code or desktop)
- Cursor
- Any MCP-compatible AI client

**Approach:**
- MCP server that wraps `WakuA2ANode`
- Each discovered agent becomes an MCP tool
- Tool calls translate to A2A tasks
- Responses stream back via MCP

**Tasks:**
- [ ] Implement MCP server using `mcp-rust-sdk` or similar
- [ ] Map A2A AgentCards → MCP tool definitions
- [ ] Map MCP tool calls → A2A tasks
- [ ] Handle async task completion → MCP responses
- [ ] Test with Claude Code as MCP client
