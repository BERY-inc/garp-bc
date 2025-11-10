package middleware

import (
	"net/http"
	"time"

	"github.com/gin-gonic/gin"
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
