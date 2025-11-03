#!/usr/bin/env pwsh
# Development script for Rurushi

Write-Host "Starting Rurushi in development mode..." -ForegroundColor Cyan
Write-Host "This will start both the Rust backend and Next.js dev server" -ForegroundColor Yellow
Write-Host ""

# Start Next.js dev server in background
Write-Host "Starting Next.js dev server on http://localhost:3000..." -ForegroundColor Yellow
Start-Process pwsh -ArgumentList "-NoExit", "-Command", "cd webui; npm run dev"

# Wait a moment for dev server to start
Start-Sleep -Seconds 2

# Start Rust backend
Write-Host "Starting Rust backend on http://localhost:8080..." -ForegroundColor Yellow
Write-Host ""
Write-Host "Access points:" -ForegroundColor Cyan
Write-Host "  - WebUI:  http://localhost:3000" -ForegroundColor Green
Write-Host "  - API:    http://localhost:8080/api" -ForegroundColor Green
Write-Host "  - Stream: http://localhost:8080/stream/tv" -ForegroundColor Green
Write-Host ""

cargo run
