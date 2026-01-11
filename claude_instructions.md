# EasyCI/CD 프로젝트 컨텍스트

## ⚠️ 필수 규칙

**배포 시 반드시 `./deploy.sh` 사용!**
- 프론트엔드만 수정: `./deploy.sh` (빌드 + 도커 재시작 포함)
- 백엔드 수정: `./deploy.sh` (컴파일 + 이미지 빌드 + 배포)
- 절대 `docker restart`, `npm run build` 단독 사용 금지

## 아키텍처

1. DDD 레이어드 아키텍처: Application → Infrastructure → API
2. AppContext 기반 DI 컨테이너 (AppState 완전 대체됨)
3. Trait 기반 의존성 주입 (ProjectRepository, BuildRepository, SettingsRepository)
4. Blue-Green 배포 전략 (active_slot으로 활성 슬롯 결정)

## 코딩 규칙

1. 모든 API 핸들러: `State(ctx): State<AppContext>` + TraceContext + Timer 사용
2. Repository 메서드 추가 시: trait 먼저 정의 → 구현체 작성
3. 비즈니스 로직: 서비스 레이어에서 처리 (BuildService, DeploymentService, ProjectService)
4. 로깅: BoundaryLogger 통해 api_entry/api_exit 호출
5. 최대한 모듈화: 파일이 길면 분리, 의존성 서로 없게 유지
6. 상세한 로깅: 모든 모듈 간 통신을 로그로 추적 가능하게
7. **오류 발생 시**: `오답노트.txt` 업데이트 (원인, 해결방법 기록)
8. **문제 해결 시**: 먼저 `오답노트.txt`에서 유사 사례 검색 후 해결

## 주요 구성요소

1. **Services**: BuildService, DeploymentService, ProjectService
2. **Repositories**: SqliteProjectRepository, SqliteBuildRepository, SqliteSettingsRepository
3. **Infrastructure**: BroadcastEventBus, BuildQueue, WsConnections, DockerClient
4. **API**: 31개 핸들러 (projects 8개, builds 5개, settings 3개, webhook 1개, github_api 7개, ws 1개)

## 레거시

1. `build/executor.rs` - DEPRECATED (BuildService로 대체)
2. `build/deployer.rs` - DEPRECATED (DeploymentService로 대체)

## 빌드/배포

1. 앱 빌드/배포: `deploy.sh` 스크립트 사용
2. Docker 컨테이너 기반 빌드 및 런타임 실행
3. **마이그레이션 규칙**:
   - `sqlx::migrate!()` 매크로는 컴파일 타임에 마이그레이션을 바이너리에 임베드
   - 마이그레이션 파일 수정/삭제 시 반드시 재빌드 필요
   - 마이그레이션 파일은 절대 수정 금지 (새 파일로 추가만 가능)
   - SQLite는 `ALTER TABLE ADD COLUMN IF NOT EXISTS` 미지원
   - 마이그레이션 실패 시 DB 초기화 필요: `docker-compose down && sudo rm -rf /data/easycicd/db.sqlite* && docker-compose up -d`

## 참고 문서

1. `REPOSITORY_API_명세서.md` - Repository API 전체 명세
2. `ARCHITECTURE.md` - 시스템 아키텍처 상세
3. `API_통신_명세서.md` - API 통신 명세
4. `오답노트.txt` - 오류 및 해결방법 기록

## 중요 패턴

1. 컨테이너 ID 업데이트: `update_blue_container()` / `update_green_container()` 분리 사용
2. Trace ID 전파: `TraceContext::extract_or_generate(&headers)`
3. 타이머 측정: `Timer::start()` → `timer.elapsed_ms()`
4. 프로젝트 조회: 이름으로 조회 시 `project_repo.get_by_name()` 사용

## 설계 원칙 (중요!)

### 1. 모듈화/인터페이스화 지향
- **큰 덩어리 단위로 명확하게 모듈 분리**
- 각 모듈은 독립적이고 책임이 명확해야 함
- 모듈 간 의존성은 인터페이스(trait)로 추상화
- 목적: 코드 추적과 디버깅을 용이하게

### 2. 통합 로깅 중심 설계 (핵심!)
- **모든 모듈 간 경계에서 상세한 로그 출력 필수**
- 통합 로그 하나만 보면 전체 실행 흐름을 파악할 수 있어야 함
- 각 모듈 진입/종료 시점, 주요 데이터 변환 시점에 로그 출력
- 로그 레벨: ERROR → WARN → INFO → DEBUG 순으로 활용
- 로그 포맷 예시:
  ```
  [PARSE] Detected port from Dockerfile EXPOSE: 8080
  [PARSE] Merged env from GitHub secrets: 3 variables
  [BUILD] Starting Docker build for project: my-app (trace_id: abc123)
  [DEPLOY] Blue-Green switch: blue → green
  ```

