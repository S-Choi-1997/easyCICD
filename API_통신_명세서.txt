================================================================================
                    Lightweight CI/CD API 통신 명세서
================================================================================

버전: 1.0
최종 수정: 2025-01-07
문서 목적: 모든 HTTP, WebSocket, Docker, Database 통신 인터페이스 완전 정의

================================================================================
목차
================================================================================

1. HTTP REST API 엔드포인트
2. WebSocket 프로토콜
3. GitHub Webhook 인터페이스
4. Docker API 호출 명세
5. 데이터베이스 스키마
6. 내부 이벤트 시스템
7. 파일 시스템 구조
8. 에러 코드 정의

================================================================================
1. HTTP REST API 엔드포인트
================================================================================

1.1 베이스 URL
--------------
관리 API: https://ci.yourdomain.com
서비스 프록시: https://app.yourdomain.com


1.2 인증
--------
현재 버전: 인증 미구현 (내부 네트워크 전용)
향후 버전: Bearer Token 또는 Session Cookie


1.3 공통 응답 형식
------------------

성공 응답:
{
  "success": true,
  "data": { ... }
}

에러 응답:
{
  "success": false,
  "error": {
    "code": "PROJECT_NOT_FOUND",
    "message": "Project with id 42 not found",
    "details": { ... }
  }
}


1.4 프로젝트 관리 API
---------------------

1.4.1 프로젝트 목록 조회
------------------------
GET /api/projects

Query Parameters:
  - limit: number (기본값: 50)
  - offset: number (기본값: 0)
  - status: "active" | "inactive" | "all" (기본값: "all")

Request Headers:
  Accept: application/json

Response: 200 OK
{
  "success": true,
  "data": {
    "total": 5,
    "projects": [
      {
        "id": 1,
        "name": "my-backend",
        "repo": "username/my-spring-boot-app",
        "path_filter": "backend/**",
        "branch": "main",
        "build_image": "gradle:jdk17",
        "build_command": "./gradlew bootJar && cp build/libs/*.jar /output/app.jar",
        "cache_type": "gradle",
        "runtime_image": "eclipse-temurin:17-jre",
        "runtime_command": "java -jar /app/app.jar",
        "health_check_url": "/actuator/health",
        "blue_port": 9001,
        "green_port": 9002,
        "active_slot": "Blue",
        "blue_container_id": "abc123...",
        "green_container_id": null,
        "last_build_id": 42,
        "last_build_status": "Success",
        "last_build_at": "2025-01-07T12:34:56Z",
        "created_at": "2025-01-01T00:00:00Z",
        "updated_at": "2025-01-07T12:34:56Z"
      },
      ...
    ]
  }
}

에러 응답:
  - 400 Bad Request: 잘못된 파라미터
  - 500 Internal Server Error: 서버 오류


1.4.2 프로젝트 생성
-------------------
POST /api/projects

Request Headers:
  Content-Type: application/json
  Accept: application/json

Request Body:
{
  "name": "my-backend",
  "repo": "username/my-spring-boot-app",
  "path_filter": "backend/**",
  "branch": "main",
  "build_image": "gradle:jdk17",
  "build_command": "./gradlew bootJar && cp build/libs/*.jar /output/app.jar",
  "cache_type": "gradle",
  "runtime_image": "eclipse-temurin:17-jre",
  "runtime_command": "java -jar /app/app.jar",
  "health_check_url": "/actuator/health"
}

필수 필드:
  - name: 프로젝트 이름 (알파벳, 숫자, 하이픈만 허용)
  - repo: GitHub 레포지토리 (username/repo 형식)
  - branch: 타겟 브랜치
  - build_image: 빌드용 Docker 이미지
  - build_command: 빌드 명령어
  - runtime_image: 런타임 Docker 이미지
  - runtime_command: 실행 명령어

선택 필드:
  - path_filter: 경로 필터 (기본값: "**")
  - cache_type: 캐시 타입 (기본값: "none")
  - health_check_url: Health Check URL (기본값: "/")

Response: 201 Created
{
  "success": true,
  "data": {
    "id": 1,
    "name": "my-backend",
    "blue_port": 9001,   // 자동 할당
    "green_port": 9002,  // 자동 할당
    "active_slot": "Blue",
    "webhook_url": "https://ci.yourdomain.com/webhook/github?project=1",
    "public_url": "https://app.yourdomain.com/my-backend/",
    "created_at": "2025-01-07T12:00:00Z"
  }
}

에러 응답:
  - 400 Bad Request: 유효성 검증 실패
    {
      "success": false,
      "error": {
        "code": "VALIDATION_ERROR",
        "message": "Invalid project name",
        "details": {
          "field": "name",
          "reason": "Only alphanumeric and hyphens allowed"
        }
      }
    }

  - 409 Conflict: 프로젝트 이름 중복
    {
      "success": false,
      "error": {
        "code": "PROJECT_EXISTS",
        "message": "Project with name 'my-backend' already exists"
      }
    }

  - 500 Internal Server Error: 서버 오류


1.4.3 프로젝트 상세 조회
------------------------
GET /api/projects/{id}

Path Parameters:
  - id: 프로젝트 ID (정수)

Response: 200 OK
{
  "success": true,
  "data": {
    "id": 1,
    "name": "my-backend",
    "repo": "username/my-spring-boot-app",
    ... (프로젝트 전체 정보)
  }
}

에러 응답:
  - 404 Not Found: 프로젝트가 존재하지 않음


1.4.4 프로젝트 수정
-------------------
PUT /api/projects/{id}

Request Body:
{
  "branch": "develop",
  "build_command": "새 빌드 명령어",
  ... (수정할 필드만 포함)
}

Response: 200 OK
{
  "success": true,
  "data": {
    "id": 1,
    ... (업데이트된 프로젝트 정보)
  }
}

주의사항:
  - name, blue_port, green_port는 수정 불가
  - 변경 즉시 다음 빌드부터 적용


1.4.5 프로젝트 삭제
-------------------
DELETE /api/projects/{id}

Response: 200 OK
{
  "success": true,
  "data": {
    "message": "Project deleted successfully",
    "containers_stopped": 2,
    "builds_deleted": 15
  }
}

동작:
  1. 실행 중인 모든 컨테이너 중지 및 삭제
  2. 빌드 히스토리 삭제 (DB)
  3. 로그 파일 삭제
  4. workspace 및 output 디렉토리 삭제


1.5 빌드 관리 API
-----------------

1.5.1 빌드 히스토리 조회
------------------------
GET /api/projects/{project_id}/builds

Query Parameters:
  - limit: number (기본값: 20)
  - offset: number (기본값: 0)
  - status: "Queued" | "Building" | "Deploying" | "Success" | "Failed" | "all"

Response: 200 OK
{
  "success": true,
  "data": {
    "total": 42,
    "builds": [
      {
        "id": 42,
        "project_id": 1,
        "build_number": 42,
        "commit_hash": "abc123def456",
        "commit_message": "Add user API",
        "commit_author": "user@example.com",
        "status": "Success",
        "deployed_slot": "Green",
        "log_path": "/data/logs/project1/42.log",
        "started_at": "2025-01-07T12:34:56Z",
        "finished_at": "2025-01-07T12:37:30Z",
        "duration_seconds": 154
      },
      ...
    ]
  }
}


1.5.2 빌드 상세 조회
--------------------
GET /api/builds/{id}

