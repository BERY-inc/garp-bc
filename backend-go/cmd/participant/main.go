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
    redis "github.com/redis/go-redis/v9"
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
    // Initialize Redis client for distributed rate limiting
    var rdb *redis.Client
    if opt, err := redis.ParseURL(cfg.Database.RedisURL); err == nil {
        rdb = redis.NewClient(opt)
    } else {
        log.Printf("rate limit: invalid Redis URL, falling back to local limiter: %v", err)
    }
    r.Use(bkmid.RateLimitRedis(240, rdb))
	// Add baseline security headers
	r.Use(bkmid.SecurityHeaders())
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

    // Chat: create message
    r.POST("/messages", func(c *gin.Context) {
        if st == nil {
            c.JSON(http.StatusServiceUnavailable, gin.H{"error": "storage unavailable"})
            return
        }
        var in struct {
            Sender    string `json:"sender"`
            Recipient string `json:"recipient"`
            Ciphertext string `json:"content_ciphertext"`
            Nonce      string `json:"content_nonce"`
        }
        if err := c.ShouldBindJSON(&in); err != nil {
            c.JSON(http.StatusBadRequest, gin.H{"error": "invalid request"})
            return
        }
        ct := []byte(in.Ciphertext)
        nn := []byte(in.Nonce)
        m, err := st.CreateMessage(c.Request.Context(), in.Sender, in.Recipient, ct, nn)
        if err != nil {
            c.JSON(http.StatusInternalServerError, gin.H{"error": err.Error()})
            return
        }
        c.JSON(http.StatusOK, gin.H{"id": m.ID, "hash": m.Hash, "created_at": m.CreatedAt})
    })

    // Chat: list messages between two peers
    r.GET("/messages", func(c *gin.Context) {
        if st == nil {
            c.JSON(http.StatusServiceUnavailable, gin.H{"error": "storage unavailable"})
            return
        }
        a := c.Query("address")
        b := c.Query("peer")
        if a == "" || b == "" {
            c.JSON(http.StatusBadRequest, gin.H{"error": "address and peer are required"})
            return
        }
        var sincePtr *time.Time
        if s := c.Query("since"); s != "" {
            if t, err := time.Parse(time.RFC3339, s); err == nil { sincePtr = &t }
        }
        limit := 100
        if l := c.Query("limit"); l != "" {
            if v, err := strconv.Atoi(l); err == nil && v > 0 { limit = v }
        }
        msgs, err := st.ListMessages(c.Request.Context(), a, b, sincePtr, limit)
        if err != nil {
            c.JSON(http.StatusInternalServerError, gin.H{"error": err.Error()})
            return
        }
        c.JSON(http.StatusOK, msgs)
    })

    // Chat: anchor a message hash to a block (placeholder)
    r.POST("/messages/:id/anchor", func(c *gin.Context) {
        if st == nil {
            c.JSON(http.StatusServiceUnavailable, gin.H{"error": "storage unavailable"})
            return
        }
        id64, err := strconv.ParseInt(c.Param("id"), 10, 64)
        if err != nil { c.JSON(http.StatusBadRequest, gin.H{"error": "invalid id"}); return }
        var in struct{ Block int64 `json:"block"` }
        if err := c.ShouldBindJSON(&in); err != nil { c.JSON(http.StatusBadRequest, gin.H{"error": "invalid request"}); return }
        if err := st.AnchorMessage(c.Request.Context(), id64, in.Block); err != nil {
            c.JSON(http.StatusInternalServerError, gin.H{"error": err.Error()})
            return
        }
        c.JSON(http.StatusOK, gin.H{"id": id64, "anchored_at_block": in.Block})
    })

    // Chat: SSE stream of new message events via Redis Pub/Sub
    r.GET("/stream/messages", func(c *gin.Context) {
        if st == nil || st.Redis == nil {
            c.JSON(http.StatusServiceUnavailable, gin.H{"error": "stream unavailable"})
            return
        }
        c.Header("Content-Type", "text/event-stream")
        c.Header("Cache-Control", "no-cache")
        c.Header("Connection", "keep-alive")
        ctx := c.Request.Context()
        sub := st.Redis.Subscribe(ctx, "messages")
        defer sub.Close()
        ch := sub.Channel()
        // initial heartbeat
        c.Writer.Write([]byte("event: heartbeat\n"))
        c.Writer.Write([]byte("data: ok\n\n"))
        c.Writer.Flush()
        for {
            select {
            case <-ctx.Done():
                return
            case msg, ok := <-ch:
                if !ok { return }
                c.Writer.Write([]byte("event: message\n"))
                c.Writer.Write([]byte("data: "))
                c.Writer.Write([]byte(msg.Payload))
                c.Writer.Write([]byte("\n\n"))
                c.Writer.Flush()
            }
        }
    })

    // --- P2P signaling: publish/subscribe ephemeral signals for WebRTC/D2D setup ---
    // Send signal: { from, to, type, payload }
    r.POST("/signals", func(c *gin.Context) {
        if st == nil || st.Redis == nil {
            c.JSON(http.StatusServiceUnavailable, gin.H{"error": "signals unavailable"})
            return
        }
        var in struct {
            From   string         `json:"from"`
            To     string         `json:"to"`
            Type   string         `json:"type"`
            Payload map[string]any `json:"payload"`
        }
        if err := c.ShouldBindJSON(&in); err != nil || in.From == "" || in.To == "" || in.Type == "" {
            c.JSON(http.StatusBadRequest, gin.H{"error": "invalid request"})
            return
        }
        env := map[string]any{
            "from": in.From,
            "to": in.To,
            "type": in.Type,
            "payload": in.Payload,
            "timestamp": time.Now().UTC().Format(time.RFC3339),
        }
        b, _ := json.Marshal(env)
        ch := "signals:" + in.To
        if err := st.Redis.Publish(c.Request.Context(), ch, b).Err(); err != nil {
            c.JSON(http.StatusInternalServerError, gin.H{"error": err.Error()})
            return
        }
        c.JSON(http.StatusOK, gin.H{"success": true})
    })

    // Stream signals for a given address via SSE
    r.GET("/stream/signals", func(c *gin.Context) {
        if st == nil || st.Redis == nil {
            c.JSON(http.StatusServiceUnavailable, gin.H{"error": "signals unavailable"})
            return
        }
        addr := c.Query("address")
        if addr == "" {
            c.JSON(http.StatusBadRequest, gin.H{"error": "address required"})
            return
        }
        c.Header("Content-Type", "text/event-stream")
        c.Header("Cache-Control", "no-cache")
        c.Header("Connection", "keep-alive")
        ctx := c.Request.Context()
        sub := st.Redis.Subscribe(ctx, "signals:"+addr)
        defer sub.Close()
        ch := sub.Channel()
        c.Writer.Write([]byte("event: heartbeat\n"))
        c.Writer.Write([]byte("data: ok\n\n"))
        c.Writer.Flush()
        for {
            select {
            case <-ctx.Done():
                return
            case msg, ok := <-ch:
                if !ok { return }
                c.Writer.Write([]byte("event: signal\n"))
                c.Writer.Write([]byte("data: "))
                c.Writer.Write([]byte(msg.Payload))
                c.Writer.Write([]byte("\n\n"))
                c.Writer.Flush()
            }
        }
    })
    // Keys (placeholder public key retrieval)
    r.GET("/keys/:addr", func(c *gin.Context) {
        addr := c.Param("addr")
        // Placeholder: return a static public key until integrated with wallet
        c.JSON(http.StatusOK, gin.H{"address": addr, "public_key": "pubkey"})
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
