# logos-messaging-a2a

**A2A (Agent2Agent) protocol over Waku decentralized transport.**

Google's [A2A protocol](https://github.com/google/A2A) defines a standard for agent-to-agent communication — Agent Cards, Tasks, Messages, and Parts. But it assumes HTTP transport: agents need stable endpoints, centralized discovery, and are vulnerable to censorship.

**logos-messaging-a2a** replaces HTTP with [Waku](https://waku.org/), a decentralized pub/sub network. This gives you A2A semantics with censorship resistance, no central registry, and no need for stable endpoints.

## Why not HTTP?

| Problem | HTTP/SSE | Waku |
|---------|----------|------|
| Discovery | Central registry required | Content-addressed pub/sub topics |
| Endpoints | Stable IP/domain needed | No endpoint needed — just a pubkey |
| Privacy | Traffic analysis easy | Optional encryption, relay mixing |
| Censorship | Single point of failure | Decentralized relay network |
| NAT/Firewall | Needs port forwarding | Works behind NAT |

## Why logos-delivery-rust-bindings?

The [logos-delivery-rust-bindings](https://github.com/logos-messaging/logos-delivery-rust-bindings) crate (`waku-bindings`) provides a Rust FFI wrapper around libwaku. This means:

- **No separate process**: libwaku embeds directly into the Rust binary
- **Logos Core compatible**: same FFI used by the Logos desktop app
- **Full protocol access**: relay, store, filter, lightpush

> **v0.1 note:** The FFI requires a Nim toolchain to compile `libwaku.so`. For this prototype, we use the nwaku REST API as a fallback. See [Issue #1](docs/issues.md) for the FFI migration plan.

## SDS: Reliable Delivery

Waku is fire-and-forget — no delivery guarantees. For agent tasks, we need reliability.

waku-a2a implements a **minimal SDS** (Scalable Data Sync) inspired layer:

- Each message has a UUID
- Sender polls for ACK on `/logos-messaging-a2a/1/ack/{message_id}/proto`
- Receiver sends ACK after processing
- No ACK within 10s → retransmit (up to 3 times)
- Deduplication via message ID tracking

See [Issue #2](docs/issues.md) for the full SDS spec migration plan.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Waku Relay Network                        │
│                                                             │
│  /logos-messaging-a2a/1/discovery/proto      AgentCard broadcasts      │
│  /logos-messaging-a2a/1/task/{pubkey}/proto  Task inbox per agent      │
│  /logos-messaging-a2a/1/ack/{msg_id}/proto   SDS acknowledgements      │
│                                                             │
└─────────┬────────────────┬────────────────┬─────────────────┘
          │                │                │
     ┌────▼────┐      ┌───▼────┐      ┌───▼────┐
     │ Agent A │      │ Agent B│      │ Agent C│
     │ (echo)  │      │ (code) │      │(search)│
     └─────────┘      └────────┘      └────────┘

Each agent:
  1. Generates a secp256k1 keypair (identity)
  2. Announces its AgentCard to the discovery topic
  3. Listens on /task/{its_pubkey}/proto for incoming tasks
  4. Publishes responses to /task/{sender_pubkey}/proto
  5. SDS layer handles ACK/retransmit for reliability
```

See [docs/architecture.md](docs/architecture.md) for the full stack diagram.

## Project Structure

```
logos-messaging-a2a/
  Cargo.toml                # workspace root
  crates/
    waku-a2a-core/          # A2A types: AgentCard, Task, Message, Part, TaskState
    waku-a2a-transport/     # Transport trait + nwaku REST impl + minimal SDS
    waku-a2a-node/          # A2A node: announce, discover, send/receive tasks
    waku-a2a-cli/           # CLI: run agents, discover peers, send tasks
  examples/
    echo_agent.rs           # Simple agent that echoes back messages
    ping_pong.rs            # Two agents exchanging tasks (in-memory transport)
  docs/
    architecture.md         # ASCII diagram of the full stack
    issues.md               # Pre-written GitHub issues for follow-up work
```

## Quick Start

### Prerequisites

A running [nwaku](https://github.com/waku-org/nwaku) node with REST API enabled (for the echo agent / CLI — the ping-pong example runs fully in-memory):

```bash
# Option 1: Docker
docker run -p 8645:8645 statusteam/nim-waku:v0.31.0 \
  --rest --rest-address=0.0.0.0 --rest-port=8645

# Option 2: nwaku-compose
git clone https://github.com/waku-org/nwaku-compose && cd nwaku-compose
docker compose up -d
```

### Build

```bash
cargo build
```

### Run Ping-Pong Demo (no nwaku needed)

```bash
cargo run --example ping_pong
```

Output:
```
=== Ping-Pong Demo ===

Ping agent: ping (02a1b2c3d4e5f6...)
Pong agent: pong (03f6e5d4c3b2a1...)

Ping discovered 1 agent(s)
  -> pong (03f6e5d4c3b2a1...)

[ping] Sending: "Ping!" (task 550e8400)
[pong] Received: "Ping!" (task 550e8400)
[pong] Replied: "Pong! (reply to: Ping!)"
[ping] Got response: "Pong! (reply to: Ping!)"

Done! Both agents communicated via in-memory Waku transport.
```

### Run the Echo Agent (requires nwaku)

```bash
# Terminal 1: start the echo agent
cargo run --example echo_agent

# Terminal 2: send a task to it
cargo run --bin logos-messaging-a2a -- task send \
  --to <agent_pubkey> \
  --text "Hello, what can you do?"

# Terminal 2: discover agents
cargo run --bin logos-messaging-a2a -- agent discover
```

### CLI Reference

```bash
logos-messaging-a2a agent run --name "echo" --capabilities text
logos-messaging-a2a agent discover
logos-messaging-a2a task send --to <pubkey> --text "Hello"
logos-messaging-a2a task status --id <uuid>
```

## A2A Protocol Types

- **AgentCard**: Identity (secp256k1 pubkey), capabilities, version, description
- **Task**: UUID, sender/recipient pubkeys, state machine (Submitted → Working → Completed/Failed), message + optional result
- **Message**: Role ("user" or "agent") + list of Parts
- **Part**: Text (v0.1), extensible to images/files
- **TaskState**: `submitted | working | input_required | completed | failed | cancelled`

## Roadmap

See [docs/issues.md](docs/issues.md) for pre-written GitHub issues:

1. **logos-delivery-rust-bindings FFI** — replace nwaku REST with embedded libwaku
2. **Full SDS protocol** — proper bloom filters, causal ordering, batch ACK
3. **Symmetric encryption** — per-conversation ECDH-derived keys
4. **LEZ agent registry** — on-chain AgentCards for permanent discovery
5. **Logos Core plugin** — `.so` module with QML agent fleet UI
6. **MCP bridge** — expose agents as MCP tools for Claude, Cursor, etc.

## License

MIT
