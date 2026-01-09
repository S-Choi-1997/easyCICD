# Easy CI/CD 아키텍처 문서

## 시스템 개요

Easy CI/CD는 GitHub Actions 워크플로우를 자동으로 감지하고, Docker 기반 Blue-Green 배포를 수행하는 경량 CI/CD 시스템입니다.

## 핵심 아키텍처 원칙

### 1. Docker-out-of-Docker (DOOD)
Agent 컨테이너가 호스트의 Docker 소켓을 마운트하여 **형제(sibling) 컨테이너**를 관리합니다.

**장점**:
- 빌드 컨테이너가 Agent와 동일한 수준에서 실행 (네스팅 불필요)
- 호스트 네트워크 직접 접근 가능
- 볼륨 마운트 경로 일관성 (`/home/choho97/easycicd/data`)

**주의사항**:
- Agent 내부 경로(`/data`)와 호스트 경로 차이를 자동 변환
- `DockerClient::to_host_path()` 메서드로 경로 변환

### 2. Blue-Green 배포
각 프로젝트는 2개의 런타임 슬롯(Blue/Green)을 가집니다.

```
Project 16:
  Blue Slot:  project-16-blue  (port 10004)
  Green Slot: project-16-green (port 10005)
  Active: Green (현재 트래픽 수신 중)
```

**배포 프로세스**:
1. 빌드 실행 → `/data/output/buildN/` 생성
2. 비활성 슬롯(Blue)에 새 컨테이너 시작
3. 헬스체크 성공 시 프록시 업데이트 (Blue로 트래픽 전환)
4. 이전 활성 슬롯(Green) 컨테이너 종료

### 3. 프록시 라우팅
Cloudflare DNS → Agent 프록시 → 런타임 컨테이너

```
project1.albl.cloud
  ↓ (Cloudflare DNS)
172.19.0.1:8080 (Agent Reverse Proxy)
  ↓ (호스트 네트워크)
172.19.0.1:10005 (project-16-green 컨테이너)
  ↓ (포트 매핑: 10005→3000)
컨테이너 내부 3000번 포트 (Node.js 앱)
```

## 모듈 구조

### Agent 컨테이너 (Rust)

```
agent/
├── src/
│   ├── main.rs              # 애플리케이션 진입점
│   ├── state.rs             # 공유 상태 관리
│   ├── api/                 # REST API 엔드포인트
│   │   ├── projects.rs      # 프로젝트 CRUD
│   │   ├── builds.rs        # 빌드 관리
│   │   ├── github_api.rs    # GitHub 연동
│   │   └── ws.rs            # WebSocket (실시간 로그)
│   ├── build/               # 빌드 시스템
│   │   ├── worker.rs        # 빌드 큐 워커
│   │   ├── executor.rs      # Docker 빌드 실행
│   │   └── deployer.rs      # 런타임 배포
│   ├── docker/              # Docker 클라이언트
│   │   └── client.rs        # bollard 래퍼
│   ├── github/              # GitHub Actions 처리
│   │   ├── client.rs        # GitHub API 클라이언트
│   │   ├── workflow_parser.rs      # YAML 파싱
│   │   ├── workflow_interpreter.rs # 의도 해석
│   │   ├── config_builder.rs       # 설정 생성
│   │   └── detector.rs      # 프로젝트 감지 총괄
│   ├── proxy/               # 리버스 프록시
│   │   └── router.rs        # HTTP 라우팅
│   └── db/                  # 데이터베이스
│       ├── models.rs        # 데이터 모델
│       └── queries.rs       # SQL 쿼리
└── migrations/              # DB 마이그레이션
    ├── 001_initial.sql
    └── 002_settings.sql
```

## 워크플로우 파싱 시스템

### 3계층 아키텍처

```
GitHub Actions YAML
        ↓
┌─────────────────────┐
│  WorkflowParser     │  ← 순수 파싱 (판단 없음)
│  (workflow_parser)  │
└──────────┬──────────┘
           │ WorkflowInfo (구조화된 데이터)
           ↓
┌─────────────────────┐
│ WorkflowInterpreter │  ← 의도 해석
│ (interpreter)       │
└──────────┬──────────┘
           │ ExecutionPlan (실행 계획)
           ↓
┌─────────────────────┐
│  ConfigBuilder      │  ← 설정 생성
│  (config_builder)   │
└──────────┬──────────┘
           │ ProjectConfig (실행 가능한 설정)
           ↓
        Docker 실행
```

### 레이어별 역할

