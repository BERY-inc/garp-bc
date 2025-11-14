package garp

import (
    "bytes"
    "context"
    "encoding/json"
    "errors"
    "fmt"
    "net/http"
    "time"
)

type Client struct {
    BaseURL string
    HTTP    *http.Client
}

func NewClient(baseURL string) *Client {
    return &Client{
        BaseURL: trimRight(baseURL, "/"),
        HTTP:    &http.Client{Timeout: 10 * time.Second},
    }
}

func NewClientWithHTTP(baseURL string, httpClient *http.Client) *Client {
    return &Client{BaseURL: trimRight(baseURL, "/"), HTTP: httpClient}
}

func trimRight(s, suffix string) string {
    for len(s) > 0 && s[len(s)-1:] == suffix {
        s = s[:len(s)-1]
    }
    return s
}

type jsonRpcRequest struct {
    Jsonrpc string      `json:"jsonrpc"`
    ID      int         `json:"id"`
    Method  string      `json:"method"`
    Params  interface{} `json:"params,omitempty"`
}

type jsonRpcError struct {
    Code    int         `json:"code"`
    Message string      `json:"message"`
    Data    interface{} `json:"data,omitempty"`
}

type jsonRpcResponse struct {
    Jsonrpc string          `json:"jsonrpc"`
    ID      int             `json:"id"`
    Result  json.RawMessage `json:"result"`
    Error   *jsonRpcError   `json:"error"`
}

type BlockTx struct {
    ID          string  `json:"id"`
    Submitter   *string `json:"submitter,omitempty"`
    CommandType *string `json:"command_type,omitempty"`
}

type BlockInfo struct {
    Slot        int64      `json:"slot"`
    Hash        string     `json:"hash"`
    ParentHash  *string    `json:"parent_hash,omitempty"`
    TimestampMs *int64     `json:"timestamp_ms,omitempty"`
    Leader      *string    `json:"leader,omitempty"`
    Transactions *[]BlockTx `json:"transactions,omitempty"`
}

type TransactionInfo struct {
    ID        string  `json:"id"`
    Submitter *string `json:"submitter,omitempty"`
    Status    *string `json:"status,omitempty"`
    CreatedAt *int64  `json:"created_at,omitempty"`
    Error     *string `json:"error,omitempty"`
}

type SimulationResult struct {
    Ok    bool     `json:"ok"`
    Logs  []string `json:"logs,omitempty"`
    Error *string  `json:"error,omitempty"`
}

func (c *Client) rpc(method string, params interface{}, out interface{}) error {
    body := jsonRpcRequest{Jsonrpc: "2.0", ID: 1, Method: method, Params: params}
    b, _ := json.Marshal(body)
    req, err := http.NewRequest("POST", c.BaseURL+"/rpc", bytes.NewReader(b))
    if err != nil {
        return err
    }
    req.Header.Set("content-type", "application/json")
    resp, err := c.HTTP.Do(req)
    if err != nil {
        return err
    }
    defer resp.Body.Close()
    var jr jsonRpcResponse
    if err := json.NewDecoder(resp.Body).Decode(&jr); err != nil {
        return err
    }
    if jr.Error != nil {
        return errors.New(fmt.Sprintf("RPC %s failed (%d): %s", method, jr.Error.Code, jr.Error.Message))
    }
    if out == nil {
        return nil
    }
    return json.Unmarshal(jr.Result, out)
}

func (c *Client) rpcCtx(ctx context.Context, method string, params interface{}, out interface{}) error {
    body := jsonRpcRequest{Jsonrpc: "2.0", ID: 1, Method: method, Params: params}
    b, _ := json.Marshal(body)
    req, err := http.NewRequestWithContext(ctx, "POST", c.BaseURL+"/rpc", bytes.NewReader(b))
    if err != nil {
        return err
    }
    req.Header.Set("content-type", "application/json")
    resp, err := c.HTTP.Do(req)
    if err != nil {
        return err
    }
    defer resp.Body.Close()
    var jr jsonRpcResponse
    if err := json.NewDecoder(resp.Body).Decode(&jr); err != nil {
        return err
    }
    if jr.Error != nil {
        return errors.New(fmt.Sprintf("RPC %s failed (%d): %s", method, jr.Error.Code, jr.Error.Message))
    }
    if out == nil {
        return nil
    }
    return json.Unmarshal(jr.Result, out)
}

