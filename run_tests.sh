#!/bin/bash
# Script to run GARP blockchain platform tests

echo "GARP Blockchain Platform - Test Runner"
echo "======================================"

# Check if we're in the correct directory
if [ ! -f "Cargo.toml" ]; then
    echo "Error: Cargo.toml not found. Please run this script from the root of the GARP project."
    exit 1
fi

# Display available test options
echo "Available test options:"
echo "1. Run all tests"
echo "2. Run bridge-specific tests"
echo "3. Run unit tests only"
echo "4. Run integration tests only"
echo "5. Check syntax only (without running tests)"
echo ""

# Get user choice
read -p "Select an option (1-5): " choice

case $choice in
    1)
        echo "Running all tests..."
        cargo test
        ;;
    2)
        echo "Running bridge-specific tests..."
        cargo test bridge
        ;;
    3)
        echo "Running unit tests..."
        cargo test --lib
        ;;
    4)
        echo "Running integration tests..."
        cargo test --test "*"
        ;;
    5)
        echo "Checking syntax only..."
        cargo check --tests
        ;;
    *)
        echo "Invalid option. Please select a number between 1 and 5."
        exit 1
        ;;
esac

echo ""
echo "Test execution completed."