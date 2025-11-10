package otel

import (
    "context"
    "log"
    "github.com/gin-gonic/gin"
    "go.opentelemetry.io/otel"
    sdktrace "go.opentelemetry.io/otel/sdk/trace"
    "go.opentelemetry.io/otel/exporters/otlp/otlptrace/otlptracehttp"
)

func Init(endpoint string, serviceName string) func(context.Context) error {
    if endpoint == "" { return func(ctx context.Context) error { return nil } }
    exp, err := otlptracehttp.New(context.Background(), otlptracehttp.WithEndpoint(endpoint), otlptracehttp.WithInsecure())
    if err != nil { log.Printf("otel exporter init failed: %v", err); return func(ctx context.Context) error { return nil } }
    tp := sdktrace.NewTracerProvider(sdktrace.WithBatcher(exp))
    otel.SetTracerProvider(tp)
    return tp.Shutdown
}

func Middleware(serviceName string) gin.HandlerFunc {
    tracer := otel.Tracer(serviceName)
    return func(c *gin.Context) {
        ctx, span := tracer.Start(c.Request.Context(), c.FullPath())
        defer span.End()
        c.Request = c.Request.WithContext(ctx)
        c.Next()
    }
}