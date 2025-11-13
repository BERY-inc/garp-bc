Param(
  [int]$NodeCount = 4,
  [int]$StartApiPort = 8000,
  [int]$StartConsensusPort = 7000,
  [string]$DatabaseUrl = "postgresql://garp:garp@localhost:5432/global_sync",
  [string]$ConfigPath = "config/global-sync.toml",
  [string]$LogLevel = "info"
)

Write-Host "Launching $NodeCount Global Synchronizer nodes..." -ForegroundColor Cyan

$procs = @()
for ($i = 0; $i -lt $NodeCount; $i++) {
  $nodeId = "global-sync-$($i+1)"
  $apiPort = $StartApiPort + $i
  $consensusPort = $StartConsensusPort + $i

  $args = @(
    "--config", $ConfigPath,
    "--node-id", $nodeId,
    "--peers", (1..$NodeCount | ForEach-Object { "localhost:$($StartConsensusPort + $_ - 1)" } | Join-String -Separator ","),
    "--database-url", $DatabaseUrl,
    "--port", $apiPort,
    "--consensus-port", $consensusPort,
    "--log-level", $LogLevel,
    "--enable-metrics"
  )

  Write-Host "Starting node $nodeId (API:$apiPort Consensus:$consensusPort)" -ForegroundColor Green
  $p = Start-Process -FilePath "cargo" -ArgumentList @("run", "-p", "global-synchronizer", "--") + $args -NoNewWindow -PassThru
  $procs += $p
}

Write-Host "All nodes launched. Press Ctrl+C to stop." -ForegroundColor Cyan

try {
  while ($true) { Start-Sleep -Seconds 2 }
}
finally {
  Write-Host "Stopping nodes..." -ForegroundColor Yellow
  foreach ($p in $procs) { try { Stop-Process -Id $p.Id -Force } catch {} }
}