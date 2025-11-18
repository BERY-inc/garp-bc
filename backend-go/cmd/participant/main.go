package main

import (
    "context"
    "fmt"
    "log"
    "net/http"
    "os"
    "os/signal"
    "syscall"
    "time"

    "github.com/gin-gonic/gin"

    "garp-backend/internal/client"
    "garp-backend/internal/config"
    "garp-backend/internal/middleware"
    "garp-backend/internal/otel"
    "garp-backend/internal/state"
    "garp-backend/internal/storage"
)

func main() {
    // Load configuration (defaults + env overrides)
    cfg := config.Default()
    config.ApplyEnv(&cfg)

    // Initialize tracing
    shutdown := otel.Init(cfg.OTEL.Endpoint, cfg.OTEL.ServiceName)
    defer shutdown(context.Background())

    // Connect to storage services (Postgres + Redis)
    store, err := storage.Init(context.Background(), storage.Config{PostgresURL: cfg.Database.PostgresURL, RedisURL: cfg.Database.RedisURL})
    if err != nil {
        log.Fatalf("Failed to initialize storage: %v", err)
    }
    defer store.Close()

    // Run migrations
    if err := store.RunMigrations(context.Background()); err != nil {
        log.Fatalf("Failed to run migrations: %v", err)
    }

	// Initialize clients
    participantClient := client.New(cfg.Participant.BaseURL)
    if err := participantClient.WithTLS(cfg.TLS.ClientCert, cfg.TLS.ClientKey, cfg.TLS.CACert); err != nil {
        log.Fatalf("Failed to configure mTLS: %v", err)
    }

	// Initialize state manager
    // Initialize in-memory state store
    stateManager := state.NewStore()
    _ = stateManager

	// Create Gin engine
    gin.SetMode(gin.ReleaseMode)
    r := gin.New()
    r.Use(gin.Logger())
    r.Use(gin.Recovery())
    r.Use(middleware.SecurityHeaders())
    r.Use(otel.Middleware(cfg.OTEL.ServiceName))
    r.Use(middleware.RateLimitRedis(120, store.Redis))

	// Health check endpoints
	r.GET("/health", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"status": "ok"})
	})
	r.GET("/ready", func(c *gin.Context) {
        // Check storage connectivity
        if !store.Ready(c.Request.Context()) {
            c.JSON(http.StatusServiceUnavailable, gin.H{"status": "database unavailable"})
            return
        }
        c.JSON(http.StatusOK, gin.H{"status": "ready"})
    })

	// API routes
	api := r.Group("/api/v1")
	{
		// Transaction endpoints
		api.POST("/transactions", func(c *gin.Context) {
			// Implementation for submitting transactions
		})
		
		api.GET("/transactions/:id", func(c *gin.Context) {
			// Implementation for getting transaction details
		})
		
		api.GET("/transactions/:id/status", func(c *gin.Context) {
			// Implementation for getting transaction status
		})

		// Account endpoints
		api.GET("/accounts/:address", func(c *gin.Context) {
			// Implementation for getting account details
		})
		
		api.GET("/accounts/:address/balance", func(c *gin.Context) {
			// Implementation for getting account balance
		})

		// Contract endpoints
		api.POST("/contracts", func(c *gin.Context) {
			// Implementation for deploying contracts
		})
		
		api.POST("/contracts/:id/exercise", func(c *gin.Context) {
			// Implementation for exercising contracts
		})

		// Wallet endpoints
		api.GET("/wallet/balance", func(c *gin.Context) {
			// Implementation for getting wallet balance
		})
		
		api.POST("/wallet/transfer", func(c *gin.Context) {
			// Implementation for transferring funds
		})
	}

	// Enterprise integration endpoints
	enterprise := r.Group("/enterprise")
	{
		// ERP integration
		enterprise.POST("/erp/transaction", func(c *gin.Context) {
			// Implementation for creating ERP transactions
		})
		
		enterprise.GET("/erp/transaction/:id", func(c *gin.Context) {
			// Implementation for getting ERP transaction details
		})

		// CRM integration
		enterprise.POST("/crm/contact", func(c *gin.Context) {
			// Implementation for creating CRM contacts
		})
		
		enterprise.GET("/crm/contact/:id", func(c *gin.Context) {
			// Implementation for getting CRM contact details
		})

		// Database integration
		enterprise.POST("/db/transaction", func(c *gin.Context) {
			// Implementation for storing blockchain transactions in external databases
		})
		
		enterprise.GET("/db/transaction/:id", func(c *gin.Context) {
			// Implementation for retrieving blockchain transactions from external databases
		})

		// Cloud integration
		enterprise.POST("/cloud/upload", func(c *gin.Context) {
			// Implementation for uploading data to cloud storage
		})
		
		enterprise.POST("/cloud/webhook", func(c *gin.Context) {
			// Implementation for receiving cloud webhooks
		})
	}

	// Start server
    srv := &http.Server{
        Addr:    fmt.Sprintf(":%d", cfg.Server.Port),
        Handler: r,
    }

	// Run server in a goroutine
    go func() {
        log.Printf("Starting server on port %d", cfg.Server.Port)
        if err := srv.ListenAndServe(); err != nil && err != http.ErrServerClosed {
            log.Fatalf("Failed to start server: %v", err)
        }
    }()

	// Wait for interrupt signal to gracefully shutdown the server
	quit := make(chan os.Signal, 1)
	signal.Notify(quit, syscall.SIGINT, syscall.SIGTERM)
	<-quit
	log.Println("Shutting down server...")

	// The context is used to inform the server it has 5 seconds to finish
	// the request it is currently handling
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	if err := srv.Shutdown(ctx); err != nil {
		log.Fatal("Server forced to shutdown:", err)
	}

	log.Println("Server exiting")
}