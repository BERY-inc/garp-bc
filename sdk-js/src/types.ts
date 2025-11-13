export type JsonRpcVersion = "2.0";

export interface JsonRpcRequest<Params = unknown> {
  jsonrpc: JsonRpcVersion;
  id: number | string;
  method: string;
  params?: Params;
}

export interface JsonRpcError {
  code: number;
  message: string;
  data?: unknown;
}

export interface JsonRpcSuccess<Result = unknown> {
  jsonrpc: JsonRpcVersion;
  id: number | string;
  result: Result;
}

export interface JsonRpcFailure {
  jsonrpc: JsonRpcVersion;
  id: number | string;
  error: JsonRpcError;
}

export type JsonRpcResponse<Result = unknown> = JsonRpcSuccess<Result> | JsonRpcFailure;

// Minimal shapes based on participant-node RPC responses
export interface BlockTx {
  id: string;
  submitter: string;
  commandType?: string; // optional summary
}

export interface BlockInfo {
  slot: number;
  hash: string;
  parentHash?: string;
  timestampMs?: number;
  leader?: string;
  transactions?: BlockTx[];
}

export interface TransactionInfo {
  id: string;
  submitter?: string;
  status?: string;
  createdAt?: number;
  error?: string;
}