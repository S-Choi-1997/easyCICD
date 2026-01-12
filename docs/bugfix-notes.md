# 버그 수정 노트

## 2026-01-11: 컨테이너 네이밍 불일치 문제

### 문제 상황
- 프록시 라우팅 시 502 Bad Gateway 발생
- 서브도메인/경로 기반 라우팅 모두 실패

### 근본 원인
**컨테이너 이름 생성 규칙 불일치**

1. **컨테이너 생성 시** (`agent/src/docker/client.rs:433`):
   ```rust
   let container_name = format!("container-{}", name);
   ```
   - 실제 생성: `container-test-nginx`, `container-httpbin-test`

2. **프록시 라우팅 시** (`agent/src/proxy/router.rs:206`):
   ```rust
   let docker_container_name = format!("standalone-{}", container.id);
   ```
   - 잘못된 참조: `standalone-3`, `standalone-2`

### 결과
- Docker 네트워크 내에서 `standalone-3` 컨테이너를 찾을 수 없음
- "error sending request" 발생
- 모든 프록시 요청 실패 (502)

### 수정 방법
`agent/src/proxy/router.rs:204-207` 수정:
```rust
// 수정 전
let docker_container_name = container.container_id
    .as_ref()
    .map(|id| format!("standalone-{}", container.id))
    .unwrap_or_else(|| format!("standalone-{}", container.id));

// 수정 후
let docker_container_name = format!("container-{}", container.name);
```

### 교훈
**네이밍 규칙은 반드시 전역적으로 통일되어야 함**
- 리소스 생성 시점의 네이밍 규칙
- 리소스 참조 시점의 네이밍 규칙
- 두 곳이 정확히 일치해야 함

### 전체 코드베이스 검토 완료 ✅

다른 곳에서도 동일한 패턴이 있는지 확인:
- ✅ **컨테이너 이름 참조** - 모든 위치 정상
  - `docker/client.rs:433`: `container-{name}` 생성
  - `proxy/router.rs:204`: `container-{name}` 참조 (수정 완료)
  - `container_service.rs`: `container.container_id` (Docker ID) 사용 - 문제없음
  - `container_log_streamer.rs`: `container.container_id` (Docker ID) 사용 - 문제없음

- ✅ **프로젝트 컨테이너 이름** - 일관성 확인 완료
  - `docker/client.rs:334`: `project-{id}-{slot}` 생성
  - `proxy/router.rs:167-168`: `project-{id}-{slot}` 참조 - 정상

- ✅ **빌드 컨테이너 이름** - 일회성 UUID 사용
  - `docker/client.rs:156`: `build-{uuid}` - 일회성이므로 문제없음

- ✅ **프론트엔드** - 하드코딩된 컨테이너 이름 없음
  - CSS 클래스명과 UI 텍스트만 존재
  - 실제 컨테이너 이름 생성/참조 로직 없음

- ✅ **설정 파일** - 네이밍 관련 설정 없음
  - `.env`, `docker-compose.yml`, `.toml` 등에서 하드코딩 없음

### 네이밍 규칙 정리

현재 시스템의 컨테이너 네이밍 규칙:

| 리소스 유형 | 네이밍 형식 | 생성 위치 | 참조 위치 |
|------------|------------|----------|----------|
| 독립 컨테이너 | `container-{name}` | `docker/client.rs:433` | `proxy/router.rs:204` |
| 프로젝트 컨테이너 | `project-{id}-{slot}` | `docker/client.rs:334` | `proxy/router.rs:167-168` |
| 빌드 컨테이너 | `build-{uuid}` | `docker/client.rs:156` | N/A (일회성) |
| 데이터 디렉토리 | `/data/easycicd/containers/{name}/data` | `docker/client.rs:460` | N/A |

**모든 네이밍 규칙이 일관되게 사용되고 있음을 확인했습니다.** ✅

---

## 2026-01-11: WebSocket 반응성 문제 (인디케이터 업데이트 지연)

### 문제 상황
- 프로젝트 빌드/배포 인디케이터가 실시간으로 업데이트되지 않음
- 페이지 접기/펼치기 같은 액션을 해야 한 개씩 업데이트됨
- WebSocket으로 이벤트는 수신되지만 UI에 반영되지 않음

### 근본 원인
**WebSocket 이벤트 타입 불일치 및 비효율적인 상태 업데이트**

1. **이벤트 타입 불일치** (`frontend-svelte/src/stores/projects.js`, `builds.js`):
   ```javascript
   // ❌ 잘못된 코드
   if (data.type === 'BuildStatus')  // 대문자 CamelCase
   if (data.type === 'Log')          // 대문자 CamelCase

   // ✅ 실제 이벤트 타입 (백엔드에서 전송)
   // agent/src/events.rs:7 - #[serde(rename = "build_status")]
   // 실제: "build_status", "log", "deployment" (소문자 + 언더스코어)
   ```

