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

### 1. 모듈화/인터페이스화 지양
- **각 기능을 과도하게 분리하지 말 것**
- 불필요한 추상화 레이어 생성 금지
- 작은 기능까지 trait으로 분리하면 코드 추적이 어려워짐
- 응집도 높은 코드를 선호 (관련 로직은 한 곳에)

### 2. 통합 로깅 중심 설계
- **모든 정보는 통합 로그에 집중**
- 로그 하나만 보면 전체 흐름을 파악할 수 있어야 함
- 각 단계마다 명확한 로그 메시지 출력
- 로그 레벨: ERROR → WARN → INFO → DEBUG 순으로 활용
- 예시:
  ```
  [PARSE] Detected port from Dockerfile EXPOSE: 8080
  [PARSE] Merged env from GitHub secrets: 3 variables
  [BUILD] Starting Docker build for project: my-app
  [DEPLOY] Blue-Green switch: blue → green
  ```

### 3. 단일 문서 원칙
- **한 문서만 보면 시스템 이해 가능하도록**
- 로그 파일이 곧 실행 가능한 문서
- 코드를 여러 파일에 나누더라도, 로그는 시간 순서대로 통합
- 디버깅 시 로그만으로 문제 해결 가능해야 함
