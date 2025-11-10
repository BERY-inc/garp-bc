package state

import (
    "sync"
    "time"
)

type Transaction struct {
    TxHash      string     `json:"tx_hash"`
    Status      string     `json:"status"`
    BlockNumber *uint64    `json:"block_number,omitempty"`
    GasUsed     *uint64    `json:"gas_used,omitempty"`
    CreatedAt   time.Time  `json:"created_at"`
}

type Account struct {
    Address  string  `json:"address"`
    Balance  uint64  `json:"balance"`
    Nonce    uint64  `json:"nonce"`
    CodeHash *string `json:"code_hash,omitempty"`
}

type BlockInfo struct {
    Number           uint64    `json:"number"`
    Hash             string    `json:"hash"`
    ParentHash       string    `json:"parent_hash"`
    Timestamp        time.Time `json:"timestamp"`
    TransactionCount uint32    `json:"transaction_count"`
    Size             uint64    `json:"size"`
    GasUsed          uint64    `json:"gas_used"`
    GasLimit         uint64    `json:"gas_limit"`
}

type Store struct {
    mu sync.RWMutex
    txs map[string]Transaction
    accounts map[string]Account
    latest BlockInfo
}

func NewStore() *Store {
    return &Store{
        txs: make(map[string]Transaction),
        accounts: make(map[string]Account),
        latest: BlockInfo{Number: 1, Hash: "0xgenesis", ParentHash: "0x0", Timestamp: time.Now().UTC(), TransactionCount: 0, Size: 1024, GasUsed: 0, GasLimit: 1000000},
    }
}

func (s *Store) SubmitTx(tx Transaction) { s.mu.Lock(); defer s.mu.Unlock(); s.txs[tx.TxHash] = tx }
func (s *Store) GetTx(hash string) (Transaction, bool) { s.mu.RLock(); defer s.mu.RUnlock(); v, ok := s.txs[hash]; return v, ok }

func (s *Store) CreateAccount(a Account) { s.mu.Lock(); defer s.mu.Unlock(); s.accounts[a.Address] = a }
func (s *Store) GetAccount(addr string) (Account, bool) { s.mu.RLock(); defer s.mu.RUnlock(); v, ok := s.accounts[addr]; return v, ok }

func (s *Store) LatestBlock() BlockInfo { s.mu.RLock(); defer s.mu.RUnlock(); return s.latest }
func (s *Store) SetLatestBlock(b BlockInfo) { s.mu.Lock(); defer s.mu.Unlock(); s.latest = b }