2. **비효율적인 상태 업데이트**:
   ```javascript
   // ❌ 전체 프로젝트 목록 API 재호출
   export function updateProjectFromWebSocket(data) {
       if (data.type === 'BuildStatus') {
           loadProjects();  // 네트워크 지연, 느림
       }
   }
   ```

3. **반응성 트리거 실패**:
   - API 재호출로 인한 타이밍 이슈
   - Svelte의 반응성 시스템이 트리거되지 않음

### 결과
- 인디케이터가 실시간으로 업데이트되지 않음
- 접기/펼치기 등으로 컴포넌트가 다시 렌더링될 때만 업데이트됨
- WebSocket은 정상 작동하지만 UI가 반응하지 않음

### 수정 방법

#### 1. **projects.js 수정** (빌드/배포 상태 직접 업데이트)

```javascript
// ✅ 수정 후
export function updateProjectFromWebSocket(data) {
    if (data.type === 'build_status') {  // 소문자 + 언더스코어
        // API 재호출 없이 store 직접 업데이트
        projects.update(projectList => {
            return projectList.map(proj => {
                if (proj.id === data.project_id) {
                    return {
                        ...proj,
                        last_build_status: data.status,
                        last_build_at: data.timestamp
                    };
                }
                return proj;
            });
        });
    } else if (data.type === 'deployment') {
        // 배포 상태도 동일하게 직접 업데이트
        projects.update(projectList => {
            return projectList.map(proj => {
                if (proj.id === data.project_id) {
                    return {
                        ...proj,
                        active_slot: data.slot,
                        last_deployed_at: data.timestamp
                    };
                }
                return proj;
            });
        });
    }
}
```

#### 2. **builds.js 수정** (빌드 상태 직접 업데이트)

```javascript
// ✅ 수정 후
export function updateBuildFromWebSocket(data) {
    if (data.type === 'log') {  // 소문자
        appendLogLine(data.line);
    } else if (data.type === 'build_status') {  // 소문자 + 언더스코어
        const { project_id, build_id, status } = data;

        // API 재호출 없이 직접 업데이트
        builds.update(allBuilds => {
            if (allBuilds[project_id]) {
                return {
                    ...allBuilds,
                    [project_id]: allBuilds[project_id].map(build =>
                        build.id === build_id
                            ? { ...build, status, updated_at: data.timestamp }
                            : build
                    )
                };
            }
            return allBuilds;
        });

        // loadBuilds(project_id) 삭제 - API 재호출 제거
    }
}
```

### 교훈

**1. 백엔드-프론트엔드 이벤트 타입 일치 필수**
- 백엔드: `#[serde(rename = "build_status")]` → `"build_status"`
- 프론트엔드: `data.type === 'build_status'` (정확히 일치해야 함)

**2. Svelte 반응성은 새 객체/배열 생성으로 트리거**
```javascript
// ✅ 올바른 패턴
store.update(items => items.map(item =>
    item.id === targetId ? { ...item, status: newStatus } : item
))

// ❌ 잘못된 패턴
loadItems()  // API 재호출은 느리고 타이밍 이슈 발생
```

**3. WebSocket 실시간 업데이트는 store 직접 조작**
- API 재호출 ❌ (느림, 비효율적, 타이밍 이슈)
- Store 직접 update ✅ (즉시, 효율적, 반응성 보장)

**4. 콘솔 로그로 디버깅 필수**
- WebSocket 이벤트 타입 확인
- Store 업데이트 확인
- 반응성 트리거 확인

### 영향 범위
- ✅ 프로젝트 빌드 상태 인디케이터 → 즉시 업데이트
- ✅ 배포 상태 인디케이터 → 즉시 업데이트
- ✅ 빌드 목록 상태 → 즉시 업데이트
- ✅ 모든 인디케이터가 실시간으로 동기화됨

---

## 2026-01-11: 빌드/배포 상태 분리

### 목적
빌드와 배포를 명확하게 구분하여 각 단계의 상태를 독립적으로 추적

### 변경 사항

#### 1. **백엔드: BuildStatus enum 수정**
`agent/src/db/models.rs`:
```rust
// 변경 전
pub enum BuildStatus {
    Queued,
    Building,
    Deploying,  // ← 제거
    Success,
    Failed,
}

// 변경 후
pub enum BuildStatus {
    Queued,
    Building,
    Success,
    Failed,
}

// 새로 추가
pub enum DeploymentStatus {
    NotDeployed,
    Deploying,
    Deployed,
    Failed,
}
```

#### 2. **백엔드: Project 구조체에 deployment_status 필드 추가**
```rust
pub struct Project {
    // ... 기존 필드들
    #[sqlx(try_from = "String")]
    pub deployment_status: DeploymentStatus,
    // ...
}
```

**중요**: `TryFrom<String>` trait 구현 필요:
```rust
impl TryFrom<String> for DeploymentStatus {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}
```

