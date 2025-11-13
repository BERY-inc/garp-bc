package garp

import (
    "bytes"
    "encoding/json"
    "fmt"
    "net/http"
    "net/url"
)

// ChatClient provides REST access to chat endpoints via gateway '/api'.
type ChatClient struct {
    BaseURL string
    HTTP    *http.Client
}

func NewChatClient(baseURL string, httpClient *http.Client) *ChatClient {
    if httpClient == nil { httpClient = &http.Client{} }
    return &ChatClient{BaseURL: trimRight(baseURL, "/"), HTTP: httpClient}
}

type CreateMessageRequest struct {
    Sender    string `json:"sender"`
    Recipient string `json:"recipient"`
    Ciphertext string `json:"content_ciphertext"`
    Nonce      string `json:"content_nonce"`
}

type CreateMessageResponse struct {
    ID        int64  `json:"id"`
    Hash      string `json:"hash"`
    CreatedAt string `json:"created_at"`
}

type Message struct {
    ID        int64   `json:"id"`
    Sender    string  `json:"sender"`
    Recipient string  `json:"recipient"`
    Ciphertext string `json:"content_ciphertext"`
    Nonce      string `json:"content_nonce"`
    Hash      string  `json:"hash"`
    CreatedAt string  `json:"created_at"`
    Anchored  bool    `json:"anchored"`
    BlockHash *string `json:"block_hash"`
    BlockNumber *int64 `json:"block_number"`
}

type SignalRequest struct {
    From    string                 `json:"from"`
    To      string                 `json:"to"`
    Type    string                 `json:"type"`
    Payload map[string]interface{} `json:"payload"`
}

func (c *ChatClient) SendMessage(req CreateMessageRequest) (*CreateMessageResponse, error) {
    b, _ := json.Marshal(req)
    httpReq, err := http.NewRequest("POST", c.BaseURL+"/messages", bytesReader(b))
    if err != nil { return nil, err }
    httpReq.Header.Set("content-type", "application/json")
    resp, err := c.HTTP.Do(httpReq)
    if err != nil { return nil, err }
    defer resp.Body.Close()
    if resp.StatusCode < 200 || resp.StatusCode >= 300 {
        return nil, fmt.Errorf("HTTP %d", resp.StatusCode)
    }
    var out CreateMessageResponse
    if err := json.NewDecoder(resp.Body).Decode(&out); err != nil { return nil, err }
    return &out, nil
}

func (c *ChatClient) ListMessages(address, peer, since string, limit int) ([]Message, error) {
    q := url.Values{}
    q.Set("address", address)
    q.Set("peer", peer)
    if since != "" { q.Set("since", since) }
    if limit > 0 { q.Set("limit", fmt.Sprintf("%d", limit)) }
    httpReq, err := http.NewRequest("GET", c.BaseURL+"/messages?"+q.Encode(), nil)
    if err != nil { return nil, err }
    resp, err := c.HTTP.Do(httpReq)
    if err != nil { return nil, err }
    defer resp.Body.Close()
    if resp.StatusCode < 200 || resp.StatusCode >= 300 {
        return nil, fmt.Errorf("HTTP %d", resp.StatusCode)
    }
    var out []Message
    if err := json.NewDecoder(resp.Body).Decode(&out); err != nil { return nil, err }
    return out, nil
}

func (c *ChatClient) GetPublicKey(address string) (map[string]string, error) {
    httpReq, err := http.NewRequest("GET", c.BaseURL+"/keys/"+address, nil)
    if err != nil { return nil, err }
    resp, err := c.HTTP.Do(httpReq)
    if err != nil { return nil, err }
    defer resp.Body.Close()
    if resp.StatusCode < 200 || resp.StatusCode >= 300 {
        return nil, fmt.Errorf("HTTP %d", resp.StatusCode)
    }
    var out map[string]string
    if err := json.NewDecoder(resp.Body).Decode(&out); err != nil { return nil, err }
    return out, nil
}

// SendSignal publishes a signaling envelope to the recipient's stream.
func (c *ChatClient) SendSignal(req SignalRequest) error {
    b, _ := json.Marshal(req)
    httpReq, err := http.NewRequest("POST", c.BaseURL+"/signals", bytesReader(b))
    if err != nil { return err }
    httpReq.Header.Set("content-type", "application/json")
    resp, err := c.HTTP.Do(httpReq)
    if err != nil { return err }
    defer resp.Body.Close()
    if resp.StatusCode < 200 || resp.StatusCode >= 300 {
        return fmt.Errorf("HTTP %d", resp.StatusCode)
    }
    return nil
}

// bytesReader avoids importing bytes in multiple files
func bytesReader(b []byte) *bytes.Reader { return bytes.NewReader(b) }