// Timing & consensus
func (c *Client) GetSlot() (int64, error) {
    var n int64
    err := c.rpc("getSlot", nil, &n)
    return n, err
}

func (c *Client) GetSlotCtx(ctx context.Context) (int64, error) {
    var n int64
    err := c.rpcCtx(ctx, "getSlot", nil, &n)
    return n, err
}

func (c *Client) GetSlotLeader() (string, error) {
    var s string
    err := c.rpc("getSlotLeader", nil, &s)
    return s, err
}

func (c *Client) GetSlotLeaderCtx(ctx context.Context) (string, error) {
    var s string
    err := c.rpcCtx(ctx, "getSlotLeader", nil, &s)
    return s, err
}

// Blocks
func (c *Client) GetBlockBySlot(slot int64) (*BlockInfo, error) {
    var b *BlockInfo
    err := c.rpc("getBlock", []interface{}{slot}, &b)
    return b, err
}

func (c *Client) GetBlockBySlotCtx(ctx context.Context, slot int64) (*BlockInfo, error) {
    var b *BlockInfo
    err := c.rpcCtx(ctx, "getBlock", []interface{}{slot}, &b)
    return b, err
}

func (c *Client) GetBlockByHash(hashHex string) (*BlockInfo, error) {
    var b *BlockInfo
    err := c.rpc("getBlock", []interface{}{hashHex}, &b)
    return b, err
}

func (c *Client) GetBlockByHashCtx(ctx context.Context, hashHex string) (*BlockInfo, error) {
    var b *BlockInfo
    err := c.rpcCtx(ctx, "getBlock", []interface{}{hashHex}, &b)
    return b, err
}

// Transactions
func (c *Client) GetTransaction(txIdHex string) (*TransactionInfo, error) {
    var t *TransactionInfo
    err := c.rpc("getTransaction", []interface{}{txIdHex}, &t)
    return t, err
}

func (c *Client) GetTransactionCtx(ctx context.Context, txIdHex string) (*TransactionInfo, error) {
    var t *TransactionInfo
    err := c.rpcCtx(ctx, "getTransaction", []interface{}{txIdHex}, &t)
    return t, err
}

func (c *Client) SendTransactionRaw(serialized string) (string, error) {
    var id string
    err := c.rpc("sendTransaction", []interface{}{serialized}, &id)
    return id, err
}

func (c *Client) SendTransactionRawCtx(ctx context.Context, serialized string) (string, error) {
    var id string
    err := c.rpcCtx(ctx, "sendTransaction", []interface{}{serialized}, &id)
    return id, err
}

func (c *Client) SimulateTransactionRaw(serialized string) (*SimulationResult, error) {
    var sr SimulationResult
    err := c.rpc("simulateTransaction", []interface{}{serialized}, &sr)
    return &sr, err
}

func (c *Client) SimulateTransactionRawCtx(ctx context.Context, serialized string) (*SimulationResult, error) {
    var sr SimulationResult
    err := c.rpcCtx(ctx, "simulateTransaction", []interface{}{serialized}, &sr)
    return &sr, err
}

// Wallets
func (c *Client) GetBalance(addressHex string) (json.RawMessage, error) {
    var v json.RawMessage
    err := c.rpc("getBalance", []interface{}{addressHex}, &v)
    return v, err
}

func (c *Client) GetBalanceCtx(ctx context.Context, addressHex string) (json.RawMessage, error) {
    var v json.RawMessage
    err := c.rpcCtx(ctx, "getBalance", []interface{}{addressHex}, &v)
    return v, err
}

// Node info
func (c *Client) GetVersion() (string, error) {
    var s string
    err := c.rpc("getVersion", nil, &s)
    return s, err
}

func (c *Client) GetVersionCtx(ctx context.Context) (string, error) {
    var s string
    err := c.rpcCtx(ctx, "getVersion", nil, &s)
    return s, err
}

func (c *Client) GetHealth() (string, error) {
    var s string
    err := c.rpc("getHealth", nil, &s)
    return s, err
}

func (c *Client) GetHealthCtx(ctx context.Context) (string, error) {
    var s string
    err := c.rpcCtx(ctx, "getHealth", nil, &s)
    return s, err
}

