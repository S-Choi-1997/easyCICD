# Repository API 명세서 (Phase 5-6 완료)

**버전**: 1.0
**최종 수정**: 2026-01-10
**목적**: DDD 아키텍처 기반 Repository Pattern API 완전 정의

---

## 목차

1. [개요](#개요)
2. [ProjectRepository API](#projectrepository-api)
3. [BuildRepository API](#buildrepository-api)
4. [SettingsRepository API](#settingsrepository-api)
5. [호출 위치 매핑](#호출-위치-매핑)
6. [AppContext 구조](#appcontext-구조)
7. [레거시 파일 정보](#레거시-파일-정보)

---

## 개요

Phase 5-6에서 완료된 DDD 아키텍처 기반 Repository Pattern의 모든 API를 정의합니다.

### 아키텍처 레이어

```
API Layer (api/)
    ↓ 의존
AppContext (DI Container)
    ↓ 주입
Application Layer (services/)
    ↓ 사용
Ports (repository traits)
    ↓ 구현
Infrastructure Layer (database/)
```

### 주요 특징

- **Trait 기반**: 모든 Repository는 Trait으로 정의되어 Mock 테스트 가능
- **의존성 주입**: AppContext를 통한 완전한 DI
- **타입 안전성**: Rust의 타입 시스템을 활용한 컴파일 타임 검증
- **async/await**: 모든 메서드가 비동기 처리

---

## ProjectRepository API

### Trait 정의

```rust
#[async_trait]
pub trait ProjectRepository: Send + Sync {
    async fn create(&self, project: CreateProject) -> Result<Project>;
    async fn get(&self, id: i64) -> Result<Option<Project>>;
    async fn get_by_name(&self, name: &str) -> Result<Option<Project>>;
    async fn list(&self) -> Result<Vec<Project>>;
    async fn update_active_slot(&self, id: i64, slot: Slot) -> Result<()>;
    async fn update_blue_container(&self, id: i64, container_id: Option<String>) -> Result<()>;
    async fn update_green_container(&self, id: i64, container_id: Option<String>) -> Result<()>;
    async fn delete(&self, id: i64) -> Result<()>;
}
```

### 메서드 상세

#### `create(project: CreateProject) -> Result<Project>`

새 프로젝트를 생성하고 Blue/Green 포트를 자동 할당합니다.

**입력**:
```rust
pub struct CreateProject {
    pub name: String,
    pub repo: String,
    pub path_filter: Option<String>,
    pub branch: String,
    pub build_image: String,
    pub build_command: String,
    pub cache_type: String,
    pub working_directory: Option<String>,
    pub runtime_image: String,
    pub runtime_command: String,
    pub health_check_url: String,
    pub runtime_port: i32,
}
```

**출력**: 생성된 `Project` (포트 자동 할당 완료)

**구현 로직**:
1. 현재 최대 포트 조회
2. 다음 포트 계산 (base_port, base_port + 1)
3. DB INSERT
4. 생성된 프로젝트 반환

**호출 위치**:
- `api/projects.rs:52` - create_project 핸들러

---

#### `get(id: i64) -> Result<Option<Project>>`

프로젝트 ID로 단건 조회.

**입력**: 프로젝트 ID (i64)
**출력**: `Option<Project>` (존재하지 않으면 None)

**SQL**:
```sql
SELECT * FROM projects WHERE id = ?
```

**호출 위치**:
- `api/projects.rs:70` - get_project 핸들러
- `api/projects.rs:93` - trigger_build 핸들러
- `api/projects.rs:113` - delete_project 핸들러
- `api/builds.rs:96` - get_build 핸들러 (project 정보 로딩)
- `build/worker.rs:60` - process_build (빌드 실행 전 프로젝트 로딩)

---

#### `get_by_name(name: &str) -> Result<Option<Project>>` ✅ NEW (Phase 5-6)

프로젝트 이름으로 조회. 프록시 라우팅에 사용.

**입력**: 프로젝트 이름 (String)
**출력**: `Option<Project>`

**SQL**:
```sql
SELECT * FROM projects WHERE name = ?
```

**호출 위치**:
- `proxy/router.rs:119` - handle_request (호스트 헤더 기반 라우팅)

**사용 예시**:
```rust
// 프록시가 "myapp.example.com" 요청을 받음
let project_name = extract_from_hostname(&host_header);
let project = ctx.project_repo.get_by_name(&project_name).await?;
let target_port = match project.active_slot {
    Slot::Blue => project.blue_port,
    Slot::Green => project.green_port,
};
```

---

#### `list() -> Result<Vec<Project>>`

모든 프로젝트 목록 조회.

**출력**: `Vec<Project>` (생성 시간 역순 정렬)

**SQL**:
```sql
SELECT * FROM projects ORDER BY created_at DESC
```

**호출 위치**:
- `api/projects.rs:28` - list_projects 핸들러
- `main.rs:169` - synchronize_container_states (시작 시 컨테이너 동기화)

---

#### `update_active_slot(id: i64, slot: Slot) -> Result<()>`

활성 슬롯(Blue/Green) 전환.

**입력**:
- `id`: 프로젝트 ID
- `slot`: `Slot::Blue` 또는 `Slot::Green`

**SQL**:
```sql
UPDATE projects SET active_slot = ? WHERE id = ?
```

**호출 위치**:
- `application/services/project_service.rs` - switch_slot 메서드 내부
- API에서는 ProjectService를 통해 간접 호출

---

#### `update_blue_container(id: i64, container_id: Option<String>) -> Result<()>`

Blue 슬롯의 컨테이너 ID 업데이트.

**입력**:
- `id`: 프로젝트 ID
- `container_id`: Docker 컨테이너 ID (None이면 빈 슬롯)

**SQL**:
```sql
UPDATE projects SET blue_container_id = ? WHERE id = ?
```

**호출 위치**:
- `main.rs:209` - synchronize_container_states (앱 시작 시)

---

#### `update_green_container(id: i64, container_id: Option<String>) -> Result<()>`

Green 슬롯의 컨테이너 ID 업데이트.

**입력**:
- `id`: 프로젝트 ID
- `container_id`: Docker 컨테이너 ID (None이면 빈 슬롯)

**SQL**:
```sql
UPDATE projects SET green_container_id = ? WHERE id = ?
```

**호출 위치**:
- `main.rs:212` - synchronize_container_states (앱 시작 시)

---

#### `delete(id: i64) -> Result<()>`

프로젝트 삭제 (Cascade로 빌드도 함께 삭제).

**입력**: 프로젝트 ID

**SQL**:
```sql
DELETE FROM projects WHERE id = ?
```

**호출 위치**:
- `api/projects.rs:131` - delete_project 핸들러

---

## BuildRepository API

### Trait 정의

```rust
#[async_trait]
pub trait BuildRepository: Send + Sync {
    async fn create(&self, build: CreateBuild) -> Result<Build>;
    async fn get(&self, id: i64) -> Result<Option<Build>>;
    async fn list(&self, limit: i64) -> Result<Vec<Build>>;
    async fn list_by_project(&self, project_id: i64, limit: i64) -> Result<Vec<Build>>;
    async fn list_recent(&self, limit: i64) -> Result<Vec<Build>>;
    async fn update_status(&self, id: i64, status: BuildStatus) -> Result<()>;
    async fn finish(&self, id: i64, status: BuildStatus) -> Result<()>;
    async fn update_deployed_slot(&self, id: i64, slot: Option<String>) -> Result<()>;
    async fn update_deploy_log_path(&self, id: i64, path: String) -> Result<()>;
}
```

### 메서드 상세

#### `create(build: CreateBuild) -> Result<Build>`

새 빌드 생성 및 build_number 자동 증가.

**입력**:
```rust
pub struct CreateBuild {
    pub project_id: i64,
    pub commit_hash: String,
    pub commit_message: String,
    pub author: String,
}
```

**출력**: 생성된 `Build` (status: "Queued")

**구현 로직**:
1. 프로젝트별 다음 build_number 계산
2. 로그 경로 생성: `/data/easycicd/logs/{project_id}/{build_number}.log`
3. DB INSERT
4. 생성된 빌드 반환

**호출 위치**:
- `api/projects.rs:192` - trigger_build 핸들러
- `api/webhook.rs:192` - github_webhook 핸들러

---

#### `get(id: i64) -> Result<Option<Build>>`

빌드 ID로 단건 조회.

**SQL**:
```sql
SELECT * FROM builds WHERE id = ?
```

**호출 위치**:
- `api/builds.rs:69` - get_build 핸들러
- `api/builds.rs:96` - get_build_logs 핸들러
- `api/builds.rs:132` - get_build_logs_only 핸들러
- `api/builds.rs:168` - get_deploy_logs 핸들러
- `build/worker.rs:66` - process_build (빌드 정보 로딩)

---

#### `list(limit: i64) -> Result<Vec<Build>>`

전체 빌드 목록 조회 (시작 시간 역순).

**SQL**:
```sql
SELECT * FROM builds ORDER BY started_at DESC LIMIT ?
```

**호출 위치**:
- 현재 미사용 (list_recent 또는 list_by_project 사용)

---

#### `list_by_project(project_id: i64, limit: i64) -> Result<Vec<Build>>`

특정 프로젝트의 빌드 목록 조회.

**SQL**:
```sql
SELECT * FROM builds WHERE project_id = ? ORDER BY started_at DESC LIMIT ?
```

**호출 위치**:
- `api/builds.rs:41` - list_builds 핸들러 (project_id 파라미터 있을 때)

---

#### `list_recent(limit: i64) -> Result<Vec<Build>>` ✅ NEW (Phase 5-6)

전체 프로젝트의 최근 빌드 목록 조회.

**SQL**:
```sql
SELECT * FROM builds ORDER BY started_at DESC LIMIT ?
```

**호출 위치**:
- `api/builds.rs:43` - list_builds 핸들러 (project_id 파라미터 없을 때)

---

#### `update_status(id: i64, status: BuildStatus) -> Result<()>`

빌드 상태 업데이트.

**입력**:
- `id`: 빌드 ID
- `status`: `Queued | Building | Deploying | Success | Failed`

**SQL**:
```sql
UPDATE builds SET status = ? WHERE id = ?
```

**호출 위치**:
- `application/services/build_service.rs` - BuildService 내부에서 호출
- API에서는 BuildService를 통해 간접 호출

---

#### `finish(id: i64, status: BuildStatus) -> Result<()>`

빌드 완료 처리 (상태 + finished_at + duration 업데이트).

**SQL**:
```sql
UPDATE builds
SET status = ?,
    finished_at = CURRENT_TIMESTAMP,
    duration_seconds = CAST((julianday(CURRENT_TIMESTAMP) - julianday(started_at)) * 86400 AS INTEGER)
WHERE id = ?
```

**호출 위치**:
- `application/services/build_service.rs` - BuildService 내부에서 호출

---

#### `update_deployed_slot(id: i64, slot: Option<String>) -> Result<()>`

배포된 슬롯 정보 업데이트.

**SQL**:
```sql
UPDATE builds SET deployed_slot = ? WHERE id = ?
```

**호출 위치**:
- `application/services/deployment_service.rs` - DeploymentService 내부에서 호출

---

#### `update_deploy_log_path(id: i64, path: String) -> Result<()>`

배포 로그 경로 업데이트.

**SQL**:
```sql
UPDATE builds SET deploy_log_path = ? WHERE id = ?
```

**호출 위치**:
- `application/services/deployment_service.rs` - DeploymentService 내부에서 호출

---

## SettingsRepository API

### Trait 정의

```rust
#[async_trait]
pub trait SettingsRepository: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<String>>;
    async fn set(&self, key: &str, value: &str) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
}
```

### 메서드 상세

#### `get(key: &str) -> Result<Option<String>>`

설정 값 조회.

**입력**: 설정 키 (예: "webhook_secret", "github_pat", "base_domain")
**출력**: `Option<String>` (없으면 None)

**SQL**:
```sql
SELECT value FROM settings WHERE key = ?
```

**호출 위치**:
- `api/settings.rs:26` - get_webhook_secret
- `api/settings.rs:122` - get_domain
- `api/webhook.rs:263` - verify_signature (webhook_secret 조회)
- `api/github_api.rs:79` - get_github_pat_status
- `api/github_api.rs:160` - list_repositories
- `api/github_api.rs:213` - list_branches
- `api/github_api.rs:267` - list_folders
- `api/github_api.rs:331` - detect_project

---

#### `set(key: &str, value: &str) -> Result<()>`

설정 값 저장 (UPSERT).

**SQL**:
```sql
INSERT INTO settings (key, value) VALUES (?, ?)
ON CONFLICT(key) DO UPDATE SET value = excluded.value
```

**호출 위치**:
- `api/settings.rs:92` - set_domain
- `api/github_api.rs:48` - set_github_pat

---

#### `delete(key: &str) -> Result<()>`

설정 삭제.

**SQL**:
```sql
DELETE FROM settings WHERE key = ?
```

**호출 위치**:
- `api/github_api.rs:129` - delete_github_pat

---

## 호출 위치 매핑

### ProjectRepository 호출 맵

| 메서드 | 호출 위치 | 목적 |
|--------|----------|------|
| `get()` | `projects.rs:70, 93, 113` | 프로젝트 조회 |
| `get()` | `builds.rs:96` | 빌드 상세에서 프로젝트 정보 |
| `get()` | `worker.rs:60` | 빌드 실행 전 프로젝트 로딩 |
| `get_by_name()` | `proxy/router.rs:119` | 프록시 라우팅 |
| `list()` | `projects.rs:28` | 프로젝트 목록 |
| `list()` | `main.rs:169` | 컨테이너 동기화 |
| `create()` | `projects.rs:52` | 프로젝트 생성 |
| `update_blue_container()` | `main.rs:209` | 컨테이너 동기화 |
| `update_green_container()` | `main.rs:212` | 컨테이너 동기화 |
| `delete()` | `projects.rs:131` | 프로젝트 삭제 |

### BuildRepository 호출 맵

| 메서드 | 호출 위치 | 목적 |
|--------|----------|------|
| `get()` | `builds.rs:69, 96, 132, 168` | 빌드 조회 |
| `get()` | `worker.rs:66` | 빌드 실행 |
| `list_recent()` | `builds.rs:43` | 전체 최근 빌드 |
| `list_by_project()` | `builds.rs:41` | 프로젝트별 빌드 |
| `create()` | `projects.rs:192` | 수동 빌드 트리거 |
| `create()` | `webhook.rs:192` | Webhook 빌드 트리거 |

### SettingsRepository 호출 맵

| 메서드 | 호출 위치 | 목적 |
|--------|----------|------|
| `get("webhook_secret")` | `webhook.rs:263` | Webhook 서명 검증 |
| `get("github_pat")` | `github_api.rs:79, 160, 213, 267, 331` | GitHub API 인증 |
| `get("base_domain")` | `settings.rs:122` | 도메인 조회 |
| `set("base_domain")` | `settings.rs:92` | 도메인 설정 |
| `set("github_pat")` | `github_api.rs:48` | PAT 저장 |
| `delete("github_pat")` | `github_api.rs:129` | PAT 삭제 |

---

## AppContext 구조

모든 Repository는 AppContext를 통해 주입됩니다.

```rust
#[derive(Clone)]
pub struct AppContext {
    // Services (Application Layer)
    pub project_service: Arc<ProjectService<...>>,
    pub build_service: Arc<BuildService<...>>,
    pub deployment_service: Arc<DeploymentService<...>>,

    // Repositories (Infrastructure Layer)
    pub project_repo: Arc<SqliteProjectRepository>,
    pub build_repo: Arc<SqliteBuildRepository>,
    pub settings_repo: Arc<SqliteSettingsRepository>,

    // Infrastructure
    pub event_bus: BroadcastEventBus,
    pub build_queue: Arc<BuildQueue>,
    pub ws_connections: Arc<WsConnections>,
    pub docker: DockerClient,
    pub logger: Arc<BoundaryLogger>,

    // Config
    pub gateway_ip: String,
    pub base_domain: Option<String>,
}
```

### AppContext 필드 접근 패턴

| 필드 | 파일 | 사용 위치 수 |
|------|------|-------------|
| `project_repo` | projects.rs | 6 |
| `project_repo` | builds.rs | 1 |
| `project_repo` | proxy/router.rs | 1 |
| `project_repo` | main.rs | 3 |
| `build_repo` | builds.rs | 6 |
| `build_repo` | projects.rs | 1 |
| `build_repo` | webhook.rs | 1 |
| `build_repo` | worker.rs | 1 |
| `settings_repo` | settings.rs | 3 |
| `settings_repo` | webhook.rs | 1 |
| `settings_repo` | github_api.rs | 8 |
| `build_service` | worker.rs | 1 |
| `deployment_service` | worker.rs | 1 |
| `event_bus` | webhook.rs | 1 |
| `event_bus` | ws_broadcaster.rs | 1 |
| `build_queue` | projects.rs | 1 |
| `build_queue` | webhook.rs | 1 |
| `build_queue` | worker.rs | 5 |
| `ws_connections` | ws.rs | 2 |
| `ws_connections` | ws_broadcaster.rs | 1 |
| `logger` | 모든 API 핸들러 | 62 (각 핸들러당 2회) |
| `gateway_ip` | proxy/router.rs | 2 |
| `base_domain` | proxy/router.rs | 1 |

---

## 레거시 파일 정보

### Deprecated 파일 (사용 안 함)

다음 파일들은 Phase 5-6에서 Services로 대체되었으며 더 이상 사용되지 않습니다:

#### 1. `agent/src/build/executor.rs`
- **상태**: DEPRECATED
- **대체**: `application/services/build_service.rs`
- **이유**: BuildService가 BuildExecutor의 모든 기능을 포함하며, Repository를 통한 데이터 접근으로 테스트 가능

#### 2. `agent/src/build/deployer.rs`
- **상태**: DEPRECATED
- **대체**: `application/services/deployment_service.rs`
- **이유**: DeploymentService가 Deployer의 모든 기능을 포함하며, 이벤트 버스 통합

#### 3. `agent/src/state.rs` (기존 AppState)
- **상태**: DEPRECATED
- **대체**: `agent/src/state/app_context.rs`
- **이유**: AppContext가 완전한 DI Container로 모든 의존성 관리

#### 4. `agent/src/db/queries.rs`
- **상태**: DEPRECATED (가능성)
- **대체**: Repository 구현체에 흡수
- **이유**: Repository Pattern으로 쿼리 로직 캡슐화

### 삭제 권장 파일

컴파일 확인 후 다음 파일들을 삭제할 수 있습니다:

```bash
# 백업 후 삭제
git mv agent/src/build/executor.rs agent/src/build/executor.rs.deprecated
git mv agent/src/build/deployer.rs agent/src/build/deployer.rs.deprecated

# 또는 완전 삭제
git rm agent/src/build/executor.rs
git rm agent/src/build/deployer.rs
```

---

## 변경 이력

### v1.0 (2026-01-10) - Phase 5-6 완료
- ✅ `ProjectRepository::get_by_name()` 추가
- ✅ `BuildRepository::list_recent()` 추가
- ✅ `update_container_id()` → `update_blue_container()` + `update_green_container()` 분리
- ✅ 모든 API 핸들러에 Trace ID + Timer 로깅 추가
- ✅ AppContext 기반 DI 완성
- ✅ 31개 API 핸들러 전환 완료

---

**문서 끝**

본 명세서는 Repository Pattern 기반 데이터 접근 계층의 모든 API를 정의합니다.
신규 개발 시 이 문서를 참조하여 Repository를 통한 데이터 접근을 구현하세요.
