package client

import (
	"bytes"
	"crypto/tls"
	"crypto/x509"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"time"
)

type ParticipantClient struct {
	base string
	http *http.Client
}

func New(base string) *ParticipantClient {
	return &ParticipantClient{base: base, http: &http.Client{Timeout: 15 * time.Second}}
}

// WithTLS configures mTLS for the client if certs are provided
func (c *ParticipantClient) WithTLS(clientCertFile, clientKeyFile, caCertFile string) error {
	if clientCertFile == "" || clientKeyFile == "" || caCertFile == "" {
		return nil
	}
	cert, err := tls.LoadX509KeyPair(clientCertFile, clientKeyFile)
	if err != nil {
		return err
	}
	caCert, err := os.ReadFile(caCertFile)
	if err != nil {
		return err
	}
	caPool := x509.NewCertPool()
	if !caPool.AppendCertsFromPEM(caCert) {
		return fmt.Errorf("failed to append CA cert")
	}
	tr := &http.Transport{TLSClientConfig: &tls.Config{Certificates: []tls.Certificate{cert}, RootCAs: caPool}}
	c.http = &http.Client{Transport: tr, Timeout: 15 * time.Second}
	return nil
}

func (c *ParticipantClient) get(path string, out any) error {
	resp, err := c.http.Get(c.base + path)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode >= 300 {
		return fmt.Errorf("participant %s returned %d", path, resp.StatusCode)
	}
	return json.NewDecoder(resp.Body).Decode(out)
}

func (c *ParticipantClient) post(path string, in any, out any) error {
	buf := &bytes.Buffer{}
	if err := json.NewEncoder(buf).Encode(in); err != nil {
		return err
	}
	resp, err := c.http.Post(c.base+path, "application/json", buf)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode >= 300 {
		return fmt.Errorf("participant %s returned %d", path, resp.StatusCode)
	}
	return json.NewDecoder(resp.Body).Decode(out)
}

func (c *ParticipantClient) delete(path string, out any) error {
	req, _ := http.NewRequest(http.MethodDelete, c.base+path, nil)
	resp, err := c.http.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode >= 300 {
		return fmt.Errorf("participant %s returned %d", path, resp.StatusCode)
	}
	if out != nil {
		return json.NewDecoder(resp.Body).Decode(out)
	}
	return nil
}

// Proxies for known participant endpoints
func (c *ParticipantClient) NodeStatus(out any) error { return c.get("/api/v1/node/status", out) }
func (c *ParticipantClient) SubmitTransaction(in any, out any) error {
	return c.post("/api/v1/transactions", in, out)
}
func (c *ParticipantClient) GetTransaction(id string, out any) error {
	return c.get("/api/v1/transactions/"+id, out)
}
func (c *ParticipantClient) CreateContract(in any, out any) error {
	return c.post("/api/v1/contracts", in, out)
}
func (c *ParticipantClient) ExerciseContract(id string, in any, out any) error {
	return c.post("/api/v1/contracts/"+id+"/exercise", in, out)
}
func (c *ParticipantClient) ArchiveContract(id string) error {
	return c.delete("/api/v1/contracts/"+id+"/archive", nil)
}
func (c *ParticipantClient) Contracts(out any) error { return c.get("/api/v1/contracts", out) }
func (c *ParticipantClient) WalletBalances(out any) error {
	return c.get("/api/v1/wallet/balances", out)
}
func (c *ParticipantClient) WalletHistory(out any) error { return c.get("/api/v1/wallet/history", out) }
func (c *ParticipantClient) LatestBlock(out any) error   { return c.get("/api/v1/blocks/latest", out) }
func (c *ParticipantClient) BlockByNumber(n uint64, out any) error {
	return c.get(fmt.Sprintf("/api/v1/blocks/%d", n), out)
}
func (c *ParticipantClient) BlockByHash(h string, out any) error {
	return c.get("/api/v1/blocks/hash/"+h, out)
}
func (c *ParticipantClient) LedgerCheckpoint(out any) error {
	return c.get("/api/v1/ledger/checkpoint", out)
}
