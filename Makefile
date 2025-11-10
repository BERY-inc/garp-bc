# Makefile for GARP Blockchain Project

# Default target
.PHONY: help
help:
	@echo "GARP Blockchain Project - Makefile"
	@echo ""
	@echo "Available targets:"
	@echo "  help          - Show this help message"
	@echo "  test          - Run tests for all components"
	@echo "  build         - Build all components"
	@echo "  run-dev       - Run development environment with docker-compose"
	@echo "  stop-dev      - Stop development environment"
	@echo "  clean         - Clean build artifacts"
	@echo "  ci-cd         - Run CI/CD pipeline locally"

# Test all components
.PHONY: test
test:
	@echo "Running tests for all components..."
	cargo test --workspace
	cd backend-go && go test -v ./...

# Build all components
.PHONY: build
build:
	@echo "Building all components..."
	cargo build --release
	cd backend-go && go build -o garp-backend ./cmd/participant

# Run development environment
.PHONY: run-dev
run-dev:
	@echo "Starting development environment..."
	docker-compose up -d
	@echo "Development environment started!"
	@echo "Services:"
	@echo "  PostgreSQL: localhost:5432"
	@echo "  Redis: localhost:6379"
	@echo "  Participant Node: localhost:8090"
	@echo "  Backend API: localhost:8081"

# Stop development environment
.PHONY: stop-dev
stop-dev:
	@echo "Stopping development environment..."
	docker-compose down
	@echo "Development environment stopped."

# Clean build artifacts
.PHONY: clean
clean:
	@echo "Cleaning build artifacts..."
	cargo clean
	rm -f backend-go/garp-backend
	rm -f backend-go/garp-backend.exe

# Run CI/CD pipeline locally
.PHONY: ci-cd
ci-cd:
	@echo "Running CI/CD pipeline locally..."
	# This would typically run your CI/CD tools locally
	# For now, we'll just run tests and build
	$(MAKE) test
	$(MAKE) build

# Install dependencies
.PHONY: install-deps
install-deps:
	@echo "Installing dependencies..."
	# Install Rust
	curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
	# Install Go
	# This would depend on your OS, on Ubuntu: sudo apt-get install golang
	# On macOS with Homebrew: brew install go