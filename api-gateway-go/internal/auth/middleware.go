package auth

import (
    "net/http"
    "strings"

    "github.com/gin-gonic/gin"
)

// AuthMiddleware performs a simple bearer token check using the provided secret.
// If secret is empty, the middleware allows all requests.
// Health and readiness endpoints are always allowed.
func AuthMiddleware(secret string) gin.HandlerFunc {
    return func(c *gin.Context) {
        // Allow health/readiness without auth
        if c.Request.Method == http.MethodGet && (c.FullPath() == "/health" || c.FullPath() == "/ready") {
            c.Next()
            return
        }

        if secret == "" {
            c.Next()
            return
        }

        auth := c.GetHeader("Authorization")
        if !strings.HasPrefix(auth, "Bearer ") {
            c.AbortWithStatusJSON(http.StatusUnauthorized, gin.H{"success": false, "error": "missing bearer token"})
            return
        }
        token := strings.TrimPrefix(auth, "Bearer ")
        if token != secret {
            c.AbortWithStatusJSON(http.StatusUnauthorized, gin.H{"success": false, "error": "invalid token"})
            return
        }

        c.Next()
    }
}