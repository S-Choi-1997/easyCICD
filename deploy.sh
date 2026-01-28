#!/bin/bash

set -e

echo "🚀 EasyCI/CD 배포 스크립트"
echo "=========================="

# 1. 프론트엔드 빌드
echo ""
echo "📦 [1/4] 프론트엔드 빌드 중..."
cd frontend-svelte
if ! npm run build 2>&1 | tee /tmp/frontend-build.log | grep -v "vite-plugin-svelte"; then
    echo "❌ 프론트엔드 빌드 실패!"
    echo "로그: /tmp/frontend-build.log"
    exit 1
fi
# Check if build output exists
if [ ! -f "../frontend/index.html" ]; then
    echo "❌ 빌드 산출물이 없습니다! (frontend/index.html not found)"
    exit 1
fi
cd ..

# 2. Docker 이미지 빌드
echo ""
echo "🐳 [2/4] Docker 이미지 빌드 중..."
cd agent
# Force rebuild by touching source files to invalidate Docker cache
touch src/main.rs
docker build -t choho97/lightweight-ci:latest .
cd ..

# 3. Docker Hub에 푸시 (동기)
echo ""
echo "📤 [3/4] Docker Hub에 푸시 중..."
if ! docker push choho97/lightweight-ci:latest 2>&1 | tee /tmp/docker-push.log; then
    echo "⚠️ Docker Hub 푸시 실패 (로컬 배포는 계속)"
    echo "로그: /tmp/docker-push.log"
    # Continue anyway for local deployment
fi

# 4. 컨테이너 재시작
echo ""
echo "🔄 [4/4] 컨테이너 재시작 중..."
docker compose down
docker compose up -d

echo ""
echo "✅ 배포 완료!"
echo ""
echo "접속 정보:"
echo "- Web UI: http://localhost:10000"
echo "- Proxy:  http://localhost:9999"
echo ""
echo "로그 확인: docker logs -f easycicd-agent"
