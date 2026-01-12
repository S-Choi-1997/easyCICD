#!/bin/bash

set -e

echo "🚀 EasyCI/CD 배포 스크립트"
echo "=========================="

# 1. 프론트엔드 빌드
echo ""
echo "📦 [1/4] 프론트엔드 빌드 중..."
cd frontend-svelte
npm run build 2>&1 | grep -v "vite-plugin-svelte" || true
cd ..

# 2. Docker 이미지 빌드
echo ""
echo "🐳 [2/4] Docker 이미지 빌드 중..."
cd agent
docker build -t choho97/lightweight-ci:latest .
cd ..

# 3. Docker Hub에 비동기 푸시 (백그라운드)
echo ""
echo "📤 [3/4] Docker Hub에 푸시 중 (백그라운드)..."
(docker push choho97/lightweight-ci:latest > /tmp/docker-push.log 2>&1 && echo "✅ Docker Hub 푸시 완료" || echo "❌ Docker Hub 푸시 실패") &

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
echo "💡 Docker Hub 푸시는 백그라운드에서 진행 중입니다."
echo "   상태 확인: tail -f /tmp/docker-push.log"
echo ""
echo "로그 확인: docker logs -f easycicd-agent"
