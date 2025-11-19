FROM golang:1.21 as build
WORKDIR /app
COPY go.mod ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 GOOS=linux go build -a -installsuffix cgo -o garp-backend ./cmd/participant

FROM alpine:latest
RUN apk --no-cache add ca-certificates
WORKDIR /root/
COPY --from=build /app/garp-backend .
# Copy migrations from monorepo root into the path the app expects
COPY migrations/ backend-go/migrations/
EXPOSE 8081
CMD ["./garp-backend"]