Response: 200 OK
{
  "success": true,
  "data": {
    "id": 42,
    "project_id": 1,
    "project_name": "my-backend",
    "build_number": 42,
    "commit_hash": "abc123def456",
    "commit_message": "Add user API",
    "commit_author": "user@example.com",
    "status": "Success",
    "deployed_slot": "Green",
    "health_check_attempts": 3,
    "health_check_success": true,
    "log_path": "/data/logs/project1/42.log",
    "output_path": "/data/output/build42/",
    "started_at": "2025-01-07T12:34:56Z",
    "finished_at": "2025-01-07T12:37:30Z",
    "duration_seconds": 154,
    "stages": [
      {
        "name": "Git Pull",
        "status": "Success",
        "started_at": "2025-01-07T12:34:56Z",
        "finished_at": "2025-01-07T12:35:02Z"
      },
      {
        "name": "Build",
        "status": "Success",
        "started_at": "2025-01-07T12:35:02Z",
        "finished_at": "2025-01-07T12:37:10Z"
      },
      {
        "name": "Deploy",
        "status": "Success",
        "started_at": "2025-01-07T12:37:10Z",
        "finished_at": "2025-01-07T12:37:30Z"
      }
    ]
  }
}


1.5.3 수동 빌드 트리거
----------------------
POST /api/projects/{project_id}/builds

Request Body (선택적):
{
  "commit_hash": "abc123def456",  // 특정 커밋 지정 (없으면 latest)
  "force": true  // 진행 중인 빌드가 있어도 강제 실행
}

Response: 202 Accepted
{
  "success": true,
  "data": {
    "build_id": 43,
    "project_id": 1,
    "status": "Queued",
    "queue_position": 2,
    "estimated_start": "2025-01-07T12:40:00Z"
  }
}

에러 응답:
  - 409 Conflict: 이미 빌드가 진행 중
    {
      "success": false,
      "error": {
        "code": "BUILD_IN_PROGRESS",
        "message": "Build #42 is already in progress",
        "details": {
          "current_build_id": 42,
          "status": "Building"
        }
      }
    }


1.5.4 빌드 로그 스트리밍 (Server-Sent Events)
----------------------------------------------
GET /api/builds/{id}/logs

Request Headers:
  Accept: text/event-stream

Response: 200 OK
Content-Type: text/event-stream
Cache-Control: no-cache
Connection: keep-alive

data: {"timestamp": "2025-01-07T12:34:56Z", "line": "[INFO] Starting build..."}

data: {"timestamp": "2025-01-07T12:35:12Z", "line": "[INFO] Downloading dependencies..."}

data: {"timestamp": "2025-01-07T12:36:45Z", "line": "[INFO] Compiling..."}

data: {"timestamp": "2025-01-07T12:37:20Z", "line": "[INFO] Build successful"}

event: complete
data: {"status": "Success", "finished_at": "2025-01-07T12:37:20Z"}

동작:
  - 실시간 로그를 SSE로 스트리밍
  - 빌드가 완료되면 'complete' 이벤트 전송 후 연결 종료
  - 이미 완료된 빌드는 저장된 로그를 전체 전송 후 종료


1.5.5 빌드 로그 다운로드
------------------------
GET /api/builds/{id}/logs/download

Response: 200 OK
Content-Type: text/plain
Content-Disposition: attachment; filename="build-42.log"

[2025-01-07T12:34:56Z] [INFO] Starting build...
[2025-01-07T12:35:12Z] [INFO] Downloading dependencies...
...


1.6 배포 관리 API
-----------------

1.6.1 현재 배포 상태 조회
--------------------------
GET /api/projects/{id}/status

Response: 200 OK
{
  "success": true,
  "data": {
    "project_id": 1,
    "project_name": "my-backend",
    "active_slot": "Blue",
    "blue": {
      "container_id": "abc123...",
      "status": "running",
      "port": 9001,
      "build_id": 41,
      "build_number": 41,
      "commit_hash": "def456...",
      "started_at": "2025-01-06T10:00:00Z",
      "uptime_seconds": 95000,
      "health": "healthy",
      "last_health_check": "2025-01-07T12:30:00Z"
    },
    "green": {
      "container_id": null,
      "status": "empty",
      "port": 9002
    },
    "public_url": "https://app.yourdomain.com/my-backend/"
  }
}


1.6.2 Blue/Green 수동 전환
---------------------------
POST /api/projects/{id}/switch

Request Body:
{
  "target_slot": "Green"  // 전환할 슬롯
}

Response: 200 OK
{
  "success": true,
  "data": {
    "project_id": 1,
    "previous_slot": "Blue",
    "current_slot": "Green",
    "switched_at": "2025-01-07T12:40:00Z"
  }
}

에러 응답:
  - 400 Bad Request: 타겟 슬롯이 비어있음
    {
      "success": false,
      "error": {
        "code": "SLOT_EMPTY",
        "message": "Green slot has no running container"
      }
    }


1.6.3 컨테이너 재시작
----------------------
POST /api/projects/{id}/restart

Request Body:
{
  "slot": "Blue"  // "Blue" | "Green" | "both"
}

Response: 200 OK
{
  "success": true,
  "data": {
    "blue_restarted": true,
    "green_restarted": false,
    "timestamp": "2025-01-07T12:45:00Z"
  }
}


1.7 Web UI 페이지
-----------------

1.7.1 대시보드
--------------
GET /

Response: 200 OK
Content-Type: text/html

(HTML 페이지 반환)


1.7.2 Setup 페이지
------------------
GET /setup

Response: 200 OK
Content-Type: text/html

(HTML 페이지 반환)


1.7.3 빌드 상세 페이지
----------------------
GET /builds/{id}

Response: 200 OK
Content-Type: text/html

(HTML 페이지 반환)


================================================================================
2. WebSocket 프로토콜
================================================================================

2.1 연결 엔드포인트
-------------------
wss://ci.yourdomain.com/ws

연결 시 헤더:
  Upgrade: websocket
  Connection: Upgrade
  Sec-WebSocket-Version: 13
  Sec-WebSocket-Key: <random-key>


2.2 연결 수명주기
-----------------

1. 클라이언트 연결
   - WebSocket handshake
   - 서버가 연결 수락

2. 환영 메시지 (서버 → 클라이언트)
   {
     "type": "connected",
     "connection_id": "conn-12345",
     "timestamp": "2025-01-07T12:00:00Z"
   }

3. 구독 설정 (클라이언트 → 서버)
   - subscribe 메시지 전송

4. 이벤트 수신 (서버 → 클라이언트)
   - 구독한 이벤트 실시간 수신

5. 연결 종료
   - 클라이언트가 연결 끊기
   - 서버가 자동으로 구독 해제


2.3 클라이언트 → 서버 메시지
-----------------------------

2.3.1 빌드 구독
---------------
{
  "type": "subscribe",
  "target": "build",
  "build_id": 42
}

응답 (서버 → 클라이언트):
{
  "type": "subscribed",
  "target": "build",
  "build_id": 42,
  "current_status": "Building",
  "timestamp": "2025-01-07T12:00:00Z"
}


2.3.2 프로젝트 구독
-------------------
{
  "type": "subscribe",
  "target": "project",
  "project_id": 1
}

응답:
{
  "type": "subscribed",
  "target": "project",
  "project_id": 1,
  "timestamp": "2025-01-07T12:00:00Z"
}


2.3.3 전역 구독 (모든 이벤트)
-----------------------------
{
  "type": "subscribe",
  "target": "global"
}

