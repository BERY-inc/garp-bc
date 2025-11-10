#!/bin/bash

# Development setup script for GARP Blockchain Platform

set -e

echo "Setting up GARP Blockchain Platform development environment..."

# Check if required tools are installed
echo "Checking for required tools..."

if ! command -v rustc &> /dev/null
then
    echo "Rust is not installed. Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    source $HOME/.cargo/env
fi

if ! command -v go &> /dev/null
then
    echo "Go is not installed. Please install Go 1.21+"
    exit 1
fi

if ! command -v docker &> /dev/null
then
    echo "Docker is not installed. Please install Docker"
    exit 1
fi

if ! command -v docker-compose &> /dev/null
then
    echo "Docker Compose is not installed. Please install Docker Compose"
    exit 1
fi

echo "All required tools are installed."

# Create necessary directories
echo "Creating necessary directories..."
mkdir -p target

# Run docker-compose for databases and services
echo "Starting development services..."
docker-compose up -d postgres redis

# Wait for services to be ready
echo "Waiting for services to be ready..."
until docker-compose exec postgres pg_isready &> /dev/null
do
    sleep 1
done

until docker-compose exec redis redis-cli ping &> /dev/null
do
    sleep 1
done

echo "Development environment is ready!"
echo ""
echo "You can now:"
echo "  - Run 'make build' to build all components"
echo "  - Run 'make run-dev' to start all services"
echo "  - Run 'make test' to run all tests"