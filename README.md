# Easy CI/CD

GitHub Actions 워크플로우 자동 감지 및 Blue-Green 배포를 지원하는 경량 CI/CD 시스템

## 주요 기능

### 🚀 자동 프로젝트 감지
- **GitHub Actions 워크플로우 자동 파싱**: `.github/workflows/` 디렉토리의 YAML 파일을 자동으로 분석
- **3계층 파싱 시스템**:
  - **Parser**: 워크플로우 YAML을 구조화된 데이터로 변환
  - **Interpreter**: 워크플로우 의도를 분석하여 프로젝트 타입 추론
  - **ConfigBuilder**: 실행 가능한 빌드/배포 설정 생성
- **지원 프로젝트 타입**:
  - Node.js (Backend/Frontend)
  - Java (Spring Boot with Gradle/Maven)
  - Python (Django/Flask/FastAPI)
  - Rust (Cargo)
  - Go
  - Static Sites

### 🔄 Blue-Green 배포
- 무중단 배포 (Zero-downtime deployment)
- Blue/Green 슬롯 자동 전환
- 롤백 지원

### 🐳 Docker 기반 빌드/런타임
- Docker-out-of-Docker (DOOD) 아키텍처
- 격리된 빌드 환경
- 프로젝트별 캐시 관리 (npm, Maven, Gradle, Cargo, Go modules)

### 🌐 자동 라우팅 및 프록시
- Cloudflare 연동
- 프로젝트별 서브도메인 자동 설정
- 동적 포트 매핑

### 🎯 동적 런타임 포트 지원 (v1.1.0)
- **워크플로우에서 포트 자동 감지**: `localhost:3000`, `PORT=3000` 등의 패턴 자동 인식
- **프로젝트 타입별 기본 포트**:
  - Node.js Backend: `3000`
  - Node.js Frontend (nginx): `80`
  - Spring Boot: `8080`
  - Django/FastAPI: `8000`
  - Go/Rust: `8080`
- **수동 포트 설정**: UI에서 포트 번호 직접 지정 가능
- **포트 자동 바인딩**: 컨테이너 내부 포트와 호스트 포트 자동 매핑

## 시스템 아키텍처

```
┌─────────────────────────────────────────────────────────┐
│                    Cloudflare Proxy                      │
│              (*.albl.cloud → 172.19.0.1)                │
└────────────────────┬────────────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────────────┐
│                  Agent Container                         │
│  ┌─────────────────────────────────────────────────┐   │
│  │  Reverse Proxy (port 8080)                      │   │
│  │  - 프로젝트별 라우팅 (project1.albl.cloud)      │   │
│  │  - Blue/Green 슬롯 관리                         │   │
│  └─────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────┐   │
│  │  API Server (port 3000)                         │   │
│  │  - 프로젝트/빌드 관리                            │   │
│  │  - GitHub Actions 워크플로우 파싱               │   │
│  │  - WebSocket 실시간 로그                        │   │
│  └─────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────┐   │
│  │  Build Worker                                    │   │
│  │  - 빌드 큐 관리                                  │   │
│  │  - Docker 빌드 컨테이너 실행                    │   │
│  │  - 캐시 관리                                     │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────┬───────────────────────────────────┘
                      │ (Docker Socket)
┌─────────────────────▼───────────────────────────────────┐
│                    Docker Host                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ Build        │  │ Runtime      │  │ Runtime      │  │
│  │ Container    │  │ Blue Slot    │  │ Green Slot   │  │
│  │ (임시)       │  │ (project-N)  │  │ (project-N)  │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└─────────────────────────────────────────────────────────┘
```

## 워크플로우 파싱 시스템

### Parser (workflow_parser.rs)
순수한 YAML 파싱만 수행. 판단 로직 없음.

```rust
pub struct WorkflowInfo {
    pub name: String,
    pub setup_actions: Vec<SetupAction>,  // actions/setup-node@v4 등
    pub run_commands: Vec<RunCommand>,     // run: npm install 등
    pub triggers: Vec<String>,
}
```

### Interpreter (workflow_interpreter.rs)
워크플로우의 의도를 해석하여 실행 계획 생성.

