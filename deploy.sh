#!/bin/bash

set -e  # Exit on error

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "=== Easy CI/CD Deployment Script ==="
echo ""

# Step 1: Build frontend
echo "[1/5] Building frontend..."
cd frontend-svelte
npm run build
cd ..
echo "✓ Frontend build complete"
echo ""

# Step 2: Build Docker image
echo "[2/5] Building Docker image..."
docker build -t choho97/lightweight-ci:latest -f agent/Dockerfile .
echo "✓ Docker build complete"
echo ""

# Step 3: Push to Docker Hub
echo "[3/5] Pushing image to Docker Hub..."
docker push choho97/lightweight-ci:latest
echo "✓ Push complete"
echo ""

# Step 4: Pull latest image
echo "[4/5] Pulling latest image..."
docker pull choho97/lightweight-ci:latest
echo "✓ Pull complete"
echo ""

# Step 5: Restart containers
echo "[5/5] Restarting containers..."
docker compose down
docker compose up -d
echo "✓ Containers restarted"
echo ""

echo "=== Deployment Complete ==="
echo "Checking container status..."
docker ps --filter "name=easycicd-agent"
