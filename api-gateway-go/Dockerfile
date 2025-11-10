# Build stage
FROM golang:1.22 AS builder
WORKDIR /app
COPY . .
ENV CGO_ENABLED=0
RUN go mod download
RUN go build -o /bin/garp-api ./cmd/server

# Runtime stage
FROM alpine:3.19
RUN adduser -D -u 10001 appuser
COPY --from=builder /bin/garp-api /bin/garp-api
USER appuser
EXPOSE 8080
ENTRYPOINT ["/bin/garp-api"]