#### 1. WorkflowParser
**입력**: YAML 문자열
**출력**: `WorkflowInfo` (구조화된 데이터)

```rust
pub struct WorkflowInfo {
    pub name: String,                    // 워크플로우 이름
    pub setup_actions: Vec<SetupAction>, // actions/setup-node@v4 등
    pub run_commands: Vec<RunCommand>,   // run: npm install 등
    pub triggers: Vec<String>,           // push, pull_request 등
}
```

**책임**:
- YAML을 Rust 구조체로 역직렬화 (serde_yaml)
- 문법 검증
- **판단 로직 없음** (예: "이게 Node.js인지" 판단 안 함)

#### 2. WorkflowInterpreter
**입력**: `WorkflowInfo`
**출력**: `ExecutionPlan`

```rust
pub struct ExecutionPlan {
    pub project_type: ProjectType,    // NodeJsBackend, JavaSpringBoot 등
    pub runtime: Runtime,             // node:20, java:17 등
    pub tasks: Vec<Task>,             // Install, Build, Test 등
    pub detected_port: Option<u16>,   // 포트 감지 결과
}
```

**책임**:
- 워크플로우의 **의도** 분석
  - `actions/setup-node@v4` → Node.js 프로젝트
  - `npm run build` → 빌드 필요
  - `node src/index.js` → Backend (빌드 불필요)
- 프로젝트 타입 추론
- 런타임 포트 감지 (localhost:3000, PORT=8080 등)

**포트 감지 로직**:
```rust
fn detect_port(info: &WorkflowInfo) -> Option<u16> {
    // localhost:3000 패턴
    // 0.0.0.0:8080 패턴
    // PORT=3000 환경변수
    // --port 8000 플래그
}
```

#### 3. ConfigBuilder
**입력**: `ExecutionPlan`
**출력**: `ProjectConfig`

```rust
pub struct ProjectConfig {
    pub build_image: String,        // "node:20"
    pub build_command: String,      // "npm ci && cp ..."
    pub runtime_image: String,      // "node:20-slim"
    pub runtime_command: String,    // "node src/index.js"
    pub runtime_port: u16,          // 3000
    pub cache_type: String,         // "npm"
    pub health_check_url: String,   // "/health"
}
```

**책임**:
- Docker 실행 가능한 구체적인 명령 생성
- 이미지 선택 (빌드용 vs 런타임용)
- 포트 결정 (감지된 포트 or 기본값)
- 캐시 타입 결정

**기본 포트 규칙**:
```rust
fn determine_default_port(project_type: &ProjectType) -> u16 {
    match project_type {
        NodeJsBackend => 3000,
        NodeJsFrontend => 80,      // nginx
        JavaSpringBoot => 8080,
        PythonDjango => 8000,
        GolangApi => 8080,
        RustCargo => 8080,
        Unknown => 8080,
    }
}
```

## 빌드 시스템

### 빌드 워커 (build/worker.rs)
비동기 큐 기반 빌드 처리.

```rust
async fn build_worker(state: AppState) {
    loop {
        if let Some(build) = state.build_queue.pop().await {
            let executor = BuildExecutor::new(state.clone());
            executor.execute_build(build).await;
        }
    }
}
```

### 빌드 실행 단계 (build/executor.rs)

1. **빌드 컨테이너 실행**
```rust
docker run --rm \
  -v /host/workspace/buildN/repo:/workspace \
  -v /host/cache/npm:/root/.npm \
  -v /host/output/buildN:/output \
  node:20 \
  sh -c "npm ci && cp -r src node_modules package*.json /output/"
```

2. **로그 스트리밍**
```rust
let mut stream = docker.attach_container(container_id);
while let Some(chunk) = stream.next().await {
    // WebSocket으로 클라이언트에 전송
    broadcast_log(build_id, chunk);
}
```

3. **결과 확인**
- Exit code 0: 빌드 성공 → Deployer 호출
- Exit code != 0: 빌드 실패 → 상태 업데이트

### 배포 (build/deployer.rs)

1. **비활성 슬롯 결정**
```rust
let target_slot = match project.active_slot {
    Slot::Blue => Slot::Green,
    Slot::Green => Slot::Blue,
};
```

2. **런타임 컨테이너 시작**
```rust
docker.run_runtime_container(
    &project.runtime_image,      // "node:20-slim"
    &project.runtime_command,    // "node src/index.js"
    output_path,                 // "/data/output/buildN"
    target_port,                 // 10005 (호스트 포트)
    project.runtime_port,        // 3000 (컨테이너 내부 포트)
    project.id,                  // 16
    &target_slot.to_string(),    // "green"
).await?;
```

