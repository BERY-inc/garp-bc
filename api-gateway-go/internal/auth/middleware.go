package auth

import (
    "crypto/hmac"
    "crypto/sha256"
    "net/http"
    "strings"
    "time"

    "github.com/gin-gonic/gin"
    "github.com/golang-jwt/jwt/v5"
)

// AuthMiddleware performs JWT token validation using the provided secret.
// Health and readiness endpoints are always allowed.
// If require is false, authentication is skipped (except health/ready which are always allowed).
func AuthMiddleware(secret string, require bool) gin.HandlerFunc {
    return func(c *gin.Context) {
        // Allow health/readiness without auth
        if c.Request.Method == http.MethodGet && (c.FullPath() == "/health" || c.FullPath() == "/ready") {
            c.Next()
            return
        }
        if !require {
            c.Next()
            return
        }

        auth := c.GetHeader("Authorization")
        if !strings.HasPrefix(auth, "Bearer ") {
            c.AbortWithStatusJSON(http.StatusUnauthorized, gin.H{"success": false, "error": "missing bearer token"})
            return
        }
        tokenString := strings.TrimPrefix(auth, "Bearer ")

        // Try to parse as JWT first
        token, err := jwt.Parse(tokenString, func(token *jwt.Token) (interface{}, error) {
            // Validate the signing method
            if _, ok := token.Method.(*jwt.SigningMethodHMAC); !ok {
                return nil, jwt.ErrSignatureInvalid
            }
            return []byte(secret), nil
        })

        if err != nil || !token.Valid {
            c.AbortWithStatusJSON(http.StatusUnauthorized, gin.H{"success": false, "error": "invalid token"})
            return
        }

        c.Next()
    }
}

// hmacEqual compares two strings using HMAC to prevent timing attacks
func hmacEqual(input, secret string) bool {
    h := hmac.New(sha256.New, []byte(secret))
    h.Write([]byte(input))
    inputHash := h.Sum(nil)

    h = hmac.New(sha256.New, []byte(secret))
    h.Write([]byte(secret))
    secretHash := h.Sum(nil)

    return hmac.Equal(inputHash, secretHash)
}

// GenerateToken creates a new JWT token for testing purposes
func GenerateToken(secret string, claims jwt.MapClaims) (string, error) {
    if claims["exp"] == nil {
        claims["exp"] = time.Now().Add(time.Hour * 24).Unix()
    }
    if claims["iat"] == nil {
        claims["iat"] = time.Now().Unix()
    }

    token := jwt.NewWithClaims(jwt.SigningMethodHS256, claims)
    return token.SignedString([]byte(secret))
}