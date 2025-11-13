from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict, List, Optional

import httpx


@dataclass
class Message:
    id: int
    sender: str
    recipient: str
    content_ciphertext: str
    content_nonce: str
    hash: str
    created_at: str
    anchored: bool
    block_hash: Optional[str] = None
    block_number: Optional[int] = None


class ChatClient:
    """
    REST client for chat endpoints via gateway '/api'.
    """

    def __init__(self, base_url: str, timeout: float = 10.0):
        self.base_url = base_url.rstrip("/")
        self.client = httpx.Client(timeout=timeout)

    def close(self) -> None:
        self.client.close()

    def send_message(self, sender: str, recipient: str, ciphertext: str, nonce: str) -> Dict[str, Any]:
        body = {
            "sender": sender,
            "recipient": recipient,
            "content_ciphertext": ciphertext,
            "content_nonce": nonce,
        }
        resp = self.client.post(f"{self.base_url}/messages", json=body)
        resp.raise_for_status()
        return resp.json()

    def list_messages(self, address: str, peer: str, since: Optional[str] = None, limit: Optional[int] = None) -> List[Message]:
        params: Dict[str, Any] = {"address": address, "peer": peer}
        if since:
            params["since"] = since
        if limit and limit > 0:
            params["limit"] = limit
        resp = self.client.get(f"{self.base_url}/messages", params=params)
        resp.raise_for_status()
        arr = resp.json()
        return [Message(**item) for item in arr]

    def get_public_key(self, addr: str) -> Dict[str, str]:
        resp = self.client.get(f"{self.base_url}/keys/{addr}")
        resp.raise_for_status()
        return resp.json()

    def send_signal(self, from_addr: str, to_addr: str, type: str, payload: Dict[str, Any]) -> Dict[str, Any]:
        body = {"from": from_addr, "to": to_addr, "type": type, "payload": payload}
        resp = self.client.post(f"{self.base_url}/signals", json=body)
        resp.raise_for_status()
        return resp.json()