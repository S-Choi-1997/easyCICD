# EasyCI/CD 프로젝트 컨텍스트

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
7. 오류 발생 시: `오답노트.txt` 업데이트 (원인, 해결방법 기록)

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
