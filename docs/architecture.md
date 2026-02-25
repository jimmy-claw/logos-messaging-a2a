# logos-messaging-a2a Architecture

## Full Stack Diagram

```
┌──────────────────────────────────────────────────────────────────────┐
│                        Application Layer                             │
│                                                                      │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐            │
│  │  logos-messaging-a2a-cli│   │  echo_agent  │   │  ping_pong   │            │
│  │  (CLI binary) │   │  (example)   │   │  (example)   │            │
│  └──────┬───────┘   └──────┬───────┘   └──────┬───────┘            │
│         │                  │                   │                     │
│         └──────────────────┼───────────────────┘                    │
│                            │                                         │
├────────────────────────────┼─────────────────────────────────────────┤
│                     Node Layer                                       │
│                                                                      │
│  ┌─────────────────────────┴──────────────────────────────┐         │
│  │                 WakuA2ANode<T>                          │         │
│  │                                                         │         │
│  │  • announce()     — broadcast AgentCard                 │         │
│  │  • discover()     — find agents on network              │         │
│  │  • send_task()    — send task with SDS reliability      │         │
│  │  • poll_tasks()   — receive incoming tasks              │         │
│  │  • respond()      — reply to a task                     │         │
│  │                                                         │         │
│  │  Identity: secp256k1 keypair                            │         │
│  └─────────────────────────┬──────────────────────────────┘         │
│                            │                                         │
├────────────────────────────┼─────────────────────────────────────────┤
│                  Reliability Layer (minimal-SDS)                      │
│                                                                      │
│  ┌─────────────────────────┴──────────────────────────────┐         │
│  │              SdsTransport<T: WakuTransport>             │         │
│  │                                                         │         │
│  │  • publish_reliable() — retransmit up to 3x             │         │
│  │  • send_ack()         — acknowledge receipt             │         │
│  │  • poll_dedup()       — deduplicate by message ID       │         │
│  │  • is_duplicate()     — bloom filter (HashSet in v0.1)  │         │
│  │                                                         │         │
│  │  ACK timeout: 10s | Max retries: 3                      │         │
│  └─────────────────────────┬──────────────────────────────┘         │
│                            │                                         │
├────────────────────────────┼─────────────────────────────────────────┤
│                  Transport Layer (swappable)                          │
│                                                                      │
│  ┌─────────────────────────┴──────────────────────────────┐         │
│  │            trait WakuTransport                          │         │
│  │                                                         │         │
│  │  • publish(topic, payload)                              │         │
│  │  • subscribe(topic)                                     │         │
│  │  • poll(topic) -> Vec<Vec<u8>>                          │         │
│  │                                                         │         │
│  ├─────────────────────────────────────────────────────────┤         │
│  │                                                         │         │
│  │  NwakuRestTransport        LogosDeliveryTransport       │         │
│  │  (v0.1 — REST fallback)    (TODO — FFI via libwaku)     │         │
│  │  http://localhost:8645     waku-bindings crate           │         │
│  │                                                         │         │
│  └─────────────────────────┬──────────────────────────────┘         │
│                            │                                         │
├────────────────────────────┼─────────────────────────────────────────┤
│                     Waku Network                                     │
│                                                                      │
│  ┌─────────────────────────┴──────────────────────────────┐         │
│  │              Waku Relay (pub/sub)                        │         │
│  │                                                         │         │
│  │  Content Topics:                                        │         │
│  │  /logos-messaging-a2a/1/discovery/proto     AgentCard broadcasts   │         │
│  │  /logos-messaging-a2a/1/task/{pubkey}/proto Task inbox per agent   │         │
│  │  /logos-messaging-a2a/1/ack/{msg_id}/proto  SDS acknowledgements   │         │
│  │                                                         │         │
│  └─────────────────────────────────────────────────────────┘         │
│                                                                      │
│  ┌─────────────────────────────────────────────────────────┐         │
│  │  nwaku node (relay, store, filter)                      │         │
│  │  OR embedded libwaku via logos-delivery-rust-bindings    │         │
│  └─────────────────────────────────────────────────────────┘         │
└──────────────────────────────────────────────────────────────────────┘
```

## A2A Types (logos-messaging-a2a-core)

```
AgentCard
├── name: String
├── description: String
├── version: String
├── capabilities: Vec<String>
└── public_key: String          (secp256k1 compressed hex)

Task
├── id: String                  (UUID v4)
├── from: String                (sender pubkey)
├── to: String                  (recipient pubkey)
├── state: TaskState            (Submitted → Working → Completed/Failed)
├── message: Message
│   ├── role: String            ("user" or "agent")
│   └── parts: Vec<Part>
│       └── Part::Text { text }
└── result: Option<Message>     (agent's response)

A2AEnvelope (wire format)
├── AgentCard(AgentCard)
├── Task(Task)
└── Ack { message_id }
```

## Message Flow

```
Agent A                    Waku Network                  Agent B
  │                            │                            │
  │── announce(AgentCard) ────▶│ /discovery/proto           │
  │                            │◀── announce(AgentCard) ────│
  │                            │                            │
  │── discover() ─────────────▶│                            │
  │◀── [AgentCard B] ─────────│                            │
  │                            │                            │
  │── send_task(Task) ────────▶│ /task/{B.pubkey}/proto     │
  │                            │──────── poll_tasks() ─────▶│
  │                            │                            │
  │   (SDS: wait for ACK)      │◀── send_ack(task.id) ─────│
  │◀── ACK on /ack/{id}/proto─│                            │
  │                            │                            │
  │                            │◀── respond(result) ───────│
  │◀── poll_tasks() ──────────│ /task/{A.pubkey}/proto     │
  │                            │                            │
```

## Crate Dependency Graph

```
logos-messaging-a2a (root)
├── logos-messaging-a2a-core          (no internal deps)
├── logos-messaging-a2a-transport     (no internal deps)
├── logos-messaging-a2a-node          (depends on core + transport)
└── logos-messaging-a2a-cli           (depends on core + transport + node)
```
