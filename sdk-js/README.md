# GARP JavaScript/TypeScript SDK

Lightweight SDK for interacting with a GARP participant node via JSON-RPC.

## Install

This repo vendors the SDK source under `sdk-js/`. Build outputs to `dist`:

```
npm ci
npm run build
```

Requires Node >= 18 for global `fetch`.

## Usage

```ts
import { GarpClient } from "@garp/sdk-js";

const client = new GarpClient("http://localhost:8080", { timeoutMs: 8000, retries: 2 });

const slot = await client.getSlot();
const leader = await client.getSlotLeader();
const block = await client.getBlockBySlot(slot);
const health = await client.getHealth();
```

### Transactions

```ts
// Raw-serialized transaction (format depends on your app)
const txId = await client.sendTransactionRaw("<hex-or-base64>");

const sim = await client.simulateTransactionRaw("<hex-or-base64>");
if (!sim.ok) console.error(sim.error);
```

### Balances

```ts
const balance = await client.getBalance("<walletAddressHex>"); // string | number

// Batch example
const [slot, leader] = await client.batch([
  { method: "getSlot" },
  { method: "getSlotLeader" },
]);
```

## Methods

- `getSlot()` → `number`
- `getSlotLeader()` → `string`
- `getBlockBySlot(slot)` → `BlockInfo | null`
- `getBlockByHash(hashHex)` → `BlockInfo | null`
- `getTransaction(txIdHex)` → `TransactionInfo | null`
- `sendTransactionRaw(serialized)` → `string` (txId)
- `simulateTransactionRaw(serialized)` → `{ ok: boolean; logs?: string[]; error?: string }`
- `getBalance(addressHex)` → `bigint | number`
- `getVersion()` → `string`
- `getHealth()` → `string`

## Notes

- SDK assumes a JSON-RPC endpoint at `POST /rpc`.
- Response shapes may evolve with node implementation; types here are intentionally minimal.