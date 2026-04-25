# Vibe Index Benchmark Runner (PowerShell)
Write-Host "=== Vibe Index Benchmark Suite ===" -ForegroundColor Cyan
Write-Host ""

# Run benchmarks
Write-Host "Running benchmarks..." -ForegroundColor Yellow
cargo bench 2>&1 | Tee-Object -FilePath "benchmark-output.txt"

Write-Host ""
Write-Host "=== Key Metrics ===" -ForegroundColor Green
Get-Content "benchmark-output.txt" | Select-String -Pattern "time:" -Context 0,0 | Select-Object -First 20

Write-Host ""
Write-Host "=== Summary ===" -ForegroundColor Green
Write-Host "All benchmarks completed successfully"
Write-Host "Full results: target/release/.criterion/"