응답:
{
  "type": "subscribed",
  "target": "global",
  "timestamp": "2025-01-07T12:00:00Z"
}


2.3.4 구독 해제
---------------
{
  "type": "unsubscribe",
  "target": "build",
  "build_id": 42
}

응답:
{
  "type": "unsubscribed",
  "target": "build",
  "build_id": 42,
  "timestamp": "2025-01-07T12:00:00Z"
}


2.3.5 Ping (연결 유지)
----------------------
{
  "type": "ping"
}

응답:
{
  "type": "pong",
  "timestamp": "2025-01-07T12:00:00Z"
}


2.4 서버 → 클라이언트 메시지
-----------------------------

2.4.1 빌드 상태 변경
--------------------
{
  "type": "build_status",
  "build_id": 42,
  "project_id": 1,
  "project_name": "my-backend",
  "status": "Building",
  "previous_status": "Queued",
  "timestamp": "2025-01-07T12:34:56Z"
}

가능한 상태:
  - Queued → Building
  - Building → Deploying
  - Deploying → Success
  - Deploying → Failed
  - Building → Failed


2.4.2 실시간 로그
-----------------
{
  "type": "log",
  "build_id": 42,
  "project_id": 1,
  "line": "[INFO] Starting build...",
  "timestamp": "2025-01-07T12:34:57Z",
  "level": "info"  // "info" | "warn" | "error"
}

로그 레벨 파싱:
  - [INFO], [DEBUG] → "info"
  - [WARN], [WARNING] → "warn"
  - [ERROR], [FATAL] → "error"


2.4.3 배포 완료
---------------
{
  "type": "deployment",
  "build_id": 42,
  "project_id": 1,
  "project_name": "my-backend",
  "status": "Success",
  "deployed_slot": "Green",
  "switched_to": "Green",
  "public_url": "https://app.yourdomain.com/my-backend/",
  "timestamp": "2025-01-07T12:37:30Z"
}


2.4.4 Health Check 진행 상황
-----------------------------
{
  "type": "health_check",
  "build_id": 42,
  "project_id": 1,
  "slot": "Green",
  "attempt": 3,
  "max_attempts": 10,
  "status": "Checking",  // "Checking" | "Success" | "Failed"
  "response_code": 200,
  "response_time_ms": 45,
  "timestamp": "2025-01-07T12:37:15Z"
}


2.4.5 컨테이너 상태 변경
------------------------
{
  "type": "container_status",
  "project_id": 1,
  "slot": "Blue",
  "container_id": "abc123...",
  "status": "running",  // "starting" | "running" | "stopped" | "error"
  "timestamp": "2025-01-07T12:00:00Z"
}


2.4.6 에러 메시지
-----------------
{
  "type": "error",
  "code": "BUILD_FAILED",
  "message": "Build failed: compilation error",
  "build_id": 42,
  "project_id": 1,
  "details": {
    "exit_code": 1,
    "stage": "Build"
  },
  "timestamp": "2025-01-07T12:36:00Z"
}


2.5 브로드캐스트 규칙
---------------------

구독 타입별 메시지 수신:

1. "build" 구독 (build_id: 42):
   - build_status (build_id = 42)
   - log (build_id = 42)
   - deployment (build_id = 42)
   - health_check (build_id = 42)
   - error (build_id = 42)

2. "project" 구독 (project_id: 1):
   - build_status (project_id = 1, 모든 빌드)
   - deployment (project_id = 1)
   - container_status (project_id = 1)

3. "global" 구독:
   - 모든 메시지 수신


2.6 에러 처리
-------------

잘못된 메시지 형식:
{
  "type": "error",
  "code": "INVALID_MESSAGE",
  "message": "Invalid message format",
  "timestamp": "2025-01-07T12:00:00Z"
}

존재하지 않는 리소스 구독:
{
  "type": "error",
  "code": "RESOURCE_NOT_FOUND",
  "message": "Build #999 not found",
  "timestamp": "2025-01-07T12:00:00Z"
}

연결 타임아웃 (5분 무활동):
{
  "type": "timeout",
  "message": "Connection timeout due to inactivity",
  "timestamp": "2025-01-07T12:05:00Z"
}


================================================================================
3. GitHub Webhook 인터페이스
================================================================================

3.1 Webhook 엔드포인트
----------------------
POST /webhook/github

Query Parameters:
  - project: number (선택적, 프로젝트 ID로 필터링)


3.2 GitHub 요청 헤더
--------------------
X-GitHub-Event: push | pull_request | release
X-Hub-Signature-256: sha256=<HMAC-SHA256 서명>
X-GitHub-Delivery: <UUID>
Content-Type: application/json
User-Agent: GitHub-Hookshot/<version>


3.3 서명 검증
-------------

알고리즘: HMAC-SHA256
Secret: 프로젝트별 설정 (환경 변수 또는 DB 저장)

검증 절차:
1. Request Body를 그대로 읽기 (파싱 전)
2. HMAC-SHA256(secret, body) 계산
3. "sha256=" + hex(hmac) 형식으로 변환
4. X-Hub-Signature-256 헤더와 비교
5. 일치하지 않으면 403 Forbidden 반환

코드 예시 (Rust):
```rust
use hmac::{Hmac, Mac};
use sha2::Sha256;

fn verify_signature(secret: &str, body: &[u8], signature: &str) -> bool {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(body);
    let result = mac.finalize();
    let expected = format!("sha256={}", hex::encode(result.into_bytes()));
    expected == signature
}
```


3.4 Push Event 페이로드
------------------------

{
  "ref": "refs/heads/main",
  "before": "abc123...",
  "after": "def456...",
  "repository": {
    "id": 123456,
    "name": "my-repo",
    "full_name": "username/my-repo",
    "owner": {
      "name": "username",
      "email": "user@example.com"
    },
    "clone_url": "https://github.com/username/my-repo.git",
    "default_branch": "main"
  },
  "pusher": {
    "name": "username",
    "email": "user@example.com"
  },
  "commits": [
    {
      "id": "def456...",
      "message": "Add user API",
      "timestamp": "2025-01-07T12:00:00Z",
      "author": {
        "name": "username",
        "email": "user@example.com"
      },
      "added": ["backend/api/user.java"],
      "removed": [],
      "modified": ["backend/pom.xml"]
    }
  ],
  "head_commit": {
    "id": "def456...",
    "message": "Add user API",
    "timestamp": "2025-01-07T12:00:00Z",
    "author": {
      "name": "username",
      "email": "user@example.com"
    }
  }
}


3.5 프로젝트 매칭 로직
----------------------

1. repository.full_name으로 프로젝트 조회
   - DB에서 repo = "username/my-repo" 검색

2. branch 필터링
   - ref가 "refs/heads/{project.branch}"와 일치하는지 확인

3. path_filter 검증
   - commits[].added, commits[].modified, commits[].removed의 모든 파일 수집
   - 각 파일이 path_filter 패턴과 일치하는지 확인
   - 예: path_filter = "backend/**"
     - "backend/api/user.java" → 매칭
     - "frontend/App.js" → 매칭 안 됨

4. 매칭 성공 시
   - 빌드 큐에 추가
   - 202 Accepted 응답


3.6 응답 형식
-------------

성공 (빌드 트리거):
HTTP/1.1 202 Accepted
{
  "success": true,
  "data": {
    "build_id": 43,
    "project_id": 1,
    "project_name": "my-backend",
    "commit_hash": "def456...",
    "status": "Queued",
    "queue_position": 1
  }
}

