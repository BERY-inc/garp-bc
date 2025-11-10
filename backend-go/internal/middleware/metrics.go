package middleware

import (
    "time"
    "github.com/gin-gonic/gin"
    prom "github.com/prometheus/client_golang/prometheus"
    promhttp "github.com/prometheus/client_golang/prometheus/promhttp"
)

var (
    httpRequests = prom.NewCounterVec(
        prom.CounterOpts{Name: "backend_http_requests_total", Help: "Total HTTP requests"},
        []string{"path","method","status"},
    )
    httpLatency = prom.NewHistogramVec(
        prom.HistogramOpts{Name: "backend_http_request_duration_seconds", Help: "Request latency", Buckets: prom.DefBuckets},
        []string{"path","method"},
    )
)

func init() {
    prom.MustRegister(httpRequests)
    prom.MustRegister(httpLatency)
}

func MetricsMiddleware() gin.HandlerFunc {
    return func(c *gin.Context) {
        start := time.Now()
        c.Next()
        dur := time.Since(start).Seconds()
        path := c.FullPath()
        method := c.Request.Method
        status := c.Writer.Status()
        httpRequests.WithLabelValues(path, method, itoa(status)).Inc()
        httpLatency.WithLabelValues(path, method).Observe(dur)
    }
}

func MetricsHandler() gin.HandlerFunc {
    h := promhttp.Handler()
    return func(c *gin.Context) { h.ServeHTTP(c.Writer, c.Request) }
}

func itoa(i int) string {
    if i == 0 { return "0" }
    b := [12]byte{}
    pos := len(b)
    for i > 0 { pos--; b[pos] = byte('0' + i%10); i /= 10 }
    return string(b[pos:])
}