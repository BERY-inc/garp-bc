package main

import (
    "log"
    "net/http"

    "github.com/gin-gonic/gin"

    cfg "garp/api-gateway-go/internal/config"
    "garp/api-gateway-go/internal/routes"
)

func main() {
    // Load configuration from environment
    config := cfg.LoadFromEnv()

    // Use release mode in production; can be overridden by env if desired
    gin.SetMode(gin.ReleaseMode)
    r := gin.New()
    r.Use(gin.Recovery())

    // Build router with health/readiness and proxy routes
    routes.Register(r, config)

    addr := ":" + config.PortString()
    log.Printf("Starting API Gateway on %s", addr)
    if err := r.Run(addr); err != nil && err != http.ErrServerClosed {
        log.Fatalf("gateway server error: %v", err)
    }
}