#!/bin/bash
# DB 초기화 스크립트 (개발 전용)

# -y 옵션으로 자동 확인
if [[ "$1" != "-y" ]]; then
    echo "⚠️  WARNING: This will delete all data!"
    read -p "Continue? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]
    then
        exit 1
    fi
fi

echo "Stopping containers..."
docker compose down

echo "Removing database..."
sudo rm -f /data/easycicd/db.sqlite*

echo "Starting containers..."
docker compose up -d

echo "✅ Database reset complete"
echo "Waiting for migrations to run..."
sleep 3
docker logs easycicd-agent 2>&1 | grep -i migration | tail -5
