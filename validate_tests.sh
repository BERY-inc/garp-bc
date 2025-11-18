#!/bin/bash
# Script to validate the syntax of Rust test files

echo "Validating Rust test files syntax..."

# Check wallet tests
echo "Checking wallet_tests.rs..."
rustc --edition=2021 --crate-type=lib --out-dir /tmp d:\garp\global-synchronizer\src\bridge\tests\wallet_tests.rs 2>/tmp/wallet_tests.err
if [ $? -eq 0 ]; then
    echo "✓ wallet_tests.rs syntax is valid"
else
    echo "✗ wallet_tests.rs has syntax errors:"
    cat /tmp/wallet_tests.err
fi

# Check liquidity tests
echo "Checking liquidity_tests.rs..."
rustc --edition=2021 --crate-type=lib --out-dir /tmp d:\garp\global-synchronizer\src\bridge\tests\liquidity_tests.rs 2>/tmp/liquidity_tests.err
if [ $? -eq 0 ]; then
    echo "✓ liquidity_tests.rs syntax is valid"
else
    echo "✗ liquidity_tests.rs has syntax errors:"
    cat /tmp/liquidity_tests.err
fi

# Check oracle tests
echo "Checking oracle_tests.rs..."
rustc --edition=2021 --crate-type=lib --out-dir /tmp d:\garp\global-synchronizer\src\bridge\tests\oracle_tests.rs 2>/tmp/oracle_tests.err
if [ $? -eq 0 ]; then
    echo "✓ oracle_tests.rs syntax is valid"
else
    echo "✗ oracle_tests.rs has syntax errors:"
    cat /tmp/oracle_tests.err
fi

# Check ethereum tests
echo "Checking ethereum_tests.rs..."
rustc --edition=2021 --crate-type=lib --out-dir /tmp d:\garp\global-synchronizer\src\bridge\tests\ethereum_tests.rs 2>/tmp/ethereum_tests.err
if [ $? -eq 0 ]; then
    echo "✓ ethereum_tests.rs syntax is valid"
else
    echo "✗ ethereum_tests.rs has syntax errors:"
    cat /tmp/ethereum_tests.err
fi

# Check integration tests
echo "Checking bridge_integration_tests.rs..."
rustc --edition=2021 --crate-type=lib --out-dir /tmp d:\garp\global-synchronizer\tests\bridge_integration_tests.rs 2>/tmp/bridge_integration_tests.err
if [ $? -eq 0 ]; then
    echo "✓ bridge_integration_tests.rs syntax is valid"
else
    echo "✗ bridge_integration_tests.rs has syntax errors:"
    cat /tmp/bridge_integration_tests.err
fi

echo "Validation complete."