import { JsonRpcRequest, JsonRpcResponse, BlockInfo, TransactionInfo } from "./types";

export interface GarpClientOptions {
  /** Request timeout in milliseconds */
  timeoutMs?: number;
  /** Custom fetch implementation (Node >=18 has global fetch) */
  fetchImpl?: typeof fetch;
  /** Number of retries on network errors */
  retries?: number;
  /** Delay between retries in milliseconds */
  retryDelayMs?: number;
  /** Base URL for REST API (gateway '/api') used by chat methods */
  apiBaseUrl?: string;
}

export class RpcError extends Error {
  code: number;
  data?: unknown;
  constructor(code: number, message: string, data?: unknown) {
    super(`RPC ${code}: ${message}`);
    this.code = code;
    this.data = data;
  }
}

export class GarpClient {
  private readonly baseUrl: string;
  private readonly timeoutMs: number;
  private readonly fetchImpl: typeof fetch;
  private readonly retries: number;
  private readonly retryDelayMs: number;
  private readonly apiBaseUrl?: string;
  private requestId = 1;

  constructor(baseUrl: string, opts: GarpClientOptions = {}) {
    this.baseUrl = baseUrl.replace(/\/$/, "");
    this.timeoutMs = opts.timeoutMs ?? 10000;
    this.fetchImpl = opts.fetchImpl ?? fetch;
    this.retries = opts.retries ?? 0;
    this.retryDelayMs = opts.retryDelayMs ?? 250;
    this.apiBaseUrl = opts.apiBaseUrl?.replace(/\/$/, "");
  }

  // Core RPC request helper
  private async rpc<Result = unknown>(method: string, params?: unknown): Promise<Result> {
    const id = this.requestId++;
    const body: JsonRpcRequest = { jsonrpc: "2.0", id, method, params };

    const controller = new AbortController();
    const t = setTimeout(() => controller.abort(), this.timeoutMs);
    let attempt = 0;
    try {
      while (true) {
        try {
          const res = await this.fetchImpl(`${this.baseUrl}/rpc`, {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify(body),
            signal: controller.signal,
          });
          const json = (await res.json()) as JsonRpcResponse<Result>;
          if ("error" in json) {
            const e = json.error;
            throw new RpcError(e.code, e.message, e.data);
          }
          return (json as any).result as Result;
        } catch (err) {
          // Only retry on network/timeout errors
          if (attempt < this.retries && (err instanceof TypeError || (err as any).name === "AbortError")) {
            attempt++;
            await new Promise((r) => setTimeout(r, this.retryDelayMs));
            continue;
          }
          throw err;
        }
      }
    } finally {
      clearTimeout(t);
    }
  }

  // Minimal REST helper for chat endpoints via gateway '/api'
  private async rest<Result = unknown>(path: string, init?: { method?: string; body?: unknown; headers?: Record<string, string> }): Promise<Result> {
    if (!this.apiBaseUrl) throw new Error("apiBaseUrl not set: provide opts.apiBaseUrl in GarpClient constructor");
    const url = `${this.apiBaseUrl}${path.startsWith("/") ? path : `/${path}`}`;
    const controller = new AbortController();
    const t = setTimeout(() => controller.abort(), this.timeoutMs);
    try {
      const res = await this.fetchImpl(url, {
        method: init?.method ?? "GET",
        headers: { "content-type": "application/json", ...(init?.headers ?? {}) },
        body: init?.body ? JSON.stringify(init.body) : undefined,
        signal: controller.signal,
      });
      if (!res.ok) {
        const err = await res.json().catch(() => ({}));
        throw new Error(`HTTP ${res.status}: ${err.error ?? res.statusText}`);
      }
      return (await res.json()) as Result;
    } finally {
      clearTimeout(t);
    }
  }

  // Timing & consensus
  async getSlot(): Promise<number> {
    return this.rpc<number>("getSlot");
  }

  async getSlotLeader(): Promise<string> {
    return this.rpc<string>("getSlotLeader");
  }