// Cross-chain bridge functionality
type BridgeTransferRequest struct {
    SourceChain   string `json:"source_chain"`
    SourceTxID    string `json:"source_tx_id"`
    TargetChain   string `json:"target_chain"`
    Amount        int64  `json:"amount"`
    SourceAddress string `json:"source_address"`
    TargetAddress string `json:"target_address"`
    AssetID       string `json:"asset_id"`
}

type BridgeTransferResponse struct {
    Success bool   `json:"success"`
    Data    struct {
        BridgeTxID string `json:"bridge_tx_id"`
    } `json:"data"`
    Error *string `json:"error,omitempty"`
}

type BridgeTransferStatusResponse struct {
    Success bool   `json:"success"`
    Data    string `json:"data"`
    Error   *string `json:"error,omitempty"`
}

type AssetMappingRequest struct {
    SourceAssetID  string  `json:"source_asset_id"`
    SourceChain    string  `json:"source_chain"`
    TargetAssetID  string  `json:"target_asset_id"`
    TargetChain    string  `json:"target_chain"`
    ConversionRate float64 `json:"conversion_rate"`
}

type AssetMappingResponse struct {
    Success bool        `json:"success"`
    Data    interface{} `json:"data"`
    Error   *string     `json:"error,omitempty"`
}

// InitiateBridgeTransfer initiates a cross-chain asset transfer
func (c *Client) InitiateBridgeTransfer(req BridgeTransferRequest) (string, error) {
    body, err := json.Marshal(req)
    if err != nil {
        return "", err
    }

    httpReq, err := http.NewRequest("POST", c.BaseURL+"/api/v1/bridge/transfer", bytes.NewReader(body))
    if err != nil {
        return "", err
    }
    httpReq.Header.Set("content-type", "application/json")

    resp, err := c.HTTP.Do(httpReq)
    if err != nil {
        return "", err
    }
    defer resp.Body.Close()

    var result BridgeTransferResponse
    if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
        return "", err
    }

    if !result.Success {
        if result.Error != nil {
            return "", errors.New(*result.Error)
        }
        return "", errors.New("bridge transfer failed")
    }

    return result.Data.BridgeTxID, nil
}

// GetBridgeTransferStatus gets the status of a bridge transfer
func (c *Client) GetBridgeTransferStatus(bridgeTxID string) (string, error) {
    url := fmt.Sprintf("%s/api/v1/bridge/transfer/%s/status", c.BaseURL, bridgeTxID)
    resp, err := c.HTTP.Get(url)
    if err != nil {
        return "", err
    }
    defer resp.Body.Close()

    var result BridgeTransferStatusResponse
    if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
        return "", err
    }

    if !result.Success {
        if result.Error != nil {
            return "", errors.New(*result.Error)
        }
        return "", errors.New("failed to get bridge transfer status")
    }

    return result.Data, nil
}

// AddAssetMapping adds an asset mapping between chains
func (c *Client) AddAssetMapping(req AssetMappingRequest) (bool, error) {
    body, err := json.Marshal(req)
    if err != nil {
        return false, err
    }

    httpReq, err := http.NewRequest("POST", c.BaseURL+"/api/v1/bridge/assets", bytes.NewReader(body))
    if err != nil {
        return false, err
    }
    httpReq.Header.Set("content-type", "application/json")

    resp, err := c.HTTP.Do(httpReq)
    if err != nil {
        return false, err
    }
    defer resp.Body.Close()

    var result struct {
        Success bool   `json:"success"`
        Message string `json:"message"`
        Error   *string `json:"error,omitempty"`
    }
    if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
        return false, err
    }

    if !result.Success {
        if result.Error != nil {
            return false, errors.New(*result.Error)
        }
        return false, errors.New(result.Message)
    }

    return true, nil
}

// GetAssetMapping gets an asset mapping between chains
func (c *Client) GetAssetMapping(sourceChain, sourceAssetID, targetChain string) (interface{}, error) {
    url := fmt.Sprintf("%s/api/v1/bridge/assets/%s/%s/%s", c.BaseURL, sourceChain, sourceAssetID, targetChain)
    resp, err := c.HTTP.Get(url)
    if err != nil {
        return nil, err
    }
    defer resp.Body.Close()

    var result AssetMappingResponse
    if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
        return nil, err
    }

    if !result.Success {
        if result.Error != nil {
            return nil, errors.New(*result.Error)
        }
        return nil, errors.New("failed to get asset mapping")
    }

    return result.Data, nil
}