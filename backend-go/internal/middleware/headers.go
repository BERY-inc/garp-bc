package middleware

import (
    "github.com/gin-gonic/gin"
)

// SecurityHeaders adds a baseline set of security headers to all responses.
func SecurityHeaders() gin.HandlerFunc {
    return func(c *gin.Context) {
        // Mitigate MIME sniffing
        c.Header("X-Content-Type-Options", "nosniff")
        // Prevent clickjacking
        c.Header("X-Frame-Options", "DENY")
        // Limit referrer leakage
        c.Header("Referrer-Policy", "no-referrer")
        // HSTS for HTTPS deployments
        c.Header("Strict-Transport-Security", "max-age=31536000; includeSubDomains")
        // Conservative permissions policy (no powerful APIs by default)
        c.Header("Permissions-Policy", "geolocation=(), microphone=(), camera=(), accelerometer=(), gyroscope=(), usb=(), magnetometer=(), midi=(), payment=(), fullscreen=(self)")
        c.Next()
    }
}