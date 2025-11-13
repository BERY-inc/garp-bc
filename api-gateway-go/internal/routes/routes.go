package routes

import (
    "context"
    "log"
    "net/http"
    "strings"
    "time"
    "strconv"

    "github.com/gin-gonic/gin"
    redis "github.com/redis/go-redis/v9"

    "garp/api-gateway-go/internal/auth"
    "garp/api-gateway-go/internal/config"
    "garp/api-gateway-go/internal/services"
)

// SecurityHeadersMiddleware sets common security headers
func SecurityHeadersMiddleware() gin.HandlerFunc {
    return func(c *gin.Context) {
        c.Header("X-Content-Type-Options", "nosniff")
        c.Header("X-Frame-Options", "DENY")
        c.Header("Referrer-Policy", "no-referrer")
        // HSTS is effective only over HTTPS, harmless otherwise
        c.Header("Strict-Transport-Security", "max-age=31536000; includeSubDomains")
        c.Next()
    }
}

// CORSMiddleware adds CORS headers to responses, honoring configured allowed origins and credentials
func CORSMiddleware(cfg config.Config) gin.HandlerFunc {
    // Pre-process allowed origins list
    var allowed []string
    for _, p := range strings.Split(cfg.AllowedOrigins, ",") {
        t := strings.TrimSpace(p)
        if t != "" { allowed = append(allowed, t) }
    }
    allowAll := len(allowed) == 0
    return func(c *gin.Context) {
        origin := c.GetHeader("Origin")
        var allowOrigin string
        matched := false
        if allowAll {
            allowOrigin = "*"
            matched = origin != ""
        } else {
            for _, o := range allowed {
                if origin == o {
                    allowOrigin = origin
                    matched = true
                    break
                }
            }
        }
        if allowOrigin != "" {
            c.Header("Access-Control-Allow-Origin", allowOrigin)
        }
        c.Header("Vary", "Origin, Access-Control-Request-Method, Access-Control-Request-Headers")
        c.Header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
        c.Header("Access-Control-Allow-Headers", "Origin, Content-Type, Authorization, X-Requested-With, Accept")
        c.Header("Access-Control-Expose-Headers", "Content-Length, Cache-Control, Content-Language, Content-Type")
        // Only allow credentials when a specific origin matched
        if cfg.AllowCredentials && !allowAll && matched {
            c.Header("Access-Control-Allow-Credentials", "true")
        }

        if c.Request.Method == http.MethodOptions {
            c.AbortWithStatus(http.StatusNoContent)
            return
        }

        c.Next()
    }
}

// LoggingMiddleware logs request details
func LoggingMiddleware() gin.HandlerFunc {
    return func(c *gin.Context) {
        start := time.Now()
        path := c.Request.URL.Path
        raw := c.Request.URL.RawQuery

        // Process request
        c.Next()

        // Log request details
        end := time.Now()
        latency := end.Sub(start)
        clientIP := c.ClientIP()
        method := c.Request.Method
        statusCode := c.Writer.Status()
        errorMessage := c.Errors.ByType(gin.ErrorTypePrivate).String()

        if raw != "" {
            path = path + "?" + raw
        }

        log.Printf("[GIN] %v | %3d | %13v | %15s | %-7s %#v\n%s",
            end.Format("2006/01/02 - 15:04:05"),
            statusCode,
            latency,
            clientIP,
            method,
            path,
            errorMessage,
        )
    }
}

// RateLimitMiddleware limits requests per IP
func RateLimitMiddleware(cfg config.Config) gin.HandlerFunc {
    // Distributed rate limit using Redis per IP per minute
    rpm := cfg.RateLimitRPM
    if rpm <= 0 { rpm = 100 }

    var rdb *redis.Client
    if cfg.RedisURL != "" {
        if opt, err := redis.ParseURL(cfg.RedisURL); err == nil {
            rdb = redis.NewClient(opt)
        } else {
            log.Printf("rate limit: invalid Redis URL: %v", err)
        }
    }

    // Fallback in-memory limiter if Redis unavailable
    if rdb == nil {
        log.Printf("rate limit: Redis unavailable, using local in-memory limiter")
        var visitors = make(map[string][]time.Time)
        return func(c *gin.Context) {
            ip := c.ClientIP()
            route := c.FullPath()
            now := time.Now()
            key := route + "|" + ip
            if times, exists := visitors[key]; exists {
                var validTimes []time.Time
                for _, t := range times {
                    if now.Sub(t) < time.Minute { validTimes = append(validTimes, t) }
                }
                visitors[key] = validTimes
            }
            if len(visitors[key]) >= rpm {
                c.AbortWithStatusJSON(http.StatusTooManyRequests, gin.H{"error": "rate limit exceeded"})
                return
            }
            visitors[key] = append(visitors[key], now)
            c.Next()
        }
    }

    // Lua script ensures atomic increment with TTL
    script := redis.NewScript(`
        local count = redis.call('INCR', KEYS[1])
        if count == 1 then
            redis.call('EXPIRE', KEYS[1], ARGV[1])
        end
        return count
    `)

    return func(c *gin.Context) {
        ip := c.ClientIP()
        route := c.FullPath()
        // Key per minute bucket
        now := time.Now()
        bucket := now.Unix() / 60
        key := "rl:" + route + ":" + ip + ":" + strconv.FormatInt(bucket, 10)

        // Execute script: TTL 60 seconds
        ctx := context.Background()
        res, err := script.Run(ctx, rdb, []string{key}, 60).Result()
        if err != nil {
            // Soft-fail: allow request but log
            log.Printf("rate limit: redis error: %v", err)
            c.Next()
            return
        }
        count, _ := res.(int64)
        if int(count) > rpm {
            c.AbortWithStatusJSON(http.StatusTooManyRequests, gin.H{"error": "rate limit exceeded"})
            return
        }
        c.Next()
    }
}

// Register wires up health/readiness and proxy routes.
func Register(r *gin.Engine, cfg config.Config) {
    // Add global middleware
    r.Use(LoggingMiddleware())
    r.Use(SecurityHeadersMiddleware())
    r.Use(CORSMiddleware(cfg))
    r.Use(RateLimitMiddleware(cfg))
    r.Use(gin.Recovery())

    // Health and readiness
    r.GET("/health", func(c *gin.Context) {
        c.JSON(http.StatusOK, gin.H{"status": "ok"})
    })
    r.GET("/ready", func(c *gin.Context) {
        c.JSON(http.StatusOK, gin.H{"status": "ready"})
    })

    // Global middleware: bearer auth (skips health/ready)
    r.Use(auth.AuthMiddleware(cfg.JWTSecret, cfg.AuthRequired))

    // Prepare reverse proxies
    backendProxy, err := services.NewReverseProxy(cfg.BackendURL, "backend")
    if err != nil {
        log.Printf("Failed to create backend proxy: %v", err)
    }
    participantProxy, err := services.NewReverseProxy(cfg.ParticipantURL, "participant")
    if err != nil {
        log.Printf("Failed to create participant proxy: %v", err)
    }
    syncProxy, err := services.NewReverseProxy(cfg.GlobalSyncURL, "sync")
    if err != nil {
        log.Printf("Failed to create sync proxy: %v", err)
    }

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