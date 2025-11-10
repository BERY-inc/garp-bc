package main

import (
	"context"
	"encoding/json"
	"flag"
	"log"
	"net/http"
	"strconv"
	"time"

	"garp-backend/internal/client"
	"garp-backend/internal/config"
	bkmid "garp-backend/internal/middleware"
	botel "garp-backend/internal/otel"
	"garp-backend/internal/state"
	"garp-backend/internal/storage"

	"github.com/gin-gonic/gin"
)

func main() {
	cfgPath := flag.String("config", "", "path to TOML config")
	flag.Parse()
	cfg := config.Default()
	if *cfgPath != "" {
		_ = config.LoadFile(*cfgPath, &cfg)
	}
	config.ApplyEnv(&cfg)

	// OpenTelemetry init
	_ = botel.Init(cfg.OTEL.Endpoint, cfg.OTEL.ServiceName)

	r := gin.Default()
	r.Use(bkmid.MetricsMiddleware())
	r.Use(bkmid.MaxBodyBytes(1 << 20))
	r.Use(bkmid.RateLimit(240))
	store := state.NewStore()
	pc := client.New(cfg.Participant.BaseURL)
	_ = pc.WithTLS(cfg.TLS.ClientCert, cfg.TLS.ClientKey, cfg.TLS.CACert)
	sc := client.NewSynchronizer(cfg.Synchronizer.BaseURL)
	st, err := storage.Init(context.Background(), storage.Config{PostgresURL: cfg.Database.PostgresURL, RedisURL: cfg.Database.RedisURL})
	if err != nil {
		log.Printf("warn: storage init failed: %v", err)
	} else {
		defer st.Close()
	}
	if st != nil {
		_ = st.RunMigrations(context.Background())
	}

	// Health
	r.GET("/health", func(c *gin.Context) { c.String(http.StatusOK, "OK") })
	r.GET("/health/ready", func(c *gin.Context) {
		if st != nil && st.Ready(c.Request.Context()) {
			c.String(http.StatusOK, "READY")
		} else {
			c.String(http.StatusServiceUnavailable, "NOT_READY")
		}
	})
	r.GET("/metrics", bkmid.MetricsHandler())

	// Background consumer for tx_queue
	if st != nil {
		go func() {
			ctx := context.Background()
			for {
				// BRPOP blocks until message arrives or timeout
				res, err := st.Redis.BRPop(ctx, 0, "tx_queue").Result()
				if err != nil {
					log.Printf("redis consumer error: %v", err)
					time.Sleep(time.Second)
					continue
				}
				if len(res) == 2 {
					raw := res[1]
					var msg struct {
						Kind  string `json:"kind"`
						Hash  string `json:"hash"`
						Retry int    `json:"retry"`
					}
					if err := json.Unmarshal([]byte(raw), &msg); err != nil {
						log.Printf("invalid queue message: %v", err)
						continue
					}
					log.Printf("processing tx: %s (retry=%d)", msg.Hash, msg.Retry)
					// Simulate processing; if DB update fails, requeue with backoff
					if _, err := st.PG.Exec(ctx, `UPDATE transactions SET payload = $2 WHERE tx_hash = $1`, msg.Hash, []byte("processed")); err != nil {
						// exponential backoff up to 5 retries
						if msg.Retry < 5 {
							msg.Retry++
							backoff := time.Duration(1<<msg.Retry) * time.Second
							time.Sleep(backoff)
							b, _ := json.Marshal(msg)
							_ = st.Redis.LPush(ctx, "tx_queue", b).Err()
						} else {
							log.Printf("dropping tx %s after max retries", msg.Hash)
						}
					}
				}
			}
		}()
	}

	// Node status -> proxy to participant
	r.GET("/node/status", func(c *gin.Context) {
		var out map[string]any
		if err := pc.NodeStatus(&out); err != nil {
			c.JSON(http.StatusBadGateway, gin.H{"error": err.Error()})
			return
		}
		c.JSON(http.StatusOK, out)
	})

	// Transactions
	r.POST("/transactions", func(c *gin.Context) {
		var in map[string]any
		if err := c.ShouldBindJSON(&in); err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "invalid request"})
			return
		}
		var out map[string]any
		if err := sc.SubmitTransaction(in, &out); err != nil {
			c.JSON(http.StatusBadGateway, gin.H{"error": err.Error()})
			return
		}
		if st != nil {
			id, _ := out["transaction_id"].(string)
			payload := []byte("{}")
			_ = st.SaveTx(c.Request.Context(), id, payload)
		}
		c.JSON(http.StatusOK, out)
	})
	r.GET("/transactions/hash/:hash", func(c *gin.Context) {
		var out map[string]any
		if err := pc.GetTransaction(c.Param("hash"), &out); err != nil {
			c.JSON(http.StatusNotFound, gin.H{"error": err.Error()})
			return
		}
		c.JSON(http.StatusOK, out)
	})
	r.GET("/transactions/:id/status", func(c *gin.Context) {
		var out map[string]any
		if err := sc.TxStatus(c.Param("id"), &out); err != nil {
			c.JSON(http.StatusBadGateway, gin.H{"error": err.Error()})
			return
		}
		c.JSON(http.StatusOK, out)
	})

	// Blocks (proxy to participant)
	r.GET("/blocks/latest", func(c *gin.Context) {
		var out any
		if err := sc.LatestBlock(&out); err != nil {
			c.JSON(http.StatusBadGateway, gin.H{"error": err.Error()})
			return
		}
		c.JSON(http.StatusOK, out)
	})
	r.GET("/blocks/:num", func(c *gin.Context) {
		n, err := strconv.ParseUint(c.Param("num"), 10, 64)
		if err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "invalid block number"})
			return
		}
		var out any
		if err := sc.BlockByNumber(n, &out); err != nil {
			c.JSON(http.StatusBadGateway, gin.H{"error": err.Error()})
			return
		}
		c.JSON(http.StatusOK, out)
	})
	r.GET("/blocks/hash/:hash", func(c *gin.Context) {
		var out any
		if err := pc.BlockByHash(c.Param("hash"), &out); err != nil {
			c.JSON(http.StatusBadGateway, gin.H{"error": err.Error()})
			return
		}
		c.JSON(http.StatusOK, out)
	})

	// Accounts (still local until participant provides accounts API)
	r.POST("/accounts", func(c *gin.Context) {
		addr := "0xacc" + strconv.FormatInt(time.Now().UnixNano(), 36)
		acc := state.Account{Address: addr, Balance: 0, Nonce: 0}
		store.CreateAccount(acc)
		c.JSON(http.StatusOK, gin.H{"address": addr, "public_key": "pubkey"})
	})
	r.GET("/accounts/:addr", func(c *gin.Context) {
		if acc, ok := store.GetAccount(c.Param("addr")); ok {
			c.JSON(http.StatusOK, acc)
			return
		}
		c.JSON(http.StatusNotFound, gin.H{"error": "Account not found"})
	})

	// Contracts
	r.GET("/contracts", func(c *gin.Context) {
		var out any
		if err := pc.Contracts(&out); err != nil {
			c.JSON(http.StatusBadGateway, gin.H{"error": err.Error()})
			return
		}
		c.JSON(http.StatusOK, out)
	})
	// Compatibility endpoints for API gateway
	r.POST("/contracts/deploy", func(c *gin.Context) {
		var in map[string]any
		if err := c.ShouldBindJSON(&in); err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "invalid request"})
			return
		}
		var out map[string]any
		if err := pc.CreateContract(in, &out); err != nil {
			c.JSON(http.StatusBadGateway, gin.H{"error": err.Error()})
			return
		}
		c.JSON(http.StatusOK, out)
	})
	r.POST("/contracts/call", func(c *gin.Context) {
		var in map[string]any
		if err := c.ShouldBindJSON(&in); err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "invalid request"})
			return
		}
		// Expect contract_address in payload
		addr, _ := in["contract_address"].(string)
		var out map[string]any
		if err := pc.ExerciseContract(addr, &in, &out); err != nil {
			c.JSON(http.StatusBadGateway, gin.H{"error": err.Error()})
			return
		}
		c.JSON(http.StatusOK, out)
	})
	r.POST("/contracts", func(c *gin.Context) {
		var in map[string]any
		if err := c.ShouldBindJSON(&in); err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "invalid request"})
			return
		}
		var out map[string]any
		if err := pc.CreateContract(in, &out); err != nil {
			c.JSON(http.StatusBadGateway, gin.H{"error": err.Error()})
			return
		}
		c.JSON(http.StatusOK, out)
	})
	r.POST("/contracts/:id/exercise", func(c *gin.Context) {
		var in map[string]any
		if err := c.ShouldBindJSON(&in); err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "invalid request"})
			return
		}
		var out map[string]any
		if err := pc.ExerciseContract(c.Param("id"), &in, &out); err != nil {
			c.JSON(http.StatusBadGateway, gin.H{"error": err.Error()})
			return
		}
		c.JSON(http.StatusOK, out)
	})
	r.DELETE("/contracts/:id/archive", func(c *gin.Context) {
		if err := pc.ArchiveContract(c.Param("id")); err != nil {
			c.JSON(http.StatusBadGateway, gin.H{"error": err.Error()})
			return
		}
		c.JSON(http.StatusOK, gin.H{"success": true})
	})

	// Privacy
	r.POST("/privacy", func(c *gin.Context) {
		var body struct {
			Data []byte `json:"data"`
		}
		if err := c.ShouldBindJSON(&body); err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "invalid request"})
			return
		}
		c.JSON(http.StatusOK, gin.H{"result": body.Data})
	})

	// Wallet proxies
	r.GET("/wallet/balances", func(c *gin.Context) {
		var out any
		if err := pc.WalletBalances(&out); err != nil {
			c.JSON(http.StatusBadGateway, gin.H{"error": err.Error()})
			return
		}
		c.JSON(http.StatusOK, out)
	})
	r.GET("/wallet/history", func(c *gin.Context) {
		var out any
		if err := pc.WalletHistory(&out); err != nil {
			c.JSON(http.StatusBadGateway, gin.H{"error": err.Error()})
			return
		}
		c.JSON(http.StatusOK, out)
	})

	// Ledger checkpoint proxy
	r.GET("/ledger/checkpoint", func(c *gin.Context) {
		var out any
		if err := pc.LedgerCheckpoint(&out); err != nil {
			c.JSON(http.StatusBadGateway, gin.H{"error": err.Error()})
			return
		}
		c.JSON(http.StatusOK, out)
	})

	addr := ":" + strconv.Itoa(cfg.Server.Port)
	log.Println("Starting GARP Go Participant Backend on ", addr)
	if err := r.Run(addr); err != nil {
		log.Fatal(err)
	}
}