#### 3. **백엔드: 배포 시작 이벤트 변경**
`agent/src/build/deployer.rs`, `agent/src/application/services/deployment_service.rs`:
```rust
// 변경 전
self.state.emit_event(Event::BuildStatus {
    status: BuildStatus::Deploying,  // ← 잘못됨
    ...
});

// 변경 후
self.state.emit_event(Event::Deployment {
    status: "deploying".to_string(),
    slot: project.get_inactive_slot(),
    ...
});
```

#### 4. **프론트엔드: projects.js 수정**
`frontend-svelte/src/stores/projects.js`:
```javascript
else if (data.type === 'deployment') {
    projects.update(projectList => {
        return projectList.map(proj => {
            if (proj.id === data.project_id) {
                return {
                    ...proj,
                    deployment_status: data.status,  // ← 추가
                    active_slot: data.slot,
                    last_deployed_at: data.timestamp
                };
            }
            return proj;
        });
    });
}
```

#### 5. **프론트엔드: Dashboard UI 수정**
`frontend-svelte/src/routes/Dashboard.svelte`:
```svelte
<!-- 변경 전 -->
<span class="status-badge {isProjectRunning(project) ? 'running' : 'stopped'}">
  {isProjectRunning(project) ? '실행 중' : '중지'}
</span>

<!-- 변경 후 -->
<div class="status-badges">
  <span class="status-badge build-status {project.last_build_status?.toLowerCase() || 'unknown'}">
    🔨 {project.last_build_status || 'N/A'}
  </span>
  <span class="status-badge deploy-status {isProjectRunning(project) ? 'running' : 'stopped'}">
    🚀 {isProjectRunning(project) ? '운영 중' : '중지'}
  </span>
</div>
```

#### 6. **CSS 추가**
```css
.status-badges {
    display: flex;
    gap: 0.375rem;
    flex-wrap: wrap;
}

/* 빌드 상태 */
.status-badge.build-status.success {
    background: #dbeafe;
    color: #1e40af;
}

.status-badge.build-status.building,
.status-badge.build-status.queued {
    background: #fef3c7;
    color: #92400e;
}

.status-badge.build-status.failed {
    background: #fee2e2;
    color: #991b1b;
}

/* 배포 상태 */
.status-badge.deploy-status.running {
    background: #dcfce7;
    color: #166534;
}

.status-badge.deploy-status.stopped {
    background: #f3f4f6;
    color: #6b7280;
}
```

### DB 마이그레이션 방법

**주의**: DB 마이그레이션 파일이 생성되었지만, **아직 실행하지 않았습니다**.

#### 마이그레이션 파일
`agent/migrations/003_add_deployment_status.sql`:
```sql
ALTER TABLE projects ADD COLUMN deployment_status TEXT NOT NULL DEFAULT 'NotDeployed';

UPDATE projects
SET deployment_status = CASE
    WHEN active_slot IS NOT NULL THEN 'Deployed'
    ELSE 'NotDeployed'
END;
```

#### 마이그레이션 실행 방법

**옵션 1: Docker 컨테이너 내부에서 실행** (추천)
```bash
# 컨테이너 접속
docker exec -it easycicd-agent /bin/sh

# 마이그레이션 실행
sqlite3 /data/easycicd/easycicd.db < /app/migrations/003_add_deployment_status.sql

# 확인
sqlite3 /data/easycicd/easycicd.db "PRAGMA table_info(projects);"
```

**옵션 2: 호스트에서 직접 실행**
```bash
sqlite3 /data/easycicd/easycicd.db < agent/migrations/003_add_deployment_status.sql
```

**옵션 3: 자동 마이그레이션** (향후 구현 필요)
- 현재는 수동으로 마이그레이션 필요
- 나중에 sqlx migrate 또는 refinery 같은 도구 도입 고려

#### 마이그레이션 확인
```bash
# deployment_status 컬럼이 추가되었는지 확인
docker exec easycicd-agent sqlite3 /data/easycicd/easycicd.db \
  "SELECT id, name, deployment_status FROM projects;"
```

### 이점

1. **명확한 상태 구분**
   - 빌드 성공 ≠ 배포 완료
   - 각 단계의 진행 상황 명확히 추적

2. **재배포 가능**
   - 빌드 없이 배포만 다시 실행 가능
   - 빌드는 성공했지만 배포 실패한 경우 구분 가능

3. **확장 가능**
   - 나중에 테스트 단계 추가 가능
   - 각 단계별로 재시도 가능

### UI 표시

프로젝트 카드:
```
┌─────────────────────────────┐
│ my-app                      │
│ 🔨 Success  🚀 운영 중      │
│                             │
│ [빌드 시작] [재배포]        │
└─────────────────────────────┘
```

### 주의사항

- **DB 마이그레이션은 수동으로 실행해야 함**
- 기존 데이터 유지됨 (active_slot 기반으로 deployment_status 설정)
- 하위 호환성: BuildStatus::Deploying은 Success로 변환됨
