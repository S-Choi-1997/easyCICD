<script>
  import { push } from 'svelte-spa-router';
  import { link } from 'svelte-spa-router';

  const API_BASE = '/api';

  let formData = {
    name: '',
    repo: '',
    branch: 'main',
    path_filter: '**',
    build_image: '',
    cache_type: 'gradle',
    build_command: '',
    runtime_image: '',
    runtime_command: '',
    health_check_url: '/',
  };

  let submitting = false;

  async function handleSubmit() {
    submitting = true;

    try {
      const response = await fetch(`${API_BASE}/projects`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(formData),
      });

      if (!response.ok) {
        const error = await response.json();
        throw new Error(error.message || '프로젝트 생성 실패');
      }

      const project = await response.json();
      alert(`"${project.name}" 프로젝트가 생성되었습니다!`);
      push('/');
    } catch (error) {
      alert('프로젝트 생성 실패: ' + error.message);
    } finally {
      submitting = false;
    }
  }
</script>

<header>
  <div class="header-content">
    <h1>Easy CI/CD</h1>
    <div class="header-actions">
      <a href="/" use:link class="btn btn-secondary">← 대시보드로 돌아가기</a>
    </div>
  </div>
</header>

<div class="container">
  <div class="card">
    <div class="card-header">
      <h2 class="card-title">새 프로젝트 만들기</h2>
    </div>

    <form on:submit|preventDefault={handleSubmit}>
      <!-- Project Info -->
      <div class="form-group">
        <label class="form-label" for="name">프로젝트 이름</label>
        <input
          type="text"
          id="name"
          class="form-input"
          bind:value={formData.name}
          required
          pattern="[a-z0-9-]+"
          placeholder="my-backend"
        />
        <div class="form-help">소문자, 숫자, 하이픈(-)만 사용 가능</div>
      </div>

      <div class="form-group">
        <label class="form-label" for="repo">GitHub 레포지토리</label>
        <input
          type="text"
          id="repo"
          class="form-input"
          bind:value={formData.repo}
          required
          placeholder="username/repository"
        />
        <div class="form-help">형식: username/repository</div>
      </div>

      <div class="form-group">
        <label class="form-label" for="branch">브랜치</label>
        <input
          type="text"
          id="branch"
          class="form-input"
          bind:value={formData.branch}
          required
        />
      </div>

      <div class="form-group">
        <label class="form-label" for="path_filter">경로 필터</label>
        <input
          type="text"
          id="path_filter"
          class="form-input"
          bind:value={formData.path_filter}
        />
        <div class="form-help">모든 파일은 **, 특정 경로는 src/**,tests/** 형식으로 입력</div>
      </div>

      <!-- Build Settings -->
      <h3 class="mt-2 mb-2">빌드 설정</h3>

      <div class="form-group">
        <label class="form-label" for="build_image">빌드 이미지</label>
        <input
          type="text"
          id="build_image"
          class="form-input"
          bind:value={formData.build_image}
          required
          placeholder="gradle:jdk17"
        />
        <div class="form-help">빌드용 Docker 이미지 (예: gradle:jdk17, node:20)</div>
      </div>

      <div class="form-group">
        <label class="form-label" for="cache_type">캐시 타입</label>
        <select id="cache_type" class="form-select" bind:value={formData.cache_type} required>
          <option value="gradle">Gradle</option>
          <option value="maven">Maven</option>
          <option value="npm">NPM</option>
          <option value="pip">Pip</option>
          <option value="cargo">Cargo</option>
          <option value="none">없음</option>
        </select>
      </div>

      <div class="form-group">
        <label class="form-label" for="build_command">빌드 명령어</label>
        <textarea
          id="build_command"
          class="form-textarea"
          bind:value={formData.build_command}
          required
          placeholder="./gradlew clean bootJar && cp build/libs/*.jar /output/app.jar"
        ></textarea>
        <div class="form-help">프로젝트 빌드 명령어. 결과물은 /output/ 폴더에 복사해야 합니다</div>
      </div>

      <!-- Runtime Settings -->
      <h3 class="mt-2 mb-2">실행 설정</h3>

      <div class="form-group">
        <label class="form-label" for="runtime_image">런타임 이미지</label>
        <input
          type="text"
          id="runtime_image"
          class="form-input"
          bind:value={formData.runtime_image}
          required
          placeholder="eclipse-temurin:17-jre"
        />
        <div class="form-help">앱 실행용 Docker 이미지</div>
      </div>

      <div class="form-group">
        <label class="form-label" for="runtime_command">실행 명령어</label>
        <input
          type="text"
          id="runtime_command"
          class="form-input"
          bind:value={formData.runtime_command}
          required
          placeholder="java -jar /app/app.jar"
        />
        <div class="form-help">애플리케이션 시작 명령어</div>
      </div>

      <div class="form-group">
        <label class="form-label" for="health_check_url">헬스체크 URL</label>
        <input
          type="text"
          id="health_check_url"
          class="form-input"
          bind:value={formData.health_check_url}
          required
          placeholder="/actuator/health"
        />
        <div class="form-help">헬스체크 경로 (예: /health, /actuator/health)</div>
      </div>

      <div class="form-group">
        <button type="submit" class="btn btn-primary" disabled={submitting}>
          {submitting ? '생성 중...' : '프로젝트 생성'}
        </button>
        <a href="/" use:link class="btn btn-secondary">취소</a>
      </div>
    </form>
  </div>
</div>
