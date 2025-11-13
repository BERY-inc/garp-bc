package config

import (
    "os"
    "strings"
    "strconv"
)

// Config defines API gateway configuration.
type Config struct {
    Port            int    // Gateway listen port (default 8080)
    BackendURL      string // Base URL for backend-go service
    ParticipantURL  string // Base URL for participant-node service
    GlobalSyncURL   string // Optional base URL for global-synchronizer service
    JWTSecret       string // Simple bearer token secret
    AllowedOrigins  string // Comma-separated list of allowed origins for CORS; empty means "*"
    AllowCredentials bool  // Whether to allow credentials in CORS responses (only with specific origins)
    AuthRequired    bool   // Require authentication for non-health endpoints
    RedisURL        string // Redis URL for distributed rate limiting
    RateLimitRPM    int    // Requests per minute per IP
}

// LoadFromEnv constructs Config using environment variables with sensible defaults.
// BACKEND_URL, PARTICIPANT_URL, GLOBAL_SYNC_URL, JWT_SECRET, API_PORT
func LoadFromEnv() Config {
    port := getenvInt("API_PORT", 8080)
    return Config{
        Port:             port,
        BackendURL:       getenv("BACKEND_URL", "http://garp-backend:8081"),
        ParticipantURL:   getenv("PARTICIPANT_URL", "http://garp-participant:8090"),
        GlobalSyncURL:    getenv("GLOBAL_SYNC_URL", ""),
        JWTSecret:        getenv("JWT_SECRET", ""),
        AllowedOrigins:   getenv("ALLOWED_ORIGINS", ""),
        AllowCredentials: getenvBool("CORS_ALLOW_CREDENTIALS", false),
        AuthRequired:     getenvBool("AUTH_REQUIRED", true),
        RedisURL:         getenv("REDIS_URL", "redis://redis:6379"),
        RateLimitRPM:     getenvInt("RATE_LIMIT_RPM", 100),
    }
}

// PortString returns the port as a string for binding.
func (c Config) PortString() string { return itoa(c.Port) }

// Helpers
func getenv(key, def string) string {
    if v := os.Getenv(key); v != "" {
        return v
    }
    return def
}

func getenvInt(key string, def int) int {
    if v := os.Getenv(key); v != "" {
        if n, err := strconv.Atoi(v); err == nil {
            return n
        }
    }
    return def
}

func getenvBool(key string, def bool) bool {
    v := strings.TrimSpace(os.Getenv(key))
    if v == "" { return def }
    switch strings.ToLower(v) {
    case "1", "true", "yes", "y", "on":
        return true
    case "0", "false", "no", "n", "off":
        return false
    default:
        return def
    }
}

// Minimal helpers to avoid external deps for trivial conversions
func itoa(n int) string {
    return strconv.Itoa(n)
}