무시 (브랜치 불일치):
HTTP/1.1 200 OK
{
  "success": true,
  "message": "Ignored: branch mismatch",
  "details": {
    "ref": "refs/heads/develop",
    "expected": "refs/heads/main"
  }
}

무시 (경로 필터 불일치):
HTTP/1.1 200 OK
{
  "success": true,
  "message": "Ignored: no matching files",
  "details": {
    "path_filter": "backend/**",
    "files_changed": ["frontend/App.js"]
  }
}

에러 (서명 검증 실패):
HTTP/1.1 403 Forbidden
{
  "success": false,
  "error": {
    "code": "SIGNATURE_INVALID",
    "message": "Invalid signature"
  }
}

에러 (프로젝트 없음):
HTTP/1.1 404 Not Found
{
  "success": false,
  "error": {
    "code": "PROJECT_NOT_FOUND",
    "message": "No project found for repository 'username/my-repo'"
  }
}


================================================================================
4. Docker API 호출 명세
================================================================================

4.1 사용 라이브러리
-------------------
Rust: bollard (https://github.com/fussybeaver/bollard)
Docker API 버전: 1.41+


4.2 빌드 컨테이너 실행
----------------------

목적: 소스 코드를 빌드하여 결과물 생성

API 호출:
POST /containers/create

Request Body:
{
  "Image": "gradle:jdk17",
  "Cmd": [
    "/bin/bash",
    "-c",
    "cd /app && ./gradlew clean bootJar && cp build/libs/*.jar /output/app.jar"
  ],
  "HostConfig": {
    "Binds": [
      "/data/workspace/project1:/app:ro",
      "/data/cache/gradle:/root/.gradle",
      "/data/output/build42:/output"
    ],
    "AutoRemove": true,
    "NetworkMode": "bridge"
  },
  "WorkingDir": "/app",
  "AttachStdout": true,
  "AttachStderr": true,
  "Tty": false,
  "OpenStdin": false
}

응답:
{
  "Id": "container-id-12345...",
  "Warnings": []
}

볼륨 마운트 설명:
  - /data/workspace/project1:/app:ro
    → 소스 코드 (읽기 전용)

  - /data/cache/gradle:/root/.gradle
    → Gradle 캐시 (읽기/쓰기)
    → 모든 Gradle 프로젝트가 공유

  - /data/output/build42:/output
    → 빌드 결과물 출력 디렉토리 (읽기/쓰기)


4.3 빌드 컨테이너 시작 및 로그 수집
-----------------------------------

시작:
POST /containers/{id}/start

응답: 204 No Content


로그 스트리밍 (Attach):
POST /containers/{id}/attach?stream=true&stdout=true&stderr=true

응답: HTTP Upgrade to TCP stream

Stream 형식 (Docker Multiplexed Stream):
  [STREAM_TYPE(1 byte)][PADDING(3 bytes)][SIZE(4 bytes)][DATA]

  STREAM_TYPE:
    - 0x01: stdout
    - 0x02: stderr

파싱 예시 (Rust):
```rust
async fn stream_logs(container_id: &str) -> Result<()> {
    let mut stream = docker.attach_container(
        container_id,
        Some(AttachContainerOptions::<String> {
            stream: Some(true),
            stdout: Some(true),
            stderr: Some(true),
            ..Default::default()
        }),
    );

    while let Some(chunk) = stream.next().await {
        match chunk? {
            LogOutput::StdOut { message } => {
                let line = String::from_utf8_lossy(&message);
                save_log(build_id, &line, "stdout").await;
                broadcast_ws_log(build_id, &line).await;
            }
            LogOutput::StdErr { message } => {
                let line = String::from_utf8_lossy(&message);
                save_log(build_id, &line, "stderr").await;
                broadcast_ws_log(build_id, &line).await;
            }
            _ => {}
        }
    }

    Ok(())
}
```


4.4 빌드 컨테이너 종료 대기
---------------------------

API 호출:
POST /containers/{id}/wait

응답 (컨테이너 종료 시):
{
  "StatusCode": 0  // 0 = 성공, 비-0 = 실패
}

빌드 성공 판단:
  - StatusCode == 0 → 성공
  - StatusCode != 0 → 실패


4.5 런타임 컨테이너 실행
------------------------

목적: 빌드 결과물을 실행하여 서비스 제공

API 호출:
POST /containers/create

Request Body:
{
  "Image": "eclipse-temurin:17-jre",
  "Cmd": [
    "java",
    "-jar",
    "/app/app.jar"
  ],
  "HostConfig": {
    "Binds": [
      "/data/output/build42:/app:ro"
    ],
    "PortBindings": {
      "8080/tcp": [
        {
          "HostPort": "9001"
        }
      ]
    },
    "RestartPolicy": {
      "Name": "unless-stopped"
    },
    "NetworkMode": "bridge"
  },
  "ExposedPorts": {
    "8080/tcp": {}
  },
  "WorkingDir": "/app",
  "Labels": {
    "ci.project_id": "1",
    "ci.build_id": "42",
    "ci.slot": "Blue"
  }
}

포트 매핑 설명:
  - "8080/tcp": 컨테이너 내부 포트
  - "HostPort": "9001": 호스트 포트 (Blue 슬롯)

라벨 용도:
  - 컨테이너 관리 및 조회 시 필터링


시작:
POST /containers/{id}/start

응답: 204 No Content


4.6 Health Check 구현
---------------------

방법: 호스트에서 HTTP 요청

URL 구성:
  - 호스트: localhost
  - 포트: {blue_port} 또는 {green_port}
  - 경로: {project.health_check_url}
  - 예: http://localhost:9001/actuator/health

HTTP 클라이언트 (Rust reqwest):
```rust
async fn health_check(port: u16, path: &str) -> Result<bool> {
    let url = format!("http://localhost:{}{}", port, path);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    match client.get(&url).send().await {
        Ok(response) if response.status().is_success() => Ok(true),
        _ => Ok(false)
    }
}
```

재시도 로직:
  - 최대 시도: 10회
  - 간격: 5초
  - 타임아웃: 30초/요청

Health Check 성공 조건:
  - HTTP 200-299 응답


4.7 컨테이너 중지 및 삭제
-------------------------

중지:
POST /containers/{id}/stop?t=10

Query Parameters:
  - t: 강제 종료 전 대기 시간 (초)

응답: 204 No Content


삭제:
DELETE /containers/{id}?force=true

Query Parameters:
  - force: 실행 중이어도 강제 삭제

응답: 204 No Content


4.8 컨테이너 목록 조회
----------------------

API 호출:
GET /containers/json?all=true&filters={"label":["ci.project_id=1"]}

Query Parameters:
  - all: true (중지된 컨테이너 포함)
  - filters: JSON 필터

응답:
[
  {
    "Id": "abc123...",
    "Names": ["/stoic_darwin"],
    "Image": "eclipse-temurin:17-jre",
    "State": "running",
    "Status": "Up 2 hours",
    "Ports": [
      {
        "PrivatePort": 8080,
        "PublicPort": 9001,
        "Type": "tcp"
      }
    ],
    "Labels": {
      "ci.project_id": "1",
      "ci.build_id": "42",
      "ci.slot": "Blue"
    }
  }
]


4.9 컨테이너 상태 조회
----------------------

API 호출:
GET /containers/{id}/json

응답:
{
  "Id": "abc123...",
  "State": {
    "Status": "running",
    "Running": true,
    "Paused": false,
    "Restarting": false,
    "OOMKilled": false,
    "Dead": false,
    "Pid": 12345,
    "ExitCode": 0,
    "StartedAt": "2025-01-07T10:00:00Z",
    "FinishedAt": "0001-01-01T00:00:00Z"
  },
  "Config": { ... },
  "NetworkSettings": {
    "Ports": {
      "8080/tcp": [
        {
          "HostIp": "0.0.0.0",
          "HostPort": "9001"
        }
      ]
    }
  }
}


4.10 이미지 Pull
----------------

API 호출:
POST /images/create?fromImage=gradle&tag=jdk17

응답: JSON Stream (진행률)
{"status":"Pulling from library/gradle","id":"jdk17"}
{"status":"Pulling fs layer","progressDetail":{},"id":"abc123"}
{"status":"Downloading","progressDetail":{"current":123456,"total":987654},"progress":"[==>  ]"}
{"status":"Download complete","progressDetail":{},"id":"abc123"}
{"status":"Pull complete","progressDetail":{},"id":"abc123"}

이미지 Pull 전략:
  - 프로젝트 생성 시 필요한 이미지 미리 Pull
  - 빌드 시작 전 이미지 존재 확인
  - 없으면 Pull 후 빌드 시작


================================================================================
5. 데이터베이스 스키마
================================================================================

5.1 DBMS
--------
SQLite 3.35+

데이터베이스 파일: /data/db/ci.db


5.2 테이블 정의
---------------

5.2.1 projects (프로젝트)
--------------------------

CREATE TABLE projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    -- 프로젝트 식별
    name TEXT NOT NULL UNIQUE,
    repo TEXT NOT NULL,
    path_filter TEXT NOT NULL DEFAULT '**',
    branch TEXT NOT NULL DEFAULT 'main',

    -- 빌드 설정
    build_image TEXT NOT NULL,
    build_command TEXT NOT NULL,
    cache_type TEXT NOT NULL DEFAULT 'none',

    -- 배포 설정
    runtime_image TEXT NOT NULL,
    runtime_command TEXT NOT NULL,
    health_check_url TEXT NOT NULL DEFAULT '/',
    health_check_timeout INTEGER NOT NULL DEFAULT 30,
    health_check_interval INTEGER NOT NULL DEFAULT 5,
    health_check_retries INTEGER NOT NULL DEFAULT 10,

    -- 포트 설정 (자동 할당)
    blue_port INTEGER NOT NULL,
    green_port INTEGER NOT NULL,

    -- 현재 상태
    active_slot TEXT NOT NULL DEFAULT 'Blue' CHECK(active_slot IN ('Blue', 'Green')),
    blue_container_id TEXT,
    green_container_id TEXT,

    -- GitHub Webhook Secret
    webhook_secret TEXT,

    -- 통계
    total_builds INTEGER NOT NULL DEFAULT 0,
    successful_builds INTEGER NOT NULL DEFAULT 0,
    failed_builds INTEGER NOT NULL DEFAULT 0,
    last_build_id INTEGER,
    last_build_at DATETIME,

    -- 타임스탬프
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

인덱스:
CREATE UNIQUE INDEX idx_projects_name ON projects(name);
CREATE INDEX idx_projects_repo ON projects(repo);
CREATE INDEX idx_projects_updated_at ON projects(updated_at);


5.2.2 builds (빌드 히스토리)
----------------------------

CREATE TABLE builds (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    -- 프로젝트 연결
    project_id INTEGER NOT NULL,
    build_number INTEGER NOT NULL,  -- 프로젝트별 증가 번호

    -- Git 정보
    commit_hash TEXT NOT NULL,
    commit_message TEXT,
    commit_author TEXT,

    -- 빌드 상태
    status TEXT NOT NULL CHECK(status IN ('Queued', 'Building', 'Deploying', 'Success', 'Failed')),

    -- 배포 정보
    deployed_slot TEXT CHECK(deployed_slot IN ('Blue', 'Green')),
    container_id TEXT,

    -- Health Check
    health_check_attempts INTEGER DEFAULT 0,
    health_check_success BOOLEAN DEFAULT 0,

    -- 경로
    log_path TEXT NOT NULL,
    output_path TEXT,

    -- 타임스탬프
    queued_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    started_at DATETIME,
    finished_at DATETIME,
    duration_seconds INTEGER,

    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);

인덱스:
CREATE INDEX idx_builds_project_id ON builds(project_id);
CREATE INDEX idx_builds_status ON builds(status);
CREATE INDEX idx_builds_queued_at ON builds(queued_at);
CREATE UNIQUE INDEX idx_builds_project_build_number ON builds(project_id, build_number);


5.2.3 build_stages (빌드 단계)
------------------------------

CREATE TABLE build_stages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    build_id INTEGER NOT NULL,
    name TEXT NOT NULL,  -- "Git Pull", "Build", "Deploy", "Health Check"
    status TEXT NOT NULL CHECK(status IN ('Pending', 'Running', 'Success', 'Failed')),

    started_at DATETIME,
    finished_at DATETIME,
    duration_seconds INTEGER,

    error_message TEXT,

    FOREIGN KEY (build_id) REFERENCES builds(id) ON DELETE CASCADE
);

