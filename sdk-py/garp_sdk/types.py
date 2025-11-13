from typing import List, Optional
from dataclasses import dataclass


@dataclass
class BlockTx:
    id: str
    submitter: Optional[str] = None
    command_type: Optional[str] = None


@dataclass
class BlockInfo:
    slot: int
    hash: str
    parent_hash: Optional[str] = None
    timestamp_ms: Optional[int] = None
    leader: Optional[str] = None
    transactions: Optional[List[BlockTx]] = None


@dataclass
class TransactionInfo:
    id: str
    submitter: Optional[str] = None
    status: Optional[str] = None
    created_at: Optional[int] = None
    error: Optional[str] = None