3. **헬스체크**
```rust
let health_url = format!(
    "http://{}:{}{}",
    gateway_ip,           // 172.19.0.1
    target_port,          // 10005
    health_check_url      // "/health"
);

for _ in 0..30 {
    if reqwest::get(&health_url).await?.status().is_success() {
        break; // 성공
    }
    tokio::time::sleep(Duration::from_secs(1)).await;
}
```

4. **슬롯 전환 및 정리**
```rust
// DB 업데이트: active_slot = Green
db.update_project_active_slot(project.id, target_slot).await?;

// 이전 Blue 컨테이너 종료
if let Some(old_id) = project.blue_container_id {
    docker.stop_container(&old_id).await?;
}
```

## 캐시 시스템

프로젝트별 의존성 캐시를 호스트에 저장하여 빌드 속도 향상.

```
/data/cache/
├── npm/           # Node.js (npm cache)
├── maven/         # Java Maven (~/.m2/repository)
├── gradle/        # Java Gradle (~/.gradle)
├── cargo/         # Rust (~/.cargo)
├── go/            # Go (GOPATH/pkg/mod)
└── pip/           # Python (pip cache)
```

**마운트 예시**:
```bash
# Node.js
-v /host/cache/npm:/root/.npm

# Maven
-v /host/cache/maven:/root/.m2/repository

# Gradle
-v /host/cache/gradle:/root/.gradle
```

## 네트워크 아키텍처

### Docker 네트워크: `easycicd_easycicd`
Bridge 네트워크로 모든 컨테이너 연결.

```
easycicd_easycicd (172.19.0.0/16)
├── Agent (172.19.0.2)
├── project-16-blue (172.19.0.3)
└── project-16-green (172.19.0.4)

Gateway: 172.19.0.1 (호스트)
```

### 포트 매핑 전략

```
사용자 요청
  ↓
project1.albl.cloud:443 (Cloudflare)
  ↓
your-server.com:8080 (Agent 프록시 포트)
  ↓
172.19.0.1:10005 (호스트 포트, Docker port binding)
  ↓
project-16-green:3000 (컨테이너 내부 포트)
  ↓
Node.js app listening on 0.0.0.0:3000
```

**포트 할당**:
- 각 프로젝트는 2개의 연속된 포트 할당
- Blue: `10000 + (project_id * 2)`
- Green: `10001 + (project_id * 2)`

예: Project ID 16
- Blue port: 10032
- Green port: 10033

## 데이터 흐름

### 1. 프로젝트 등록 플로우

```
사용자 (UI)
  ↓ POST /api/github/detect-project?repo=user/repo&branch=main
Agent API
  ↓
ProjectDetector::detect()
  ↓ GitHub API로 워크플로우 YAML 가져오기
WorkflowParser::parse()
  ↓ WorkflowInfo
WorkflowInterpreter::interpret()
  ↓ ExecutionPlan (포트 감지 포함)
ConfigBuilder::build()
  ↓ ProjectConfig
응답 (UI에 자동 감지 결과 표시)
```

### 2. 빌드 트리거 플로우

```
사용자 (UI)
  ↓ POST /api/projects/:id/builds
Agent API
  ↓
BuildQueue.push(build)
  ↓
BuildWorker (백그라운드)
  ↓
1. Git Clone (workspace/buildN/)
2. Docker Build Container 실행
3. 로그 WebSocket 스트리밍
4. Build 성공 시 Deployer 호출
  ↓
Deployer
  ↓
1. 비활성 슬롯에 런타임 컨테이너 시작
2. 헬스체크
3. 프록시 업데이트 (슬롯 전환)
4. 이전 컨테이너 종료
```

### 3. 프록시 라우팅 플로우

```
HTTP Request to project1.albl.cloud
  ↓
Cloudflare (DNS 해석)
  ↓
Agent Container (172.19.0.1:8080)
  ↓
proxy/router.rs
  ↓ 호스트 헤더로 프로젝트 조회
DB: SELECT * FROM projects WHERE name='project1'
  ↓ active_slot=Green, green_port=10005
hyper::Client
  ↓ HTTP 요청 프록시
http://172.19.0.1:10005/
  ↓ Docker port binding (10005→3000)
project-16-green 컨테이너 (Node.js app)
```

## 에러 처리 전략