인덱스:
CREATE INDEX idx_build_stages_build_id ON build_stages(build_id);


5.3 주요 쿼리
-------------

5.3.1 프로젝트 생성 및 포트 자동 할당
--------------------------------------

-- 1. 다음 프로젝트 ID 예측
SELECT COALESCE(MAX(id), 0) + 1 AS next_id FROM projects;

-- 2. 포트 계산 (Rust 코드에서)
let blue_port = 9000 + (next_id * 10) + 1;
let green_port = 9000 + (next_id * 10) + 2;

-- 3. 프로젝트 생성
INSERT INTO projects (
    name, repo, path_filter, branch,
    build_image, build_command, cache_type,
    runtime_image, runtime_command, health_check_url,
    blue_port, green_port, webhook_secret
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);


5.3.2 빌드 번호 증가 및 생성
----------------------------

-- 1. 프로젝트별 다음 빌드 번호
SELECT COALESCE(MAX(build_number), 0) + 1 AS next_build_number
FROM builds
WHERE project_id = ?;

-- 2. 빌드 생성
INSERT INTO builds (
    project_id, build_number, commit_hash, commit_message, commit_author,
    status, log_path, output_path
) VALUES (?, ?, ?, ?, ?, 'Queued', ?, ?);

-- 3. 프로젝트 통계 업데이트
UPDATE projects
SET total_builds = total_builds + 1,
    last_build_id = ?,
    last_build_at = CURRENT_TIMESTAMP,
    updated_at = CURRENT_TIMESTAMP
WHERE id = ?;


5.3.3 빌드 상태 업데이트
------------------------

UPDATE builds
SET status = ?,
    started_at = CASE WHEN ? = 'Building' THEN CURRENT_TIMESTAMP ELSE started_at END,
    finished_at = CASE WHEN ? IN ('Success', 'Failed') THEN CURRENT_TIMESTAMP ELSE finished_at END,
    duration_seconds = CASE
        WHEN ? IN ('Success', 'Failed')
        THEN CAST((julianday(CURRENT_TIMESTAMP) - julianday(started_at)) * 86400 AS INTEGER)
        ELSE duration_seconds
    END
WHERE id = ?;


