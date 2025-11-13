package middleware

import (
    "context"
    "net/http"
    "time"
    "strconv"

    "github.com/gin-gonic/gin"
    redis "github.com/redis/go-redis/v9"
)

func MaxBodyBytes(limit int64) gin.HandlerFunc {
	return func(c *gin.Context) {
		c.Request.Body = http.MaxBytesReader(c.Writer, c.Request.Body, limit)
		c.Next()
	}
}

// Simple token bucket per route+client
type bucket struct {
	tokens int
	last   int64
}

var limiter = make(map[string]*bucket)

func RateLimit(reqPerMin int) gin.HandlerFunc {
    return func(c *gin.Context) {
        key := c.FullPath() + "|" + c.ClientIP()
        b := limiter[key]
        now := time.Now().Unix()
        if b == nil {
            b = &bucket{tokens: reqPerMin, last: now}
            limiter[key] = b
        }
        if now-b.last >= 60 {
            b.tokens = reqPerMin
            b.last = now
        }
        if b.tokens <= 0 {
            c.AbortWithStatusJSON(http.StatusTooManyRequests, gin.H{"error": "rate limit exceeded"})
            return
        }
        b.tokens--
        c.Next()
    }
}

// RateLimitRedis provides distributed rate limiting using Redis per route+IP.
func RateLimitRedis(reqPerMin int, rdb *redis.Client) gin.HandlerFunc {
    if rdb == nil || reqPerMin <= 0 { return RateLimit(reqPerMin) }
    // Lua script to atomically increment and set TTL
    script := redis.NewScript(`
        local count = redis.call('INCR', KEYS[1])
        if count == 1 then redis.call('EXPIRE', KEYS[1], 60) end
        return count
    `)
    return func(c *gin.Context) {
        bucket := time.Now().Unix() / 60
        key := "rl:" + c.FullPath() + ":" + c.ClientIP() + ":" + strconv.FormatInt(bucket, 10)
        ctx := context.Background()
        res, err := script.Run(ctx, rdb, []string{key}).Result()
        if err != nil {
            // Allow request on Redis error
            c.Next()
            return
        }
        count, _ := res.(int64)
        if int(count) > reqPerMin {
            c.AbortWithStatusJSON(http.StatusTooManyRequests, gin.H{"error": "rate limit exceeded"})
            return
        }
        c.Next()
    }
}
