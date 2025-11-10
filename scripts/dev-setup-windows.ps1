# Development setup script for GARP Blockchain Platform (Windows)

Write-Host "Setting up GARP Blockchain Platform development environment..." -ForegroundColor Green

# Check if required tools are installed
Write-Host "Checking for required tools..." -ForegroundColor Yellow

# Check for Rust
try {
    $rustVersion = rustc --version
    Write-Host "Rust is installed: $rustVersion" -ForegroundColor Green
} catch {
    Write-Host "Rust is not installed. Please install Rust from https://www.rust-lang.org/" -ForegroundColor Red
    exit 1
}

# Check for Go
try {
    $goVersion = go version
    Write-Host "Go is installed: $goVersion" -ForegroundColor Green
} catch {
    Write-Host "Go is not installed. Please install Go 1.21+ from https://golang.org/" -ForegroundColor Red
    exit 1
}

# Check for Docker
try {
    $dockerVersion = docker --version
    Write-Host "Docker is installed: $dockerVersion" -ForegroundColor Green
} catch {
    Write-Host "Docker is not installed. Please install Docker Desktop from https://www.docker.com/" -ForegroundColor Red
    exit 1
}

# Check for Docker Compose
try {
    $dockerComposeVersion = docker-compose --version
    Write-Host "Docker Compose is installed: $dockerComposeVersion" -ForegroundColor Green
} catch {
    Write-Host "Docker Compose is not installed. Please install Docker Compose" -ForegroundColor Red
    exit 1
}

Write-Host "All required tools are installed." -ForegroundColor Green

# Create necessary directories
Write-Host "Creating necessary directories..." -ForegroundColor Yellow
New-Item -ItemType Directory -Path "target" -Force | Out-Null

# Run docker-compose for databases and services
Write-Host "Starting development services..." -ForegroundColor Yellow
docker-compose up -d postgres redis

# Wait for services to be ready
Write-Host "Waiting for services to be ready..." -ForegroundColor Yellow
do {
    Start-Sleep -Seconds 1
    $postgresReady = docker-compose exec postgres pg_isready 2>$null
} while ($LASTEXITCODE -ne 0)

do {
    Start-Sleep -Seconds 1
    $redisReady = docker-compose exec redis redis-cli ping 2>$null
} while ($LASTEXITCODE -ne 0)

Write-Host "Development environment is ready!" -ForegroundColor Green
Write-Host ""
Write-Host "You can now:" -ForegroundColor Yellow
Write-Host "  - Run 'make build' to build all components" -ForegroundColor Cyan
Write-Host "  - Run 'make run-dev' to start all services" -ForegroundColor Cyan
Write-Host "  - Run 'make test' to run all tests" -ForegroundColor Cyan