### 3. 단일 문서 원칙
- **Claude가 디버깅 시 로그 파일 하나만 보면 문제 해결 가능하도록**
- 여러 모듈에 코드가 분산되어 있어도, 로그는 시간 순서대로 통합
- 각 모듈/파일을 일일이 열어보지 않아도 로그만으로 흐름 추적 가능
- 로그가 곧 실행 가능한 문서(Living Documentation)

## 로깅 시스템 분석

### 로그 파일 위치 및 역할

#### 1. 애플리케이션 통합 로그 (메인 로그)
- **위치**: `docker logs easycicd-agent`
- **역할**: 전체 시스템의 통합 실시간 로그 (stdout/stderr)
- **내용**:
  - 모든 API 요청/응답 (trace_id 포함)
  - 서비스 레이어 호출 흐름
  - Repository, Docker, GitHub 등 외부 시스템 통신
  - 이벤트 발행/구독
  - 에러 및 경고
- **포맷**: `[timestamp] [LEVEL] [module] [trace_id] [flow] message`
- **확인 방법**:
  - 실시간: `docker logs -f easycicd-agent`
  - 최근 100줄: `docker logs --tail 100 easycicd-agent`

#### 2. 빌드별 개별 로그
- **위치**: `/data/easycicd/logs/{project_id}/{build_id}.log`
- **역할**: 각 빌드의 Docker 빌드 과정 상세 로그
- **내용**:
  - Dockerfile 실행 단계별 출력
  - 의존성 설치 로그
  - 컴파일/빌드 에러
  - 이미지 레이어 생성 과정
- **예시**: `/data/easycicd/logs/6/4.log` = 프로젝트 6의 빌드 4번 로그

#### 3. 배포 로그
- **위치**: `/data/easycicd/logs/{project_id}/{build_id}_deploy.log`
- **역할**: Blue-Green 배포 과정 상세 로그
- **내용**:
  - 컨테이너 시작 로그
  - Health check 결과
  - 라우팅 전환 과정
  - 이전 컨테이너 정리
- **예시**: `/data/easycicd/logs/6/4_deploy.log` = 프로젝트 6의 빌드 4번 배포 로그

### 로깅 인프라 구성요소

#### BoundaryLogger (핵심 로깅 도구)
- **파일**: `agent/src/infrastructure/logging/boundary_logger.rs`
- **역할**: 모든 모듈 경계에서 구조화된 로그 생성
- **주요 메서드**:
  - `api_entry()` / `api_exit()` / `api_error()`: API 핸들러 진입/종료
  - `service_entry()` / `service_exit()` / `service_error()`: 서비스 레이어
  - `repo_call()` / `repo_done()` / `repo_error()`: Repository 호출
  - `external_call()` / `external_done()` / `external_error()`: Docker, GitHub 등
  - `event_emit()`: 이벤트 발행
- **로그 포맷**: `[{trace_id}] [{from}→{to}] {method} [{stage}] {details}`
  - 예: `[abc-123] [API→BuildService] execute_build [ENTRY] params={build_id: 42}`

#### TraceContext (요청 추적)
- **파일**: `agent/src/infrastructure/logging/trace_context.rs`
- **역할**: HTTP 요청마다 고유 trace_id 생성 및 전파
- **사용법**:
  - `TraceContext::extract_or_generate(&headers)`: 헤더에서 추출 또는 생성
  - `TraceContext::new_trace_id()`: 새 UUID 생성
- **목적**: 단일 요청의 전체 생명주기를 trace_id로 추적

#### Timer (성능 측정)
- **파일**: `agent/src/infrastructure/logging/boundary_logger.rs`
- **역할**: 각 작업의 소요 시간 측정
- **사용법**:
  ```rust
  let timer = Timer::start();
  // ... 작업 수행 ...
  logger.api_exit(trace_id, method, path, timer.elapsed_ms(), status);
  ```

### Claude 디버깅 워크플로우

1. **문제 발생 시 첫 번째 확인**: `docker logs --tail 200 easycicd-agent`
   - trace_id로 특정 요청의 전체 흐름 추적
   - `[←FAIL]` 스테이지 검색하여 에러 위치 파악
   - 모듈 간 호출 순서 및 소요 시간 확인

2. **빌드 실패 시**: `/data/easycicd/logs/{project_id}/{build_id}.log`
   - Docker 빌드 과정의 상세 에러 메시지 확인
   - 어느 단계에서 실패했는지 파악

3. **배포 실패 시**: `/data/easycicd/logs/{project_id}/{build_id}_deploy.log`
   - 컨테이너 시작 에러
   - Health check 실패 원인
   - 포트 충돌 등 배포 관련 이슈

4. **로그 검색 팁**:
   - Trace ID로 검색: `docker logs easycicd-agent 2>&1 | grep "abc-123"`
   - 에러만 보기: `docker logs easycicd-agent 2>&1 | grep ERROR`
   - 특정 모듈: `docker logs easycicd-agent 2>&1 | grep "BuildService"`
