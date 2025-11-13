package services

import (
    "fmt"
    "log"
    "net/http"
    "net/http/httputil"
    "net/url"
    "strings"
    "time"
)

// ProxyService wraps httputil.ReverseProxy with additional functionality
type ProxyService struct {
    proxy  *httputil.ReverseProxy
    target *url.URL
    name   string
}

// NewReverseProxy creates a reverse proxy to the given base URL.
func NewReverseProxy(base, name string) (*ProxyService, error) {
    if base == "" {
        return nil, nil
    }
    target, err := url.Parse(base)
    if err != nil {
        return nil, err
    }

    director := func(req *http.Request) {
        // Preserve original path after the prefix and join with target
        req.URL.Scheme = target.Scheme
        req.URL.Host = target.Host
        // If the target has a base path, join it
        if target.Path != "" && target.Path != "/" {
            req.URL.Path = singleJoiningSlash(target.Path, req.URL.Path)
        }
        // Adjust Host header for upstream
        req.Host = target.Host
        // Add identification header
        req.Header.Set("X-Forwarded-By", "GARP-API-Gateway")
        req.Header.Set("X-Forwarded-For", req.RemoteAddr)
    }

    // Create reverse proxy with custom error handler
    proxy := &httputil.ReverseProxy{
        Director: director,
        ErrorHandler: func(w http.ResponseWriter, r *http.Request, err error) {
            log.Printf("Proxy error for %s: %v", name, err)
            w.WriteHeader(http.StatusBadGateway)
            w.Write([]byte(fmt.Sprintf("{\"error\": \"upstream service %s error: %v\"}", name, err)))
        },
        ModifyResponse: func(resp *http.Response) error {
            // Log response details
            log.Printf("Proxy response from %s: %s %s -> %d", name, resp.Request.Method, resp.Request.URL.Path, resp.StatusCode)
            return nil
        },
    }

    return &ProxyService{
        proxy:  proxy,
        target: target,
        name:   name,
    }, nil
}

// ServeHTTP implements the http.Handler interface
func (p *ProxyService) ServeHTTP(w http.ResponseWriter, r *http.Request) {
    start := time.Now()
    log.Printf("Proxying request to %s: %s %s", p.name, r.Method, r.URL.Path)
    
    // Add request ID for tracing
    r.Header.Set("X-Request-ID", generateRequestID())
    
    p.proxy.ServeHTTP(w, r)
    
    duration := time.Since(start)
    log.Printf("Completed proxy request to %s in %v", p.name, duration)
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

// generateRequestID creates a simple request ID (in production, use a proper UUID library)
func generateRequestID() string {
    return fmt.Sprintf("%d", time.Now().UnixNano())
}