package config

import (
    "os"
    "strings"
    "github.com/BurntSushi/toml"
)

type Config struct {
    Server struct {
        Port int `toml:"port"`
    } `toml:"server"`
    Participant struct {
        BaseURL string `toml:"base_url"`
    } `toml:"participant"`
    Synchronizer struct {
        BaseURL string `toml:"base_url"`
    } `toml:"synchronizer"`
    Database struct {
        PostgresURL string `toml:"postgres_url"`
        RedisURL    string `toml:"redis_url"`
    } `toml:"database"`
    TLS struct {
        ClientCert string `toml:"client_cert"`
        ClientKey  string `toml:"client_key"`
        CACert     string `toml:"ca_cert"`
    } `toml:"tls"`
    OTEL struct {
        Endpoint string `toml:"endpoint"`
        ServiceName string `toml:"service_name"`
    } `toml:"otel"`
}

func Default() Config {
    var c Config
    c.Server.Port = 8081
    c.Participant.BaseURL = "http://participant:8090"
    c.Synchronizer.BaseURL = "http://synchronizer:8000"
    c.Database.PostgresURL = "postgres://postgres:postgres@db:5432/garp?sslmode=disable"
    c.Database.RedisURL = "redis://redis:6379"
    c.TLS.ClientCert = ""
    c.TLS.ClientKey = ""
    c.TLS.CACert = ""
    c.OTEL.Endpoint = ""
    c.OTEL.ServiceName = "garp-backend"
    return c
}

func LoadFile(path string, out *Config) error {
    _, err := toml.DecodeFile(path, out)
    return err
}

func ApplyEnv(out *Config) {
    // Simple env overrides
    if v := os.Getenv("BACKEND_PORT"); v != "" { out.Server.Port = atoiSafe(v, out.Server.Port) }
    if v := os.Getenv("PARTICIPANT_URL"); v != "" { out.Participant.BaseURL = v }
    if v := os.Getenv("SYNCHRONIZER_URL"); v != "" { out.Synchronizer.BaseURL = v }
    if v := os.Getenv("POSTGRES_URL"); v != "" { out.Database.PostgresURL = v }
    if v := os.Getenv("REDIS_URL"); v != "" { out.Database.RedisURL = v }
    if v := os.Getenv("TLS_CLIENT_CERT"); v != "" { out.TLS.ClientCert = v }
    if v := os.Getenv("TLS_CLIENT_KEY"); v != "" { out.TLS.ClientKey = v }
    if v := os.Getenv("TLS_CA_CERT"); v != "" { out.TLS.CACert = v }
    if v := os.Getenv("OTEL_ENDPOINT"); v != "" { out.OTEL.Endpoint = v }
    if v := os.Getenv("OTEL_SERVICE_NAME"); v != "" { out.OTEL.ServiceName = v }
}

func atoiSafe(s string, def int) int {
    n := 0
    for _, ch := range strings.TrimSpace(s) { if ch < '0' || ch > '9' { return def } ; n = n*10 + int(ch-'0') }
    return n
}