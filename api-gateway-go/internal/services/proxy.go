package services

import (
    "net/http"
    "net/http/httputil"
    "net/url"
    "strings"
)

// NewReverseProxy creates a reverse proxy to the given base URL.
func NewReverseProxy(base string) (*httputil.ReverseProxy, error) {
    if base == "" {
        return nil, nil
    }
    target, err := url.Parse(base)
    if err != nil {
        return nil, err
    }

    director := func(req *http.Request) {
        // Preserve original path after the prefix and join with target
        // The handler should set req.URL.Path appropriately before proxying
        req.URL.Scheme = target.Scheme
        req.URL.Host = target.Host
        // If the target has a base path, join it
        if target.Path != "" && target.Path != "/" {
            req.URL.Path = singleJoiningSlash(target.Path, req.URL.Path)
        }
        // Adjust Host header for upstream
        req.Host = target.Host
    }
    return &httputil.ReverseProxy{Director: director}, nil
}

func singleJoiningSlash(a, b string) string {
    aslash := strings.HasSuffix(a, "/")
    bslash := strings.HasPrefix(b, "/")
    switch {
    case aslash && bslash:
        return a + b[1:]
    case !aslash && !bslash:
        return a + "/" + b
    }
    return a + b
}