5.3.4 배포 슬롯 전환
--------------------

-- 1. 컨테이너 ID 업데이트
UPDATE projects
SET blue_container_id = CASE WHEN ? = 'Blue' THEN ? ELSE blue_container_id END,
    green_container_id = CASE WHEN ? = 'Green' THEN ? ELSE green_container_id END,
    updated_at = CURRENT_TIMESTAMP
WHERE id = ?;

-- 2. 활성 슬롯 전환
UPDATE projects
SET active_slot = ?,
    updated_at = CURRENT_TIMESTAMP
WHERE id = ?;

-- 3. 빌드에 배포 정보 기록
UPDATE builds
SET deployed_slot = ?,
    container_id = ?,
    status = 'Success',
    finished_at = CURRENT_TIMESTAMP,
    duration_seconds = CAST((julianday(CURRENT_TIMESTAMP) - julianday(started_at)) * 86400 AS INTEGER)
WHERE id = ?;


5.3.5 프로젝트별 최근 빌드 조회
-------------------------------

SELECT
    b.*,
    p.name AS project_name
FROM builds b
JOIN projects p ON b.project_id = p.id
WHERE b.project_id = ?
ORDER BY b.build_number DESC
LIMIT ? OFFSET ?;


5.3.6 Repository로 프로젝트 찾기
--------------------------------

SELECT * FROM projects
WHERE repo = ? AND branch = ?
LIMIT 1;


5.3.7 진행 중인 빌드 조회
-------------------------

SELECT * FROM builds
WHERE project_id = ?
  AND status IN ('Queued', 'Building', 'Deploying')
ORDER BY build_number ASC;


================================================================================
6. 내부 이벤트 시스템
================================================================================

6.1 이벤트 버스 구조
--------------------

Rust 구현:
```rust
use tokio::sync::broadcast;

pub struct EventBus {
    sender: broadcast::Sender<Event>,
}

pub enum Event {
    BuildStatusChanged {
        build_id: i64,
        project_id: i64,
        status: BuildStatus,
    },
    LogLine {
        build_id: i64,
        line: String,
    },
    DeploymentCompleted {
        build_id: i64,
        project_id: i64,
        slot: Slot,
    },
    HealthCheckProgress {
        build_id: i64,
        attempt: u32,
        success: bool,
    },
    ContainerStatusChanged {
        project_id: i64,
        slot: Slot,
        status: ContainerStatus,
    },
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1000);
        Self { sender }
    }

    pub fn publish(&self, event: Event) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }
}
```


6.2 이벤트 발행 타이밍
----------------------

6.2.1 BuildStatusChanged
-------------------------

발행 시점:
  1. 빌드가 큐에 추가될 때 (Queued)
  2. 빌드가 시작될 때 (Building)
  3. 배포가 시작될 때 (Deploying)
  4. 빌드가 성공할 때 (Success)
  5. 빌드가 실패할 때 (Failed)

핸들러:
  - WebSocket: 구독자에게 브로드캐스트
  - DB: 빌드 상태 업데이트
  - 로깅: 시스템 로그 기록


6.2.2 LogLine
-------------

발행 시점:
  - Docker 컨테이너에서 stdout/stderr 라인을 읽을 때마다

핸들러:
  - 파일: /data/logs/{project_id}/{build_number}.log에 기록
  - WebSocket: 구독자에게 실시간 전송
  - 버퍼: 메모리에 최근 100줄 유지 (빠른 조회용)


6.2.3 DeploymentCompleted
--------------------------

발행 시점:
  - Health Check 성공 후 슬롯 전환 완료 시

핸들러:
  - WebSocket: 구독자에게 배포 완료 알림
  - DB: 빌드 상태 및 프로젝트 활성 슬롯 업데이트
  - Cleanup: 이전 슬롯 컨테이너 중지


6.2.4 HealthCheckProgress
--------------------------

발행 시점:
  - Health Check 각 시도마다

핸들러:
  - WebSocket: 구독자에게 진행 상황 전송
  - DB: build.health_check_attempts 업데이트


6.2.5 ContainerStatusChanged
-----------------------------

발행 시점:
  - 컨테이너가 시작될 때
  - 컨테이너가 중지될 때
  - 컨테이너가 오류 상태가 될 때

핸들러:
  - WebSocket: 구독자에게 상태 변경 알림
  - DB: 프로젝트 컨테이너 ID 업데이트


6.3 WebSocket 브로드캐스트 매핑
-------------------------------

이벤트 → WebSocket 메시지:

Event::BuildStatusChanged { build_id, project_id, status } =>
    WsMessage {
        type: "build_status",
        build_id,
        project_id,
        status,
        timestamp: now()
    }

Event::LogLine { build_id, line } =>
    WsMessage {
        type: "log",
        build_id,
        line,
        timestamp: now()
    }

Event::DeploymentCompleted { build_id, project_id, slot } =>
    WsMessage {
        type: "deployment",
        build_id,
        project_id,
        deployed_slot: slot,
        status: "Success",
        timestamp: now()
    }

Event::HealthCheckProgress { build_id, attempt, success } =>
    WsMessage {
        type: "health_check",
        build_id,
        attempt,
        status: if success { "Success" } else { "Checking" },
        timestamp: now()
    }


6.4 빌드 큐 관리
----------------

구조:
```rust
pub struct BuildQueue {
    // 프로젝트별 큐
    queues: Arc<RwLock<HashMap<i64, VecDeque<i64>>>>,

    // 현재 실행 중인 빌드
    running: Arc<RwLock<HashMap<i64, i64>>>,
}

impl BuildQueue {
    pub async fn enqueue(&self, project_id: i64, build_id: i64) {
        let mut queues = self.queues.write().await;
        queues.entry(project_id)
            .or_insert_with(VecDeque::new)
            .push_back(build_id);
    }

    pub async fn dequeue(&self) -> Option<(i64, i64)> {
        let mut queues = self.queues.write().await;
        let running = self.running.read().await;

        // 실행 중이지 않은 프로젝트의 큐에서 꺼내기
        for (project_id, queue) in queues.iter_mut() {
            if !running.contains_key(project_id) {
                if let Some(build_id) = queue.pop_front() {
                    return Some((*project_id, build_id));
                }
            }
        }

        None
    }

    pub async fn mark_running(&self, project_id: i64, build_id: i64) {
        let mut running = self.running.write().await;
        running.insert(project_id, build_id);
    }

    pub async fn mark_completed(&self, project_id: i64) {
        let mut running = self.running.write().await;
        running.remove(&project_id);
    }
}
```

동작:
  1. Webhook 수신 → BuildQueue.enqueue(project_id, build_id)
  2. 백그라운드 워커가 주기적으로 BuildQueue.dequeue() 호출
  3. 빌드 시작 → BuildQueue.mark_running(project_id, build_id)
  4. 빌드 완료 → BuildQueue.mark_completed(project_id)
  5. 다음 빌드 dequeue


6.5 백그라운드 워커
-------------------

```rust
async fn run_build_queue(state: Arc<AppState>) {
    loop {
        // 큐에서 빌드 꺼내기
        if let Some((project_id, build_id)) = state.build_queue.dequeue().await {
            state.build_queue.mark_running(project_id, build_id).await;

            let state_clone = state.clone();
            tokio::spawn(async move {
                match execute_build(build_id, &state_clone).await {
                    Ok(_) => {
                        info!("Build #{} completed successfully", build_id);
                    }
                    Err(e) => {
                        error!("Build #{} failed: {}", build_id, e);
                    }
                }

                state_clone.build_queue.mark_completed(project_id).await;
            });
        }

        // 100ms 대기 후 재시도
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
```


