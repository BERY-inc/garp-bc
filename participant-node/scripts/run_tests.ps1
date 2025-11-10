# Privacy Engine Test Runner Script
# This script runs comprehensive tests for the privacy engine

param(
    [string]$TestType = "all",
    [switch]$Verbose,
    [switch]$NoBenchmarks,
    [switch]$Coverage,
    [string]$OutputDir = "test-results"
)

Write-Host "🔒 Privacy Engine Test Runner" -ForegroundColor Green
Write-Host "==============================" -ForegroundColor Green

# Create output directory
if (!(Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

# Set environment variables for testing
$env:RUST_LOG = "debug"
$env:RUST_BACKTRACE = "1"

function Run-UnitTests {
    Write-Host "🧪 Running Unit Tests..." -ForegroundColor Yellow
    
    $unitTestArgs = @("test", "--lib")
    if ($Verbose) {
        $unitTestArgs += "--verbose"
    }
    
    $unitTestResult = & cargo @unitTestArgs
    $unitTestExitCode = $LASTEXITCODE
    
    if ($unitTestExitCode -eq 0) {
        Write-Host "✅ Unit tests passed!" -ForegroundColor Green
    } else {
        Write-Host "❌ Unit tests failed!" -ForegroundColor Red
        return $false
    }
    
    return $true
}

function Run-IntegrationTests {
    Write-Host "🔗 Running Integration Tests..." -ForegroundColor Yellow
    
    $integrationTestArgs = @("test", "--test", "privacy_engine_tests", "--test", "integration_tests")
    if ($Verbose) {
        $integrationTestArgs += "--verbose"
    }
    
    $integrationTestResult = & cargo @integrationTestArgs
    $integrationTestExitCode = $LASTEXITCODE
    
    if ($integrationTestExitCode -eq 0) {
        Write-Host "✅ Integration tests passed!" -ForegroundColor Green
    } else {
        Write-Host "❌ Integration tests failed!" -ForegroundColor Red
        return $false
    }
    
    return $true
}

function Run-Benchmarks {
    if ($NoBenchmarks) {
        Write-Host "⏭️  Skipping benchmarks..." -ForegroundColor Yellow
        return $true
    }
    
    Write-Host "📊 Running Performance Benchmarks..." -ForegroundColor Yellow
    
    $benchmarkArgs = @("bench", "--bench", "privacy_engine_benchmarks")
    if ($Verbose) {
        $benchmarkArgs += "--verbose"
    }
    
    # Run benchmarks and save results
    $benchmarkResult = & cargo @benchmarkArgs
    $benchmarkExitCode = $LASTEXITCODE
    
    if ($benchmarkExitCode -eq 0) {
        Write-Host "✅ Benchmarks completed!" -ForegroundColor Green
        
        # Copy benchmark results if they exist
        if (Test-Path "target/criterion") {
            Copy-Item -Path "target/criterion" -Destination "$OutputDir/benchmarks" -Recurse -Force
            Write-Host "📈 Benchmark results saved to $OutputDir/benchmarks" -ForegroundColor Cyan
        }
    } else {
        Write-Host "❌ Benchmarks failed!" -ForegroundColor Red
        return $false
    }
    
    return $true
}

function Run-CodeCoverage {
    if (!$Coverage) {
        return $true
    }
    
    Write-Host "📋 Running Code Coverage Analysis..." -ForegroundColor Yellow
    
    # Check if cargo-tarpaulin is installed
    $tarpaulinCheck = & cargo tarpaulin --version 2>$null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "⚠️  cargo-tarpaulin not found. Installing..." -ForegroundColor Yellow
        & cargo install cargo-tarpaulin
        if ($LASTEXITCODE -ne 0) {
            Write-Host "❌ Failed to install cargo-tarpaulin!" -ForegroundColor Red
            return $false
        }
    }
    
    # Run coverage analysis
    $coverageArgs = @(
        "tarpaulin",
        "--out", "Html",
        "--output-dir", "$OutputDir/coverage",
        "--exclude-files", "tests/*",
        "--exclude-files", "benches/*"
    )
    
    if ($Verbose) {
        $coverageArgs += "--verbose"
    }
    
    $coverageResult = & cargo @coverageArgs
    $coverageExitCode = $LASTEXITCODE
    
    if ($coverageExitCode -eq 0) {
        Write-Host "✅ Code coverage analysis completed!" -ForegroundColor Green
        Write-Host "📊 Coverage report saved to $OutputDir/coverage/tarpaulin-report.html" -ForegroundColor Cyan
    } else {
        Write-Host "❌ Code coverage analysis failed!" -ForegroundColor Red
        return $false
    }
    
    return $true
}

function Run-SecurityAudit {
    Write-Host "🔍 Running Security Audit..." -ForegroundColor Yellow
    
    # Check if cargo-audit is installed
    $auditCheck = & cargo audit --version 2>$null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "⚠️  cargo-audit not found. Installing..." -ForegroundColor Yellow
        & cargo install cargo-audit
        if ($LASTEXITCODE -ne 0) {
            Write-Host "❌ Failed to install cargo-audit!" -ForegroundColor Red
            return $false
        }
    }
    
    # Run security audit
    $auditResult = & cargo audit --json | Out-File -FilePath "$OutputDir/security-audit.json"
    $auditExitCode = $LASTEXITCODE
    
    if ($auditExitCode -eq 0) {
        Write-Host "✅ Security audit completed!" -ForegroundColor Green
        Write-Host "🔒 Audit report saved to $OutputDir/security-audit.json" -ForegroundColor Cyan
    } else {
        Write-Host "⚠️  Security audit found issues. Check $OutputDir/security-audit.json" -ForegroundColor Yellow
    }
    
    return $true
}

