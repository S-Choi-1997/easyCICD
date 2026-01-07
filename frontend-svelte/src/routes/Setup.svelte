<script>
  import { push } from 'svelte-spa-router';
  import { link } from 'svelte-spa-router';
  import { onMount } from 'svelte';

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

  // GitHub PAT 상태
  let githubPAT = '';
  let patConfigured = false;
  let githubUsername = '';
  let showPATInput = false;

  // 레포지토리 관련
  let repositories = [];
  let branches = [];
  let loadingRepos = false;
  let loadingBranches = false;
  let showRepoDropdown = false;
  let showBranchDropdown = false;

  // 빌드/런타임 이미지 프리셋
  const buildImagePresets = [
    { value: 'gradle:jdk17', label: 'Gradle + JDK 17' },
    { value: 'gradle:jdk21', label: 'Gradle + JDK 21' },
    { value: 'maven:3-openjdk-17', label: 'Maven + JDK 17' },
    { value: 'node:20', label: 'Node.js 20' },
    { value: 'node:22', label: 'Node.js 22' },
    { value: 'python:3.11', label: 'Python 3.11' },
    { value: 'python:3.12', label: 'Python 3.12' },
    { value: 'rust:latest', label: 'Rust' },
    { value: 'golang:1.22', label: 'Go 1.22' },
  ];

  const runtimeImagePresets = [
    { value: 'eclipse-temurin:17-jre', label: 'JRE 17' },
    { value: 'eclipse-temurin:21-jre', label: 'JRE 21' },
    { value: 'node:20-slim', label: 'Node.js 20 Slim' },
    { value: 'python:3.11-slim', label: 'Python 3.11 Slim' },
    { value: 'nginx:alpine', label: 'Nginx Alpine' },
    { value: 'debian:trixie-slim', label: 'Debian Trixie' },
  ];

  onMount(async () => {
    await checkPATStatus();
  });

  async function checkPATStatus() {
    try {
      const response = await fetch(`${API_BASE}/settings/github-pat-status`);
      const data = await response.json();
      patConfigured = data.configured;
      if (data.github_username) {
        githubUsername = data.github_username;
      }
    } catch (err) {
      console.error('Failed to check PAT status:', err);
    }
  }

  async function savePAT() {
    if (!githubPAT.trim()) {
      alert('GitHub PAT을 입력해주세요');
      return;
    }

    try {
      const response = await fetch(`${API_BASE}/settings/github-pat`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ github_pat: githubPAT }),
      });

      const data = await response.json();

      if (response.ok) {
        patConfigured = true;
        githubUsername = data.github_username;
        showPATInput = false;
        alert(`GitHub 계정 연결 성공: ${data.github_username}`);
      } else {
        alert(`오류: ${data.error}`);
      }
    } catch (err) {
      alert(`PAT 저장 실패: ${err.message}`);
    }
  }

  async function loadRepositories() {
    if (!patConfigured) {
      alert('먼저 GitHub PAT을 설정해주세요');
      showPATInput = true;
      return;
    }

    loadingRepos = true;
    try {
      const response = await fetch(`${API_BASE}/github/repositories`);
      const data = await response.json();

      if (response.ok) {
        repositories = data.repositories;
        showRepoDropdown = true;
      } else {
        alert(`레포지토리 로딩 실패: ${data.error}`);
      }
    } catch (err) {
      alert(`레포지토리 로딩 실패: ${err.message}`);
    } finally {
      loadingRepos = false;
    }
  }

  async function selectRepository(repo) {
    formData.repo = repo.full_name;
    formData.branch = repo.default_branch;
    showRepoDropdown = false;

    // 브랜치 목록 자동 로딩
    await loadBranches(repo.full_name);
  }

  async function loadBranches(repoFullName) {
    if (!repoFullName) {
      repoFullName = formData.repo;
    }

    if (!repoFullName) {
      alert('레포지토리를 먼저 선택해주세요');
      return;
    }

    const [owner, repo] = repoFullName.split('/');

    loadingBranches = true;
    try {
      const response = await fetch(
        `${API_BASE}/github/branches?owner=${owner}&repo=${repo}`
      );
      const data = await response.json();

      if (response.ok) {
        branches = data.branches;
        showBranchDropdown = true;
      } else {
        alert(`브랜치 로딩 실패: ${data.error}`);
      }
    } catch (err) {
      alert(`브랜치 로딩 실패: ${err.message}`);
    } finally {
      loadingBranches = false;
    }
  }

  function selectBranch(branch) {
    formData.branch = branch.name;
    showBranchDropdown = false;
  }

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
  <!-- GitHub PAT 설정 -->
  <div class="card">
    <div class="card-header">
      <h2 class="card-title">GitHub 연동 설정</h2>
    </div>

    {#if patConfigured}
      <div class="pat-status">
        <span class="status-badge status-running">
          <span class="status-dot"></span>
          연결됨: {githubUsername}
        </span>
        <button
          on:click={() => (showPATInput = !showPATInput)}
          class="btn btn-secondary btn-sm"
        >
          {showPATInput ? '취소' : 'PAT 변경'}
        </button>
      </div>
    {:else}
      <div class="pat-status">
        <span class="status-badge status-failed">
          <span class="status-dot"></span>
          미연결
        </span>
        <button on:click={() => (showPATInput = true)} class="btn btn-primary btn-sm">
          GitHub PAT 설정
        </button>
      </div>
    {/if}

    {#if showPATInput}
      <div class="form-group">
        <label class="form-label" for="github_pat">GitHub Personal Access Token</label>
        <input
          type="password"
          id="github_pat"
          class="form-input"
          bind:value={githubPAT}
          placeholder="ghp_xxxxxxxxxxxxxxxxxxxx"
        />
        <div class="form-help">
          <a
            href="https://github.com/settings/tokens/new"
            target="_blank"
            style="color: var(--primary); text-decoration: underline;"
          >
            GitHub에서 PAT 생성하기
          </a>
          (권한: repo, admin:repo_hook)
        </div>
        <button on:click={savePAT} class="btn btn-primary">저장</button>
      </div>
    {/if}
  </div>

  <!-- 프로젝트 생성 폼 -->
  <div class="card">
    <div class="card-header">
      <h2 class="card-title">새 프로젝트 만들기</h2>
    </div>

    <form on:submit|preventDefault={handleSubmit}>
      <!-- 프로젝트 정보 -->
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
        <div class="input-with-button">
          <input
            type="text"
            id="repo"
            class="form-input"
            bind:value={formData.repo}
            required
            placeholder="username/repository"
          />
          <button
            type="button"
            on:click={loadRepositories}
            class="btn btn-primary btn-sm"
            disabled={loadingRepos}
          >
            {loadingRepos ? '로딩 중...' : '레포 선택'}
          </button>
        </div>
        <div class="form-help">형식: username/repository</div>

        {#if showRepoDropdown && repositories.length > 0}
          <div class="dropdown">
            <div class="dropdown-header">
              <span>레포지토리 선택 ({repositories.length}개)</span>
              <button type="button" on:click={() => (showRepoDropdown = false)} class="close-btn">
                ✕
              </button>
            </div>
            <ul class="dropdown-list">
              {#each repositories as repo}
                <li on:click={() => selectRepository(repo)} class="dropdown-item">
                  <div>
                    <strong>{repo.name}</strong>
                    <span class="text-muted text-sm">{repo.full_name}</span>
                  </div>
                  <span class="text-muted text-xs">
                    {repo.private ? '🔒 Private' : '🌐 Public'}
                  </span>
                </li>
              {/each}
            </ul>
          </div>
        {/if}
      </div>

      <div class="form-group">
        <label class="form-label" for="branch">브랜치</label>
        <div class="input-with-button">
          <input
            type="text"
            id="branch"
            class="form-input"
            bind:value={formData.branch}
            required
          />
          <button
            type="button"
            on:click={() => loadBranches()}
            class="btn btn-primary btn-sm"
            disabled={loadingBranches || !formData.repo}
          >
            {loadingBranches ? '로딩 중...' : '브랜치 선택'}
          </button>
        </div>

        {#if showBranchDropdown && branches.length > 0}
          <div class="dropdown">
            <div class="dropdown-header">
              <span>브랜치 선택 ({branches.length}개)</span>
              <button type="button" on:click={() => (showBranchDropdown = false)} class="close-btn">
                ✕
              </button>
            </div>
            <ul class="dropdown-list">
              {#each branches as branch}
                <li on:click={() => selectBranch(branch)} class="dropdown-item">
                  <div>
                    <strong>{branch.name}</strong>
                    {#if branch.protected}
                      <span class="text-muted text-xs">🔒 Protected</span>
                    {/if}
                  </div>
                </li>
              {/each}
            </ul>
          </div>
        {/if}
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

      <!-- 빌드 설정 -->
      <h3 class="mt-2 mb-2">빌드 설정</h3>

      <div class="form-group">
        <label class="form-label" for="build_image">빌드 이미지</label>
        <div class="preset-selector">
          {#each buildImagePresets as preset}
            <button
              type="button"
              class="preset-btn {formData.build_image === preset.value ? 'active' : ''}"
              on:click={() => (formData.build_image = preset.value)}
            >
              {preset.label}
            </button>
          {/each}
        </div>
        <input
          type="text"
          id="build_image"
          class="form-input"
          bind:value={formData.build_image}
          required
          placeholder="또는 직접 입력"
        />
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

      <!-- 실행 설정 -->
      <h3 class="mt-2 mb-2">실행 설정</h3>

      <div class="form-group">
        <label class="form-label" for="runtime_image">런타임 이미지</label>
        <div class="preset-selector">
          {#each runtimeImagePresets as preset}
            <button
              type="button"
              class="preset-btn {formData.runtime_image === preset.value ? 'active' : ''}"
              on:click={() => (formData.runtime_image = preset.value)}
            >
              {preset.label}
            </button>
          {/each}
        </div>
        <input
          type="text"
          id="runtime_image"
          class="form-input"
          bind:value={formData.runtime_image}
          required
          placeholder="또는 직접 입력"
        />
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

<style>
  .pat-status {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 1rem;
    background: var(--gray-50);
    border-radius: 0.5rem;
    margin-bottom: 1rem;
  }

  .input-with-button {
    display: flex;
    gap: 0.5rem;
  }

  .input-with-button .form-input {
    flex: 1;
  }

  .dropdown {
    margin-top: 0.5rem;
    border: 1px solid var(--gray-300);
    border-radius: 0.5rem;
    background: white;
    max-height: 300px;
    overflow-y: auto;
    box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);
  }

  .dropdown-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 1rem;
    border-bottom: 1px solid var(--gray-200);
    background: var(--gray-50);
    font-weight: 600;
  }

  .close-btn {
    background: none;
    border: none;
    font-size: 1.25rem;
    cursor: pointer;
    color: var(--gray-600);
    padding: 0;
    line-height: 1;
  }

  .close-btn:hover {
    color: var(--gray-900);
  }

  .dropdown-list {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  .dropdown-item {
    padding: 0.75rem 1rem;
    cursor: pointer;
    border-bottom: 1px solid var(--gray-100);
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .dropdown-item:last-child {
    border-bottom: none;
  }

  .dropdown-item:hover {
    background: var(--gray-50);
  }

  .preset-selector {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
  }

  .preset-btn {
    padding: 0.5rem 1rem;
    border: 1px solid var(--gray-300);
    border-radius: 0.375rem;
    background: white;
    cursor: pointer;
    font-size: 0.875rem;
    transition: all 0.2s;
  }

  .preset-btn:hover {
    border-color: var(--primary);
    background: var(--gray-50);
  }

  .preset-btn.active {
    border-color: var(--primary);
    background: var(--primary);
    color: white;
  }
</style>