================================================================================
7. 파일 시스템 구조
================================================================================

7.1 디렉토리 레이아웃
---------------------

/opt/lightweight-ci/
├── agent/                           # Rust 소스 코드
│   ├── src/
│   ├── Cargo.toml
│   └── Dockerfile
│
├── data/                            # 모든 런타임 데이터
│   ├── cache/                       # 전역 캐시 (패키지 매니저별)
│   │   ├── gradle/
│   │   │   ├── caches/
│   │   │   └── wrapper/
│   │   ├── maven/
│   │   │   └── repository/
│   │   ├── npm/
│   │   │   └── _cacache/
│   │   ├── pip/
│   │   │   └── http/
│   │   └── cargo/
│   │       ├── registry/
│   │       └── git/
│   │
│   ├── workspace/                   # 프로젝트별 소스 코드
│   │   ├── 1/                       # project_id = 1
│   │   │   ├── .git/
│   │   │   ├── src/
│   │   │   ├── build.gradle
│   │   │   └── ...
│   │   └── 2/                       # project_id = 2
│   │       ├── .git/
│   │       └── ...
│   │
│   ├── output/                      # 빌드별 결과물
│   │   ├── 1/                       # build_id = 1
│   │   │   └── app.jar
│   │   ├── 2/                       # build_id = 2
│   │   │   └── app.jar
│   │   └── ...
│   │
│   ├── logs/                        # 빌드 로그
│   │   ├── 1/                       # project_id = 1
│   │   │   ├── 1.log                # build_number = 1
│   │   │   ├── 2.log
│   │   │   └── ...
│   │   └── 2/                       # project_id = 2
│   │       └── ...
│   │
│   └── db/                          # 데이터베이스
│       └── ci.db                    # SQLite 파일
│
└── docker-compose.yml


7.2 볼륨 마운트 매핑
--------------------

7.2.1 Rust Agent 컨테이너
--------------------------

docker-compose.yml:
```yaml
services:
  agent:
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - ./data:/data
```

매핑:
  호스트: /opt/lightweight-ci/data
  → 컨테이너: /data


7.2.2 빌드 컨테이너 (임시)
---------------------------

docker run 예시:
```bash
docker run --rm \
  -v /opt/lightweight-ci/data/workspace/1:/app:ro \
  -v /opt/lightweight-ci/data/cache/gradle:/root/.gradle \
  -v /opt/lightweight-ci/data/output/42:/output \
  gradle:jdk17 \
  bash -c "cd /app && ./gradlew bootJar && cp build/libs/*.jar /output/"
```

매핑:
  호스트: /opt/lightweight-ci/data/workspace/1
  → 컨테이너: /app (읽기 전용)

  호스트: /opt/lightweight-ci/data/cache/gradle
  → 컨테이너: /root/.gradle (읽기/쓰기)

  호스트: /opt/lightweight-ci/data/output/42
  → 컨테이너: /output (읽기/쓰기)


7.2.3 런타임 컨테이너 (지속)
----------------------------

docker run 예시:
```bash
docker run -d \
  -v /opt/lightweight-ci/data/output/42:/app:ro \
  -p 9001:8080 \
  --restart unless-stopped \
  eclipse-temurin:17-jre \
  java -jar /app/app.jar
```

매핑:
  호스트: /opt/lightweight-ci/data/output/42
  → 컨테이너: /app (읽기 전용)

포트:
  컨테이너: 8080
  → 호스트: 9001


7.3 로그 파일 형식
------------------

경로: /data/logs/{project_id}/{build_number}.log

형식 (줄 단위):
[{timestamp}] [{level}] {message}

예시:
[2025-01-07T12:34:56Z] [INFO] Starting build...
[2025-01-07T12:35:12Z] [INFO] Downloading dependencies...
[2025-01-07T12:36:45Z] [INFO] Compiling src/main/java/App.java
[2025-01-07T12:37:20Z] [INFO] BUILD SUCCESS
[2025-01-07T12:37:21Z] [INFO] Starting deployment to Green slot
[2025-01-07T12:37:25Z] [INFO] Health check 1/10... OK
[2025-01-07T12:37:30Z] [INFO] Deployment complete

타임스탬프: ISO 8601 (UTC)
레벨: INFO | WARN | ERROR | DEBUG


7.4 캐시 디렉토리 상세
----------------------

7.4.1 Gradle 캐시
-----------------

호스트: /data/cache/gradle
컨테이너 마운트: /root/.gradle

구조:
/root/.gradle/
├── caches/               # JAR 캐시
│   ├── modules-2/
│   └── transforms-3/
└── wrapper/              # Gradle Wrapper 캐시
    └── dists/


7.4.2 npm 캐시
--------------

호스트: /data/cache/npm
컨테이너 마운트: /root/.npm

구조:
/root/.npm/
├── _cacache/
└── _logs/


7.4.3 pip 캐시
--------------

호스트: /data/cache/pip
컨테이너 마운트: /root/.cache/pip

구조:
/root/.cache/pip/
├── http/
└── wheels/


7.4.4 Cargo 캐시
----------------

호스트: /data/cache/cargo
컨테이너 마운트: /root/.cargo

구조:
/root/.cargo/
├── registry/
├── git/
└── bin/


7.5 디스크 사용량 관리
----------------------

정책:
  1. 로그 파일: 빌드당 최대 10MB, 30일 이상 자동 삭제
  2. 빌드 결과물: 최근 10개 빌드만 유지, 이전 것 자동 삭제
  3. 캐시: 수동 관리 (전역 캐시이므로 삭제 주의)

정리 스크립트 (Cron):
```bash
#!/bin/bash
# 30일 이상 된 로그 삭제
find /opt/lightweight-ci/data/logs -name "*.log" -mtime +30 -delete

# 프로젝트별로 최근 10개 빌드만 유지
for project_dir in /opt/lightweight-ci/data/output/*; do
    ls -t "$project_dir" | tail -n +11 | xargs -I {} rm -rf "$project_dir/{}"
done
```


================================================================================
8. 에러 코드 정의
================================================================================

8.1 HTTP API 에러 코드
----------------------

클라이언트 에러 (4xx):

VALIDATION_ERROR (400)
  - 요청 데이터 유효성 검증 실패
  - 필수 필드 누락
  - 잘못된 형식

PROJECT_EXISTS (409)
  - 프로젝트 이름 중복

BUILD_IN_PROGRESS (409)
  - 이미 빌드가 진행 중

SIGNATURE_INVALID (403)
  - GitHub Webhook 서명 검증 실패

PROJECT_NOT_FOUND (404)
  - 프로젝트가 존재하지 않음

BUILD_NOT_FOUND (404)
  - 빌드가 존재하지 않음

SLOT_EMPTY (400)
  - 전환하려는 슬롯이 비어있음


서버 에러 (5xx):

INTERNAL_ERROR (500)
  - 일반적인 서버 오류

DATABASE_ERROR (500)
  - 데이터베이스 쿼리 실패

DOCKER_ERROR (500)
  - Docker API 호출 실패

BUILD_FAILED (500)
  - 빌드 실패 (내부 오류)

DEPLOYMENT_FAILED (500)
  - 배포 실패 (내부 오류)


8.2 WebSocket 에러 코드
-----------------------

