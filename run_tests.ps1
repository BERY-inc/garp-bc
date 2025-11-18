# Script to run GARP blockchain platform tests
Write-Host "GARP Blockchain Platform - Test Runner"
Write-Host "======================================"

# Check if we're in the correct directory
if (-not (Test-Path "Cargo.toml")) {
    Write-Host "Error: Cargo.toml not found. Please run this script from the root of the GARP project."
    exit 1
}

# Display available test options
Write-Host "Available test options:"
Write-Host "1. Run all tests"
Write-Host "2. Run bridge-specific tests"
Write-Host "3. Run unit tests only"
Write-Host "4. Run integration tests only"
Write-Host "5. Check syntax only (without running tests)"
Write-Host ""

# Get user choice
$choice = Read-Host "Select an option (1-5)"

switch ($choice) {
    1 {
        Write-Host "Running all tests..."
        cargo test
    }
    2 {
        Write-Host "Running bridge-specific tests..."
        cargo test bridge
    }
    3 {
        Write-Host "Running unit tests..."
        cargo test --lib
    }
    4 {
        Write-Host "Running integration tests..."
        cargo test --test "*"
    }
    5 {
        Write-Host "Checking syntax only..."
        cargo check --tests
    }
    default {
        Write-Host "Invalid option. Please select a number between 1 and 5."
        exit 1
    }
}

Write-Host ""
Write-Host "Test execution completed."