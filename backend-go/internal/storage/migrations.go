package storage

import (
    "context"
    "os"
    "path/filepath"
)

// RunMigrations ensures required tables exist.
func (s *Storage) RunMigrations(ctx context.Context) error {
    if s.PG == nil { return nil }
    base := filepath.Join("backend-go","migrations")
    entries, err := os.ReadDir(base)
    if err != nil { return err }
    for _, e := range entries {
        if e.IsDir() { continue }
        path := filepath.Join(base, e.Name())
        sql, err := os.ReadFile(path)
        if err != nil { return err }
        if _, err := s.PG.Exec(ctx, string(sql)); err != nil { return err }
    }
    return nil
}