function Generate-TestReport {
    Write-Host "📝 Generating Test Report..." -ForegroundColor Yellow
    
    $reportPath = "$OutputDir/test-report.md"
    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    
    $report = @"
# Privacy Engine Test Report

**Generated:** $timestamp
**Test Type:** $TestType
**Output Directory:** $OutputDir

## Test Results Summary

"@

    if ($unitTestsPassed) {
        $report += "- ✅ Unit Tests: PASSED`n"
    } else {
        $report += "- ❌ Unit Tests: FAILED`n"
    }
    
    if ($integrationTestsPassed) {
        $report += "- ✅ Integration Tests: PASSED`n"
    } else {
        $report += "- ❌ Integration Tests: FAILED`n"
    }
    
    if (!$NoBenchmarks -and $benchmarksPassed) {
        $report += "- ✅ Performance Benchmarks: COMPLETED`n"
    } elseif (!$NoBenchmarks) {
        $report += "- ❌ Performance Benchmarks: FAILED`n"
    }
    
    if ($Coverage -and $coveragePassed) {
        $report += "- ✅ Code Coverage: COMPLETED`n"
    } elseif ($Coverage) {
        $report += "- ❌ Code Coverage: FAILED`n"
    }
    
    $report += "- ✅ Security Audit: COMPLETED`n"
    
    $report += @"

## Files Generated

- Test Report: $reportPath
- Security Audit: $OutputDir/security-audit.json
"@

    if (!$NoBenchmarks) {
        $report += "- Benchmark Results: $OutputDir/benchmarks/`n"
    }
    
    if ($Coverage) {
        $report += "- Coverage Report: $OutputDir/coverage/tarpaulin-report.html`n"
    }
    
    $report += @"

## Next Steps

1. Review any failed tests and fix issues
2. Check benchmark results for performance regressions
3. Review security audit findings
4. Update documentation if needed

---
*Generated by Privacy Engine Test Runner*
"@

    $report | Out-File -FilePath $reportPath -Encoding UTF8
    Write-Host "📄 Test report saved to $reportPath" -ForegroundColor Cyan
}

# Main execution
Write-Host "Starting test execution..." -ForegroundColor Cyan

$allTestsPassed = $true
$unitTestsPassed = $false
$integrationTestsPassed = $false
$benchmarksPassed = $false
$coveragePassed = $false

# Run tests based on type
switch ($TestType.ToLower()) {
    "unit" {
        $unitTestsPassed = Run-UnitTests
        $allTestsPassed = $unitTestsPassed
    }
    "integration" {
        $integrationTestsPassed = Run-IntegrationTests
        $allTestsPassed = $integrationTestsPassed
    }
    "benchmarks" {
        $benchmarksPassed = Run-Benchmarks
        $allTestsPassed = $benchmarksPassed
    }
    "all" {
        $unitTestsPassed = Run-UnitTests
        $integrationTestsPassed = Run-IntegrationTests
        $benchmarksPassed = Run-Benchmarks
        $coveragePassed = Run-CodeCoverage
        
        $allTestsPassed = $unitTestsPassed -and $integrationTestsPassed -and $benchmarksPassed -and $coveragePassed
    }
    default {
        Write-Host "❌ Unknown test type: $TestType" -ForegroundColor Red
        Write-Host "Valid options: unit, integration, benchmarks, all" -ForegroundColor Yellow
        exit 1
    }
}

# Always run security audit
Run-SecurityAudit | Out-Null

# Generate test report
Generate-TestReport

# Final results
Write-Host "`n==============================" -ForegroundColor Green
if ($allTestsPassed) {
    Write-Host "🎉 All tests completed successfully!" -ForegroundColor Green
    exit 0
} else {
    Write-Host "💥 Some tests failed. Check the results above." -ForegroundColor Red
    exit 1
}