  // Blocks
  async getBlockBySlot(slot: number): Promise<BlockInfo | null> {
    return this.rpc<BlockInfo | null>("getBlock", [slot]);
  }

  async getBlockByHash(hashHex: string): Promise<BlockInfo | null> {
    return this.rpc<BlockInfo | null>("getBlock", [hashHex]);
  }

  // Transactions
  async getTransaction(txIdHex: string): Promise<TransactionInfo | null> {
    return this.rpc<TransactionInfo | null>("getTransaction", [txIdHex]);
  }

  async sendTransactionRaw(txSerialized: string): Promise<string> {
    // returns transaction id (hex) upon success
    return this.rpc<string>("sendTransaction", [txSerialized]);
  }

  async simulateTransactionRaw(txSerialized: string): Promise<{ ok: boolean; logs?: string[]; error?: string }>{
    return this.rpc<{ ok: boolean; logs?: string[]; error?: string }>("simulateTransaction", [txSerialized]);
  }

  // Wallets
  async getBalance(addressHex: string): Promise<string | number> {
    // JSON cannot carry BigInt; nodes may encode balances as string for safety.
    return this.rpc<string | number>("getBalance", [addressHex]);
  }

  // Node info
  async getVersion(): Promise<string> {
    return this.rpc<string>("getVersion");
  }

  async getHealth(): Promise<string> {
    return this.rpc<string>("getHealth");
  }

  // Batch requests: [{ method, params }] => array of results in order
  async batch(calls: Array<{ method: string; params?: unknown }>): Promise<unknown[]> {
    const idStart = this.requestId;
    this.requestId += calls.length;
    const payload = calls.map((c, i) => ({ jsonrpc: "2.0", id: idStart + i, method: c.method, params: c.params }));
    const controller = new AbortController();
    const t = setTimeout(() => controller.abort(), this.timeoutMs);
    try {
      const res = await this.fetchImpl(`${this.baseUrl}/rpc`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(payload),
        signal: controller.signal,
      });
      const json = (await res.json()) as Array<JsonRpcResponse<unknown>>;
      return json.map((item) => {
        if ("error" in item) throw new RpcError(item.error.code, item.error.message, item.error.data);
        return (item as any).result;
      });
    } finally {
      clearTimeout(t);
    }
  }

  // --- Chat (REST) ---
  async getPublicKey(addressHex: string): Promise<{ address: string; public_key: string }>{
    return this.rest(`/keys/${addressHex}`);
  }

  async sendMessage(input: { sender: string; recipient: string; ciphertext: string; nonce: string }): Promise<{ id: number; hash: string; created_at: string }>{
    const body = { sender: input.sender, recipient: input.recipient, content_ciphertext: input.ciphertext, content_nonce: input.nonce };
    return this.rest(`/messages`, { method: "POST", body });
  }

  async listMessages(params: { address: string; peer: string; since?: string; limit?: number }): Promise<Array<{ id: number; sender: string; recipient: string; content_ciphertext: string; content_nonce: string; hash: string; created_at: string; anchored: boolean; block_hash?: string | null; block_number?: number | null }>>{
    const q = new URLSearchParams({ address: params.address, peer: params.peer });
    if (params.since) q.set("since", params.since);
    if (params.limit && params.limit > 0) q.set("limit", String(params.limit));
    return this.rest(`/messages?${q.toString()}`);
  }

  // --- P2P Signaling ---
  async sendSignal(input: { from: string; to: string; type: string; payload: Record<string, unknown> }): Promise<{ success: boolean }>{
    return this.rest(`/signals`, { method: "POST", body: input });
  }

  // Browser-only: open SSE stream for signals addressed to `addressHex`.
  // Returns native EventSource; for Node, use a polyfill like 'eventsource'.
  openSignalStream(addressHex: string): EventSource {
    if (!this.apiBaseUrl) throw new Error("apiBaseUrl not set");
    const url = `${this.apiBaseUrl}/stream/signals?address=${encodeURIComponent(addressHex)}`;
    return new EventSource(url);
  }
}