package client

import (
    "encoding/json"
    "fmt"
    "net/http"
    "time"
    "bytes"
)

type SynchronizerClient struct { base string; http *http.Client }

func NewSynchronizer(base string) *SynchronizerClient { return &SynchronizerClient{ base: base, http: &http.Client{ Timeout: 10 * time.Second } } }

func (c *SynchronizerClient) get(path string, out any) error {
    resp, err := c.http.Get(c.base + path)
    if err != nil { return err }
    defer resp.Body.Close()
    if resp.StatusCode >= 300 { return fmt.Errorf("synchronizer %s returned %d", path, resp.StatusCode) }
    return json.NewDecoder(resp.Body).Decode(out)
}

func (c *SynchronizerClient) LatestBlock(out any) error { return c.get("/api/v1/blocks/latest", out) }
func (c *SynchronizerClient) BlockByNumber(n uint64, out any) error { return c.get(fmt.Sprintf("/api/v1/blocks/%d", n), out) }
func (c *SynchronizerClient) Status(out any) error { return c.get("/api/v1/status", out) }
func (c *SynchronizerClient) TxStatus(id string, out any) error { return c.get("/api/v1/transactions/"+id+"/status", out) }
func (c *SynchronizerClient) SubmitTransaction(in any, out any) error {
    b, _ := json.Marshal(in)
    resp, err := c.http.Post(c.base+"/api/v1/transactions", "application/json", bytes.NewReader(b))
    if err != nil { return err }
    defer resp.Body.Close()
    if resp.StatusCode >= 300 { return fmt.Errorf("synchronizer /transactions returned %d", resp.StatusCode) }
    return json.NewDecoder(resp.Body).Decode(out)
}