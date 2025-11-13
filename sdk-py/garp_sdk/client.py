from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Dict, Optional, Type, TypeVar

import httpx


class RpcError(Exception):
    def __init__(self, code: int, message: str):
        self.code = code
        self.message = message
        super().__init__(f"RPC error {code}: {message}")


T = TypeVar("T")


@dataclass
class SimulationResult:
    ok: bool
    logs: Optional[list[str]] = None
    error: Optional[str] = None


def _from_dict(cls: Type[T], data: Dict[str, Any]) -> T:
    # Simple dataclass conversion without 3rd-party libs
    return cls(**data)


class GarpClient:
    def __init__(self, base_url: str, timeout: float = 10.0):
        self.base_url = base_url.rstrip("/")
        self.client = httpx.Client(timeout=timeout)
        self._id = 1

    def _rpc(self, method: str, params: Optional[Any] = None) -> Any:
        body = {"jsonrpc": "2.0", "id": self._id, "method": method}
        self._id += 1
        if params is not None:
            body["params"] = params
        resp = self.client.post(f"{self.base_url}/rpc", json=body)
        data = resp.json()
        if "error" in data:
            raise RpcError(data["error"]["code"], data["error"]["message"])
        return data["result"]

    # Timing & consensus
    def get_slot(self) -> int:
        return self._rpc("getSlot")

    def get_slot_leader(self) -> str:
        return self._rpc("getSlotLeader")

    # Blocks
    def get_block_by_slot(self, slot: int) -> Optional[Dict[str, Any]]:
        return self._rpc("getBlock", [slot])

    def get_block_by_hash(self, hash_hex: str) -> Optional[Dict[str, Any]]:
        return self._rpc("getBlock", [hash_hex])

    # Transactions
    def get_transaction(self, tx_id_hex: str) -> Optional[Dict[str, Any]]:
        return self._rpc("getTransaction", [tx_id_hex])

    def send_transaction_raw(self, serialized: str) -> str:
        return self._rpc("sendTransaction", [serialized])

    def simulate_transaction_raw(self, serialized: str) -> SimulationResult:
        res = self._rpc("simulateTransaction", [serialized])
        return _from_dict(SimulationResult, res)

    # Wallets
    def get_balance(self, address_hex: str) -> Any:
        # Balance may be bigint; return raw JSON value
        return self._rpc("getBalance", [address_hex])

    # Node info
    def get_version(self) -> str:
        return self._rpc("getVersion")

    def get_health(self) -> str:
        return self._rpc("getHealth")

    def close(self) -> None:
        self.client.close()

    def __enter__(self) -> "GarpClient":
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self.close()


class AsyncGarpClient:
    def __init__(self, base_url: str, timeout: float = 10.0):
        self.base_url = base_url.rstrip("/")
        self.client = httpx.AsyncClient(timeout=timeout)
        self._id = 1

    async def _rpc(self, method: str, params: Optional[Any] = None) -> Any:
        body = {"jsonrpc": "2.0", "id": self._id, "method": method}
        self._id += 1
        if params is not None:
            body["params"] = params
        resp = await self.client.post(f"{self.base_url}/rpc", json=body)
        data = resp.json()
        if "error" in data:
            raise RpcError(data["error"]["code"], data["error"]["message"])
        return data["result"]

    # Timing & consensus
    async def get_slot(self) -> int:
        return await self._rpc("getSlot")

    async def get_slot_leader(self) -> str:
        return await self._rpc("getSlotLeader")

    # Blocks
    async def get_block_by_slot(self, slot: int) -> Optional[Dict[str, Any]]:
        return await self._rpc("getBlock", [slot])

    async def get_block_by_hash(self, hash_hex: str) -> Optional[Dict[str, Any]]:
        return await self._rpc("getBlock", [hash_hex])

    # Transactions
    async def get_transaction(self, tx_id_hex: str) -> Optional[Dict[str, Any]]:
        return await self._rpc("getTransaction", [tx_id_hex])

    async def send_transaction_raw(self, serialized: str) -> str:
        return await self._rpc("sendTransaction", [serialized])

    async def simulate_transaction_raw(self, serialized: str) -> SimulationResult:
        res = await self._rpc("simulateTransaction", [serialized])
        return _from_dict(SimulationResult, res)

    # Wallets
    async def get_balance(self, address_hex: str) -> Any:
        return await self._rpc("getBalance", [address_hex])

    # Node info
    async def get_version(self) -> str:
        return await self._rpc("getVersion")

    async def get_health(self) -> str:
        return await self._rpc("getHealth")

    async def aclose(self) -> None:
        await self.client.aclose()

    async def __aenter__(self) -> "AsyncGarpClient":
        return self

    async def __aexit__(self, exc_type, exc, tb) -> None:
        await self.aclose()