### 빌드 실패
- Exit code 저장 (`builds.status = 'Failed'`)
- 로그 파일 보존 (`/data/logs/buildN.log`)
- WebSocket으로 실시간 에러 전송
- 기존 활성 슬롯 유지 (롤백 불필요)

### 배포 실패
- 헬스체크 실패 시 새 컨테이너 종료
- 기존 활성 슬롯 유지
- 빌드 상태 'Failed'로 업데이트
- 에러 메시지 로그에 기록

### 컨테이너 크래시
- Docker restart policy: `unless-stopped`
- 프록시는 컨테이너 상태 체크 없이 포트로 직접 전달
- 크래시 시 502 Bad Gateway 반환

## 보안 고려사항

### GitHub PAT
- SQLite settings 테이블에 평문 저장 (현재)
- **TODO**: 암호화 저장 권장

### Docker 소켓 접근
- Agent 컨테이너가 호스트 Docker 완전 제어 가능
- 프로덕션 환경에서는 별도 Docker 인증 고려

### 컨테이너 격리
- 빌드 컨테이너: `--rm` (일회성)
- 런타임 컨테이너: read-only 볼륨 마운트 (`/app:ro`)

## 성능 최적화

### 캐시 활용
- 의존성 캐시로 빌드 시간 단축 (npm, Maven, Cargo 등)
- 첫 빌드: ~2분 → 이후 빌드: ~30초

### 비동기 처리
- Tokio 비동기 런타임
- 빌드 큐 (여러 빌드 동시 처리 가능)
- WebSocket 스트리밍 (논블로킹)

### 리소스 제한
- **TODO**: Docker 메모리/CPU 제한 설정
- 현재: 무제한 (호스트 리소스 전체 사용 가능)

## 확장성

### 수평 확장 (Horizontal Scaling)
**현재 제약**:
- SQLite (파일 기반, 단일 인스턴스)
- 로컬 볼륨 (/data)

**확장 방안**:
1. PostgreSQL/MySQL로 DB 마이그레이션
2. 공유 스토리지 (NFS, S3) 사용
3. Redis 큐로 빌드 작업 분산

### 멀티 테넌시
- 현재: 단일 사용자/팀 가정
- **TODO**: 사용자/조직 개념 추가

## 모니터링 및 로깅

### 로그 레벨
- `tracing` crate 사용
- 환경변수 `RUST_LOG=info` 설정

### 로그 저장
- 빌드 로그: `/data/logs/buildN.log`
- Agent 로그: Docker stdout (docker logs로 확인)

### 메트릭
- **TODO**: Prometheus 메트릭 추가
  - 빌드 성공/실패율
  - 빌드 시간
  - 활성 컨테이너 수

## 향후 개선 사항

1. **멀티 레지스트리 지원**: Docker Hub 외 GitLab Registry, ECR 등
2. **빌드 캐시 최적화**: Docker layer 캐시 활용
3. **보안 강화**: Secret 관리, 컨테이너 스캔
4. **알림**: Slack/Discord 빌드 결과 알림
5. **롤백 UI**: 이전 빌드로 원클릭 롤백
6. **A/B 테스팅**: Blue-Green 동시 트래픽 분산

## 문제 해결 (Troubleshooting)

### 컨테이너가 재시작 반복
```bash
docker logs easycicd-agent
```
- DB 마이그레이션 에러 확인
- Docker 소켓 마운트 확인: `-v /var/run/docker.sock:/var/run/docker.sock`

### 프록시 502 에러
1. 런타임 컨테이너 상태 확인: `docker ps`
2. 헬스체크 URL 확인: `curl http://172.19.0.1:10005/health`
3. 컨테이너 로그 확인: `docker logs project-16-green`

### 빌드 실패
1. 빌드 로그 확인: UI 또는 `/data/logs/buildN.log`
2. 워크플로우 YAML 검증
3. 캐시 삭제 후 재시도: `rm -rf /data/cache/*`

### 포트 충돌
- 다른 프로젝트와 포트 충돌 시 `docker ps` 확인
- 수동으로 포트 할당 변경 (DB 직접 수정)

## 참고 자료

- [Docker API (bollard)](https://docs.rs/bollard/)
- [Axum Web Framework](https://docs.rs/axum/)
- [GitHub Actions Workflow Syntax](https://docs.github.com/en/actions/using-workflows/workflow-syntax-for-github-actions)
- [Blue-Green Deployment Pattern](https://martinfowler.com/bliki/BlueGreenDeployment.html)
