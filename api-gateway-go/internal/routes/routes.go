package routes

import (
    "net/http"
    "strings"

    "github.com/gin-gonic/gin"

    "garp/api-gateway-go/internal/auth"
    "garp/api-gateway-go/internal/config"
    "garp/api-gateway-go/internal/services"
)

// Register wires up health/readiness and proxy routes.
func Register(r *gin.Engine, cfg config.Config) {
    // Health and readiness
    r.GET("/health", func(c *gin.Context) {
        c.JSON(http.StatusOK, gin.H{"status": "ok"})
    })
    r.GET("/ready", func(c *gin.Context) {
        c.JSON(http.StatusOK, gin.H{"status": "ready"})
    })

    // Global middleware: simple bearer auth (skips health/ready)
    r.Use(auth.AuthMiddleware(cfg.JWTSecret))

    // Prepare reverse proxies
    backendProxy, _ := services.NewReverseProxy(cfg.BackendURL)
    participantProxy, _ := services.NewReverseProxy(cfg.ParticipantURL)
    syncProxy, _ := services.NewReverseProxy(cfg.GlobalSyncURL)

    // Backend proxy: /backend/*path -> BACKEND_URL/*path
    r.Any("/backend/*path", func(c *gin.Context) {
        if backendProxy == nil {
            c.JSON(http.StatusBadGateway, gin.H{"success": false, "error": "backend not configured"})
            return
        }
        // Rewrite URL path to upstream
        upstreamPath := ensureLeadingSlash(c.Param("path"))
        c.Request.URL.Path = upstreamPath
        backendProxy.ServeHTTP(c.Writer, c.Request)
    })

    // Participant proxy: /participant/*path -> PARTICIPANT_URL/*path
    r.Any("/participant/*path", func(c *gin.Context) {
        if participantProxy == nil {
            c.JSON(http.StatusBadGateway, gin.H{"success": false, "error": "participant not configured"})
            return
        }
        upstreamPath := ensureLeadingSlash(c.Param("path"))
        c.Request.URL.Path = upstreamPath
        participantProxy.ServeHTTP(c.Writer, c.Request)
    })

    // Global synchronizer proxy (optional): /sync/*path -> GLOBAL_SYNC_URL/*path
    r.Any("/sync/*path", func(c *gin.Context) {
        if syncProxy == nil {
            c.JSON(http.StatusBadGateway, gin.H{"success": false, "error": "global sync not configured"})
            return
        }
        upstreamPath := ensureLeadingSlash(c.Param("path"))
        c.Request.URL.Path = upstreamPath
        syncProxy.ServeHTTP(c.Writer, c.Request)
    })
}

func ensureLeadingSlash(p string) string {
    if p == "" {
        return "/"
    }
    if strings.HasPrefix(p, "/") {
        return p
    }
    return "/" + p
}