```rust
pub struct ExecutionPlan {
    pub project_type: ProjectType,        // NodeJsBackend, JavaSpringBoot 등
    pub runtime: Runtime,                 // 언어, 버전, 환경변수
    pub tasks: Vec<Task>,                 // 의존성 설치, 빌드, 테스트 등
    pub detected_port: Option<u16>,       // 워크플로우에서 감지된 포트
}
```

**포트 감지 로직**:
- `localhost:3000` 패턴
- `0.0.0.0:8080` 패턴
- `PORT=3000` 환경변수
- `--port 8000` 플래그

### ConfigBuilder (config_builder.rs)
ExecutionPlan을 실제 실행 가능한 설정으로 변환.

```rust
pub struct ProjectConfig {
    pub build_image: String,          // "node:20"
    pub build_command: String,        // "npm ci && cp -r src /output/"
    pub runtime_image: String,        // "node:20-slim"
    pub runtime_command: String,      // "node src/index.js"
    pub runtime_port: u16,            // 3000 (감지됨 or 기본값)
    // ...
}
```

## 빌드 프로세스

1. **워크플로우 분석**: GitHub Actions YAML 파싱 및 프로젝트 타입 감지
2. **빌드 컨테이너 실행**:
   ```bash
   docker run --rm \
     -v /host/cache/npm:/root/.npm \
     -v /host/output:/output \
     node:20 sh -c "npm ci && cp -r src node_modules package*.json /output/"
   ```
3. **런타임 컨테이너 배포**:
   ```bash
   docker run -d \
     --name project-16-green \
     -v /host/output:/app:ro \
     -p 10005:3000 \
     -e PORT=3000 \
     node:20-slim sh -c "node src/index.js"
   ```
4. **프록시 업데이트**: Green 슬롯으로 트래픽 전환
5. **Blue 슬롯 정리**: 이전 버전 컨테이너 종료

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

## 데이터베이스 스키마

### projects 테이블
```sql
CREATE TABLE projects (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    repo TEXT NOT NULL,
    branch TEXT NOT NULL,

    -- 빌드 설정
    build_image TEXT NOT NULL,
    build_command TEXT NOT NULL,
    cache_type TEXT NOT NULL,
    working_directory TEXT,

    -- 런타임 설정
    runtime_image TEXT NOT NULL,
    runtime_command TEXT NOT NULL,
    runtime_port INTEGER NOT NULL DEFAULT 8080,  -- v1.1.0 추가
    health_check_url TEXT NOT NULL,

    -- Blue-Green 배포
    blue_port INTEGER NOT NULL,
    green_port INTEGER NOT NULL,
    active_slot TEXT NOT NULL,
    blue_container_id TEXT,
    green_container_id TEXT,

    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

## API 엔드포인트

### 프로젝트 관리
- `GET /api/projects` - 프로젝트 목록
- `POST /api/projects` - 프로젝트 등록
- `GET /api/projects/:id` - 프로젝트 상세
- `DELETE /api/projects/:id` - 프로젝트 삭제

### 빌드 관리
- `POST /api/projects/:id/builds` - 빌드 트리거
- `GET /api/builds/:id/logs` - 빌드 로그 (WebSocket)

### GitHub 연동
- `POST /api/settings/github-pat` - GitHub PAT 설정
- `GET /api/github/repositories` - 레포지토리 목록
- `GET /api/github/detect-project` - 프로젝트 자동 감지

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

### v1.1.0 (2026-01-09)
- ✨ 동적 runtime_port 지원
- 🔍 워크플로우에서 포트 자동 감지
- 🏗️ 워크플로우 파싱 시스템 3계층 모듈화
- 📦 마이그레이션 파일 통합 및 단순화
- 🐛 컨테이너 포트 바인딩 동적 설정

### v1.0.0 (2026-01-08)
- 🎉 초기 릴리스
- GitHub Actions 워크플로우 자동 감지
- Blue-Green 배포
- 프로젝트별 캐시 관리

## 라이선스

MIT License

## 기여

Pull Request 환영합니다!

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'feat: add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request
