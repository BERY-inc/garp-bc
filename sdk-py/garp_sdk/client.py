import json
import requests
from typing import Optional, Dict, Any, Union
from dataclasses import dataclass
from .types import BlockInfo, TransactionInfo

@dataclass
class SimulationResult:
    ok: bool
    logs: Optional[list] = None
    error: Optional[str] = None

class GarpClient:
    def __init__(self, base_url: str, timeout: float = 10.0):
        self.base_url = base_url.rstrip('/')
        self.timeout = timeout
        self.session = requests.Session()
        
    def _rpc(self, method: str, params: Optional[list] = None) -> Any:
        """Make an RPC call to the GARP node."""
        payload = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params or []
        }
        
        response = self.session.post(
            f"{self.base_url}/rpc",
            json=payload,
            timeout=self.timeout
        )
        response.raise_for_status()
        
        result = response.json()
        if "error" in result:
            raise Exception(f"RPC error: {result['error']}")
            
        return result.get("result")
    
    # Timing & consensus
    def get_slot(self) -> int:
        return self._rpc("getSlot")
    
    def get_slot_leader(self) -> str:
        return self._rpc("getSlotLeader")
    
    # Blocks
    def get_block_by_slot(self, slot: int) -> Optional[BlockInfo]:
        result = self._rpc("getBlock", [slot])
        if result is None:
            return None
        return BlockInfo(**result)
    
    def get_block_by_hash(self, hash_hex: str) -> Optional[BlockInfo]:
        result = self._rpc("getBlock", [hash_hex])
        if result is None:
            return None
        return BlockInfo(**result)
    
    # Transactions
    def get_transaction(self, tx_id_hex: str) -> Optional[TransactionInfo]:
        result = self._rpc("getTransaction", [tx_id_hex])
        if result is None:
            return None
        return TransactionInfo(**result)
    
    def send_transaction_raw(self, tx_serialized: str) -> str:
        return self._rpc("sendTransaction", [tx_serialized])
    
    def simulate_transaction_raw(self, tx_serialized: str) -> SimulationResult:
        result = self._rpc("simulateTransaction", [tx_serialized])
        return SimulationResult(**result)
    
    # Wallets
    def get_balance(self, address_hex: str) -> Union[str, int]:
        return self._rpc("getBalance", [address_hex])
    
    # Node info
    def get_version(self) -> str:
        return self._rpc("getVersion")
    
    def get_health(self) -> str:
        return self._rpc("getHealth")
    
    # Cross-chain bridge functionality
    def initiate_bridge_transfer(
        self, 
        source_chain: str,
        source_tx_id: str,
        target_chain: str,
        amount: int,
        source_address: str,
        target_address: str,
        asset_id: str
    ) -> str:
        """Initiate a cross-chain asset transfer."""
        payload = {
            "source_chain": source_chain,
            "source_tx_id": source_tx_id,
            "target_chain": target_chain,
            "amount": amount,
            "source_address": source_address,
            "target_address": target_address,
            "asset_id": asset_id
        }
        
        response = self.session.post(
            f"{self.base_url}/api/v1/bridge/transfer",
            json=payload,
            timeout=self.timeout
        )
        response.raise_for_status()
        
        result = response.json()
        if not result.get("success"):
            raise Exception(f"Bridge transfer failed: {result.get('error')}")
            
        return result["data"]["bridge_tx_id"]
    
    def get_bridge_transfer_status(self, bridge_tx_id: str) -> str:
        """Get the status of a bridge transfer."""
        response = self.session.get(
            f"{self.base_url}/api/v1/bridge/transfer/{bridge_tx_id}/status",
            timeout=self.timeout
        )
        response.raise_for_status()
        
        result = response.json()
        if not result.get("success"):
            raise Exception(f"Failed to get bridge transfer status: {result.get('error')}")
            
        return result["data"]
    
    def add_asset_mapping(
        self,
        source_asset_id: str,
        source_chain: str,
        target_asset_id: str,
        target_chain: str,
        conversion_rate: float
    ) -> bool:
        """Add an asset mapping between chains."""
        payload = {
            "source_asset_id": source_asset_id,
            "source_chain": source_chain,
            "target_asset_id": target_asset_id,
            "target_chain": target_chain,
            "conversion_rate": conversion_rate
        }
        
        response = self.session.post(
            f"{self.base_url}/api/v1/bridge/assets",
            json=payload,
            timeout=self.timeout
        )
        response.raise_for_status()
        
        result = response.json()
        return result.get("success", False)
    
    def get_asset_mapping(
        self,
        source_chain: str,
        source_asset_id: str,
        target_chain: str
    ) -> Optional[Dict[str, Any]]:
        """Get an asset mapping between chains."""
        response = self.session.get(
            f"{self.base_url}/api/v1/bridge/assets/{source_chain}/{source_asset_id}/{target_chain}",
            timeout=self.timeout
        )
        response.raise_for_status()
        
        result = response.json()
        if not result.get("success"):
            return None
            
        return result["data"]
    
    # Batch requests
    def batch(self, calls: list) -> list:
        """Execute multiple RPC calls in a batch."""
        payload = []
        for i, call in enumerate(calls):
            payload.append({
                "jsonrpc": "2.0",
                "id": i + 1,
                "method": call["method"],
                "params": call.get("params", [])
            })
        
        response = self.session.post(
            f"{self.base_url}/rpc",
            json=payload,
            timeout=self.timeout
        )
        response.raise_for_status()
        
        results = response.json()
        return [r.get("result") for r in results]