INVALID_MESSAGE
  - 메시지 형식 오류

RESOURCE_NOT_FOUND
  - 구독하려는 리소스가 존재하지 않음

SUBSCRIPTION_FAILED
  - 구독 실패


8.3 빌드 실패 이유
------------------

BuildFailureReason (DB 저장용):

GIT_PULL_FAILED
  - Git clone/pull 실패

BUILD_COMMAND_FAILED
  - 빌드 명령어 실행 실패 (exit code != 0)

BUILD_TIMEOUT
  - 빌드 타임아웃 (기본 30분)

OUTPUT_NOT_FOUND
  - 빌드 결과물이 /output에 없음

HEALTH_CHECK_FAILED
  - Health Check 최대 재시도 초과

CONTAINER_START_FAILED
  - 런타임 컨테이너 시작 실패

DOCKER_API_ERROR
  - Docker API 호출 오류


================================================================================
9. 통신 흐름 시나리오
================================================================================

9.1 전체 배포 프로세스
----------------------

1. GitHub Push
   ↓
2. GitHub Webhook 전송
   POST https://ci.yourdomain.com/webhook/github
   X-Hub-Signature-256: sha256=...
   Body: { "ref": "refs/heads/main", "commits": [...] }
   ↓
3. Rust Agent: Webhook 수신 및 검증
   - 서명 검증
   - Repository 매칭
   - Branch 필터링
   - Path 필터링
   ↓
4. 빌드 큐 추가
   - DB INSERT into builds (status='Queued')
   - BuildQueue.enqueue(project_id, build_id)
   - Event: BuildStatusChanged { status: Queued }
   - WebSocket → 클라이언트: { "type": "build_status", "status": "Queued" }
   ↓
5. 백그라운드 워커: 빌드 시작
   - BuildQueue.dequeue()
   - DB UPDATE builds SET status='Building', started_at=NOW()
   - Event: BuildStatusChanged { status: Building }
   - WebSocket → 클라이언트: { "type": "build_status", "status": "Building" }
   ↓
6. Git Pull
   - cd /data/workspace/{project_id}
   - git pull origin {branch}
   ↓
7. Docker 빌드 컨테이너 실행
   - Docker API: POST /containers/create
   - Docker API: POST /containers/{id}/start
   - Docker API: POST /containers/{id}/attach (로그 스트리밍)
   ↓
8. 실시간 로그 스트리밍
   - Docker stdout/stderr → Event: LogLine
   - 파일 저장: /data/logs/{project_id}/{build_number}.log
   - WebSocket → 클라이언트: { "type": "log", "line": "..." }
   ↓
9. 빌드 완료 대기
   - Docker API: POST /containers/{id}/wait
   - Response: { "StatusCode": 0 }
   ↓
10. 배포 시작
   - DB UPDATE builds SET status='Deploying'
   - Event: BuildStatusChanged { status: Deploying }
   - WebSocket → 클라이언트: { "type": "build_status", "status": "Deploying" }
   ↓
11. 런타임 컨테이너 시작 (Green 슬롯)
   - Docker API: POST /containers/create
     Image: eclipse-temurin:17-jre
     Binds: /data/output/{build_id}:/app:ro
     PortBindings: 8080 → 9002
   - Docker API: POST /containers/{id}/start
   - Event: ContainerStatusChanged { slot: Green, status: starting }
   ↓
12. Health Check
   - 루프 (최대 10회, 5초 간격):
     - HTTP GET http://localhost:9002/actuator/health
     - Event: HealthCheckProgress { attempt: N }
     - WebSocket → 클라이언트: { "type": "health_check", "attempt": N }
     - 200 OK → 성공, 루프 종료
   ↓
13. 프록시 전환 (Blue → Green)
   - DB UPDATE projects SET active_slot='Green', green_container_id='{id}'
   - 리버스 프록시 라우팅 테이블 갱신
   ↓
14. 이전 컨테이너 종료 (Blue)
   - Docker API: POST /containers/{blue_id}/stop?t=10
   - Docker API: DELETE /containers/{blue_id}
   - Event: ContainerStatusChanged { slot: Blue, status: stopped }
   ↓
15. 배포 완료
   - DB UPDATE builds SET status='Success', finished_at=NOW()
   - DB UPDATE projects SET successful_builds++
   - Event: DeploymentCompleted { slot: Green }
   - WebSocket → 클라이언트:
     {
       "type": "deployment",
       "status": "Success",
       "deployed_slot": "Green",
       "public_url": "https://app.yourdomain.com/my-backend/"
     }


9.2 실시간 로그 스트리밍 흐름
-----------------------------

[클라이언트: 대시보드]
   ↓
1. WebSocket 연결
   wss://ci.yourdomain.com/ws
   ↓
2. 빌드 구독
   → { "type": "subscribe", "target": "build", "build_id": 42 }
   ← { "type": "subscribed", "build_id": 42, "current_status": "Building" }
   ↓
3. [서버: Docker 컨테이너에서 로그 읽기]
   Docker API: POST /containers/{id}/attach?stream=true
   ↓
4. [서버: 로그 라인 수신]
   stdout: "[INFO] Starting build..."
   ↓
5. [서버: 이벤트 발행]
   Event::LogLine { build_id: 42, line: "[INFO] Starting build..." }
   ↓
6. [서버: 파일 저장]
   /data/logs/1/42.log ← "[2025-01-07T12:34:56Z] [INFO] Starting build..."
   ↓
7. [서버: WebSocket 브로드캐스트]
   구독자 필터링: build_id = 42
   ↓
8. [클라이언트: 메시지 수신]
   ← {
       "type": "log",
       "build_id": 42,
       "line": "[INFO] Starting build...",
       "timestamp": "2025-01-07T12:34:56Z"
     }
   ↓
9. [클라이언트: UI 업데이트]
   로그 창에 라인 추가
   자동 스크롤


9.3 Blue/Green 무중단 배포 흐름
-------------------------------

초기 상태:
  - Blue (9001): app-v1.0 실행 중 (활성)
  - Green (9002): 비어있음
  - Proxy: 8080 → 9001

새 빌드 (#42) 시작:
  1. 빌드 완료 → /data/output/42/app.jar
  2. Green 슬롯에 새 컨테이너 시작 (app-v1.1)
     - docker run -d -p 9002:8080 -v /data/output/42:/app:ro ...
  3. Health Check: http://localhost:9002/actuator/health
     - 재시도 1: 503 Service Unavailable (앱 시작 중)
     - 재시도 2: 503 Service Unavailable
     - 재시도 3: 200 OK (성공!)
  4. Proxy 전환: 8080 → 9002
  5. Blue 컨테이너 중지 (app-v1.0)
     - docker stop {blue_id}
     - docker rm {blue_id}

최종 상태:
  - Blue (9001): 비어있음
  - Green (9002): app-v1.1 실행 중 (활성)
  - Proxy: 8080 → 9002

다음 빌드 (#43):
  - Blue 슬롯에 app-v1.2 배포
  - Proxy: 8080 → 9001
  - Green 컨테이너 중지


================================================================================
문서 끝
================================================================================

본 명세서는 Lightweight CI/CD 시스템의 모든 통신 인터페이스를 정의합니다.
구현 시 이 문서를 기준으로 각 모듈 간 데이터 교환이 이루어져야 합니다.

추가 질문이나 불명확한 부분이 있으면 각 섹션을 참조하거나 문서를 업데이트하세요.
