package config

import (
    "os"
)

// Config defines API gateway configuration.
type Config struct {
    Port            int    // Gateway listen port (default 8080)
    BackendURL      string // Base URL for backend-go service
    ParticipantURL  string // Base URL for participant-node service
    GlobalSyncURL   string // Optional base URL for global-synchronizer service
    JWTSecret       string // Simple bearer token secret
}

// LoadFromEnv constructs Config using environment variables with sensible defaults.
// BACKEND_URL, PARTICIPANT_URL, GLOBAL_SYNC_URL, JWT_SECRET, API_PORT
func LoadFromEnv() Config {
    port := getenvInt("API_PORT", 8080)
    return Config{
        Port:           port,
        BackendURL:     getenv("BACKEND_URL", "http://garp-backend:8081"),
        ParticipantURL: getenv("PARTICIPANT_URL", "http://garp-participant:8090"),
        GlobalSyncURL:  getenv("GLOBAL_SYNC_URL", ""),
        JWTSecret:      getenv("JWT_SECRET", ""),
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
        // Best-effort parse; fall back to default on error
        var n int
        _, err := fmtSscanf(v, &n)
        if err == nil {
            return n
        }
    }
    return def
}

// Minimal helpers to avoid external deps for trivial conversions
func itoa(n int) string {
    return fmtItoa(n)
}

// Lightweight internal fmt wrappers (implemented below) to avoid importing fmt in other files
// We still use fmt here locally to keep code simple.
// These wrappers exist only to maintain a tiny and explicit API surface.
// In real code you can just use fmt directly everywhere.

// --- local fmt wrappers ---
// (We keep them here to avoid spreading fmt imports across files.)
// Using standard library fmt underneath.

import "fmt"

func fmtItoa(n int) string { return fmt.Sprintf("%d", n) }

func fmtSscanf(s string, out *int) (int, error) { return fmt.Sscanf(s, "%d", out) }