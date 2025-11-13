# GARP Chat API (v1)

This document specifies the minimal, stable REST endpoints to build chat applications on the GARP blockchain stack. The API uses the gateway path `'/api'` which proxies to the backend service. All endpoints return JSON.

## Base URLs

- Gateway (recommended): `https://<gateway-host>/api`
- Direct backend (alternative): `https://<backend-host>/` then prefix paths below with `/` (no `/api` prefix).

Note: The gateway handles CORS and rate limiting and is the preferred public surface.

## Endpoints

### Create Message

- `POST /messages`
- Request:
  - `sender` (string, hex address)
  - `recipient` (string, hex address)
  - `content_ciphertext` (string, application-level ciphertext)
  - `content_nonce` (string, nonce used in encryption)
- Response:
  - `id` (number, internal ID)
  - `hash` (string, SHA-256 of message envelope)
  - `created_at` (string, RFC3339 timestamp)

### List Messages

- `GET /messages?address=<addr>&peer=<addr>&since=<RFC3339>&limit=<int>`
- Query:
  - `address` (required): one party address
  - `peer` (required): the other party address
  - `since` (optional): RFC3339 timestamp lower bound
  - `limit` (optional, default 100): max number of messages
- Response: array of Message objects:
  - `id` (number)
  - `sender` (string)
  - `recipient` (string)
  - `content_ciphertext` (string)
  - `content_nonce` (string)
  - `hash` (string)
  - `created_at` (string, RFC3339)
  - `anchored` (boolean)
  - `block_hash` (string|null)
  - `block_number` (number|null)

### Anchor Message (optional)

- `POST /messages/:id/anchor`
- Request:
  - `block` (number): block number to mark anchoring
- Response:
  - `id` (number)
  - `anchored_at_block` (number)

### Stream New Messages (SSE)

- `GET /stream/messages`
- Response: Server-Sent Events stream
  - `event: heartbeat` then `data: ok` on connect
  - `event: message` with `data: { id, sender, recipient, hash, created_at }`

### P2P Signaling (WebRTC/D2D)

These endpoints provide a simple signaling bus for peers to exchange session descriptions and ICE candidates. Signals are ephemeral and delivered via Redis Pub/Sub; no content is persisted.

- `POST /signals`
  - Request:
    - `from` (string): sender address
    - `to` (string): recipient address
    - `type` (string): e.g. `offer`, `answer`, `ice`
    - `payload` (object): user-defined payload (SDP, ICE candidate, etc.)
  - Response:
    - `{ "success": true }`

- `GET /stream/signals?address=<addr>`
  - Response: Server-Sent Events
    - `event: heartbeat` followed by `data: ok`
    - `event: signal` with `data: { from, to, type, payload, timestamp }`

Example (JS):

```ts
// Send signal
await client.sendSignal({ from, to, type: "offer", payload: { sdp } });

// Receive signals via SSE
const es = new EventSource(`${apiBaseUrl}/stream/signals?address=${myAddr}`);
es.addEventListener("signal", (ev) => {
  const msg = JSON.parse(ev.data);
  if (msg.type === "answer") { /* setRemoteDescription */ }
  if (msg.type === "ice") { /* addIceCandidate */ }
});
```

### Discover Public Key

- `GET /keys/:addr`
- Response:
  - `address` (string)
  - `public_key` (string) — placeholder until wallet integration is wired

## Status Codes & Errors

- `200 OK`: successful operation
- `400 Bad Request`: invalid input
- `404 Not Found`: missing resource
- `429 Too Many Requests`: rate limit exceeded (gateway)
- `500 Internal Server Error`: server-side error
- `503 Service Unavailable`: storage or stream temporarily unavailable

## Notes on Encryption & Privacy

- The API treats `content_ciphertext` and `content_nonce` as opaque. Perform E2EE in the client using your preferred scheme (e.g., X25519 + ChaCha20-Poly1305).
- `hash` is computed over a normalized envelope; clients may verify integrity off-chain.
- Use the SSE stream for real-time notifications and fetch full content via `GET /messages`.

## Example Usage

### cURL

```bash
curl -X POST "https://gateway.example.com/api/messages" \
  -H "content-type: application/json" \
  -d '{
    "sender": "0xabc...",
    "recipient": "0xdef...",
    "content_ciphertext": "<base64-or-utf8>",
    "content_nonce": "<nonce>"
  }'

curl "https://gateway.example.com/api/messages?address=0xabc...&peer=0xdef...&limit=50"
```

### JS SDK

```ts
import { GarpClient } from "@garp/sdk-js";

const client = new GarpClient("https://participant.example.com", { apiBaseUrl: "https://gateway.example.com/api" });

await client.sendMessage({ sender, recipient, ciphertext, nonce });
const msgs = await client.listMessages({ address: sender, peer: recipient, limit: 50 });
const key = await client.getPublicKey(sender);
```

### Go SDK

```go
chat := garp.NewChatClient("https://gateway.example.com/api", nil)
resp, _ := chat.SendMessage(garp.SendMessageRequest{Sender: "0xabc", Recipient: "0xdef", Ciphertext: "...", Nonce: "..."})
msgs, _ := chat.ListMessages("0xabc", "0xdef", "", 100)
```

### Python SDK

```py
from garp_sdk.chat import ChatClient

chat = ChatClient(base_url="https://gateway.example.com/api")
chat.send_message(sender="0xabc", recipient="0xdef", ciphertext="...", nonce="...")
msgs = chat.list_messages(address="0xabc", peer="0xdef", limit=100)
```

## Versioning

This is `v1`. Future changes will be additive. Introduce breaking changes via new paths (`/api/v2/...`).