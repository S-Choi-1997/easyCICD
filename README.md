# Easy CI/CD

간단한 Docker 기반 빌드/배포 자동화 시스템

## 주요 기능

### 🚀 프로젝트 관리
- GitHub 저장소 자동 감지 및 빌드 설정
- 프로젝트별 Docker 이미지 빌드
- 환경변수 및 포트 설정

### 🔄 Blue-Green 배포
- 무중단 배포 (Zero-downtime deployment)
- Blue/Green 슬롯 자동 전환
- 롤백 지원

### 🐳 Docker 기반
- Docker-out-of-Docker (DOOD) 아키텍처
- 프로젝트별 격리된 컨테이너 실행
- 빌드 및 배포 로그 실시간 추적

### 🎛️ 컨테이너 관리
- 실행 중인 컨테이너 모니터링
- 포트 자동 스캔 및 할당
- 컨테이너 생성/중지/삭제

## 시스템 아키텍처

### DDD 레이어드 아키텍처
```
API Layer (Handlers)
    ↓
Application Layer (Services)
    - BuildService: 빌드 실행 및 관리
    - DeploymentService: Blue-Green 배포
    - ProjectService: 프로젝트 CRUD
    - ContainerService: 컨테이너 관리
    ↓
Infrastructure Layer
    - Repositories (SQLite)
    - DockerClient
    - EventBus
    - Logging (BoundaryLogger, TraceContext)
```

### 컨테이너 구조
```
Agent Container (easycicd-agent)
  ├─ API Server (port 3000)
  ├─ Reverse Proxy (port 8080)
  ├─ Build Worker (백그라운드)
  └─ WebSocket (실시간 로그)
       ↓ (Docker Socket)
User Containers (Blue/Green slots)
```

## 빌드/배포 프로세스

1. **빌드 트리거**: GitHub webhook 또는 수동 빌드
2. **Docker 빌드**: Dockerfile 기반 이미지 빌드
3. **Blue-Green 배포**:
   - 비활성 슬롯에 새 컨테이너 시작
   - Health check 통과 시 슬롯 전환
   - 이전 컨테이너 정리
4. **프록시 업데이트**: 새 컨테이너로 라우팅

## 설치 및 실행

### 요구사항
- Docker 및 Docker Compose
- GitHub Personal Access Token (repo 권한)

### 배포
```bash
# 전체 빌드 및 배포
./deploy.sh

# 개별 실행
docker compose up -d
```

### 초기 설정
1. `http://your-domain:10000`에 접속
2. GitHub PAT 설정
3. 레포지토리 선택 및 자동 감지 실행
4. 프로젝트 등록

## 데이터베이스

### 주요 테이블
- `projects`: 프로젝트 정보 (repo, branch, ports, slots)
- `builds`: 빌드 이력 (status, logs)
- `settings`: 시스템 설정 (GitHub PAT)
- `containers`: 컨테이너 정보 (이름, 포트, 상태)

## API 엔드포인트

### 프로젝트
- `GET /api/projects`, `POST /api/projects`, `GET /api/projects/:id`, `DELETE /api/projects/:id`
- `POST /api/projects/:id/rollback/:build_id`: 이전 빌드로 롤백
- `GET /api/projects/:id/runtime-logs`: 런타임 로그 스트리밍 (WebSocket)

### 빌드
- `POST /api/projects/:id/builds`, `GET /api/builds/:id/logs` (WebSocket)

### 컨테이너
- `GET /api/containers`, `POST /api/containers`, `DELETE /api/containers/:id`

### GitHub
- `POST /api/settings/github-pat`, `GET /api/github/repositories`

### 설정
- `GET /api/settings`, `POST /api/settings`

## 기술 스택

### Backend (Rust)
- **axum**: 웹 프레임워크
- **bollard**: Docker API 클라이언트
- **sqlx**: SQLite ORM
- **tokio**: 비동기 런타임
- **serde_yaml**: YAML 파싱

### Frontend (Svelte)
- **Svelte**: UI 프레임워크
- **svelte-spa-router**: 클라이언트 라우팅
- **Vite**: 빌드 도구

### Infrastructure
- **Docker**: 컨테이너화
- **Cloudflare**: DNS 및 프록시
- **SQLite**: 데이터베이스

## 변경 이력

### v1.3.0 (2026-01-11)
- 🔄 롤백 기능 추가 (이전 빌드로 복원)
- 📜 런타임 로그 스트리밍 (실시간 컨테이너 로그)

### v1.2.0 (2026-01-11)
- 🏗️ DDD 레이어드 아키텍처 완성
- 📦 컨테이너 관리 기능 추가
- 🔍 통합 로깅 시스템 (BoundaryLogger, TraceContext)
- 🧩 AppContext 기반 DI 컨테이너
- 📝 설계 원칙 문서화 (모듈화, 통합 로깅, 단일 문서)

### v1.0.0 (2026-01-08)
- 🎉 초기 릴리스
- Blue-Green 배포
- GitHub 연동

## 라이선스

MIT License

## 기여

Pull Request 환영합니다!

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'feat: add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request
