# LMAO — Logos Module for Agent Orchestration

> **LMAO** = **L**ogos **M**odule for **A**gent **O**rchestration
>
> Yes, the acronym is intentional. Building decentralized AI agent infrastructure is serious work — but it doesn't have to be humourless. LMAO implements Google's [A2A protocol](https://github.com/google/A2A) over [Waku](https://waku.org/) decentralized transport, bringing censorship-resistant, serverless agent-to-agent communication to the Logos stack.

## The Problem

Google's A2A protocol is great. But it assumes HTTP: stable endpoints, central registries, easy censorship. That's fine for web2. For a decentralized agent network running on Logos, it's a non-starter.

**LMAO** replaces HTTP with Waku — a decentralized pub/sub network — giving you full A2A semantics with:

| | HTTP/SSE | LMAO (Waku) |
|---|---|---|
| Discovery | Central registry | Content-addressed pub/sub topics |
| Endpoints | Stable IP required | Just a pubkey |
| Privacy | Traffic analysis easy | Optional E2E encryption |
| Censorship | Single point of failure | Decentralized relay |
| NAT | Needs port forwarding | Works behind NAT |

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                  Waku Relay Network                  │
│                                                      │
│  /lmao/1/discovery/proto    ← AgentCard broadcasts  │
│  /lmao/1/task/{pubkey}/proto ← Task inbox per agent │
│  /lmao/1/ack/{msg_id}/proto  ← SDS acknowledgements │
└──────────┬──────────────┬──────────────┬─────────────┘
           │              │              │
      ┌────▼────┐    ┌───▼────┐    ┌───▼────┐
      │ Agent A │    │ Agent B│    │ Agent C│
      │ (echo)  │    │ (code) │    │(search)│
      └─────────┘    └────────┘    └────────┘
```

## Encryption

End-to-end encrypted using **X25519 ECDH + ChaCha20-Poly1305** (stepping stone). Future: [Logos Chat SDK](https://github.com/nicola/logos-chat-sdk) with Double Ratchet for forward secrecy.

## Quick Start

```bash
# Ping-pong demo (no nwaku needed — fully in-memory)
cargo run --example ping_pong

# With encryption
cargo run --example ping_pong -- --encrypt
```

## Project Structure

```
lmao/
  crates/
    waku-a2a-crypto/     # X25519 + ChaCha20-Poly1305
    waku-a2a-core/       # A2A types: AgentCard, Task, Message, Part
    waku-a2a-transport/  # Transport trait + nwaku REST + SDS reliability layer
    waku-a2a-node/       # A2A node: announce, discover, send/receive
    waku-a2a-cli/        # CLI
  examples/
    ping_pong.rs         # Two agents exchanging tasks
    echo_agent.rs        # Simple echo agent
```

## Roadmap

1. **libwaku FFI** — replace nwaku REST with embedded libwaku (no separate process)
2. **Full SDS protocol** — bloom filters, causal ordering, batch ACK
3. **Logos Chat SDK** — Double Ratchet for forward secrecy (replacing static ECDH)
4. **LEZ agent registry** — on-chain AgentCards via SPELbook for permanent discovery
5. **Logos Core plugin** — `.lgx` module with QML agent fleet UI
6. **MCP bridge** — expose agents as MCP tools for Claude, Cursor, etc.

## Part of the SPEL ecosystem

| Repo | Description |
|------|-------------|
| [spel](https://github.com/jimmy-claw/spel) | Smart Program Execution Layer — LEZ framework |
| [spelbook](https://github.com/jimmy-claw/spelbook) | On-chain program registry |
| [lez-multisig-framework](https://github.com/jimmy-claw/lez-multisig-framework) | Multisig governance |
| [lmao](https://github.com/jimmy-claw/lmao) | This repo |

## License

MIT
