<script>
  import { onMount } from 'svelte';
  import { link } from 'svelte-spa-router';

  export let params = {};
  const projectId = params.id;
  const API_BASE = '/api';

  let project = null;
  let builds = [];
  let selectedBuild = null;
  let loading = true;
  let error = null;

  onMount(async () => {
    await loadProjectInfo();
    await loadBuilds();
  });

  async function loadProjectInfo() {
    try {
      const response = await fetch(`${API_BASE}/projects/${projectId}`);
      if (!response.ok) throw new Error('프로젝트 정보를 가져올 수 없습니다');
      project = await response.json();
    } catch (err) {
      error = err.message;
    }
  }

  async function loadBuilds() {
    loading = true;
    error = null;

    try {
      const response = await fetch(`${API_BASE}/builds?project_id=${projectId}&limit=50`);
      if (!response.ok) throw new Error('빌드 목록을 가져올 수 없습니다');
      builds = await response.json();
    } catch (err) {
      error = err.message;
    } finally {
      loading = false;
    }
  }

  async function triggerBuild() {
    try {
      const response = await fetch(`${API_BASE}/projects/${projectId}/builds`, {
        method: 'POST'
      });
      if (!response.ok) throw new Error('빌드를 시작할 수 없습니다');

      const result = await response.json();
      alert(`빌드 #${result.build_id}가 시작되었습니다!`);
      setTimeout(() => loadBuilds(), 1000);
    } catch (err) {
      alert('빌드 시작 실패: ' + err.message);
    }
  }

  async function showBuildDetail(buildId) {
    try {
      const response = await fetch(`${API_BASE}/builds/${buildId}`);
      if (!response.ok) throw new Error('빌드 상세 정보를 가져올 수 없습니다');
      selectedBuild = await response.json();
    } catch (err) {
      alert('빌드 상세 정보 로딩 실패: ' + err.message);
    }
  }

  function formatTimeAgo(dateString) {
    const date = new Date(dateString);
    const now = new Date();
    const seconds = Math.floor((now - date) / 1000);

    if (seconds < 60) return '방금 전';
    if (seconds < 3600) return `${Math.floor(seconds / 60)}분 전`;
    if (seconds < 86400) return `${Math.floor(seconds / 3600)}시간 전`;
    return `${Math.floor(seconds / 86400)}일 전`;
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
  <!-- Project Info -->
  {#if project}
    <div class="card">
      <div class="card-header">
        <h2 class="card-title">{project.name}</h2>
      </div>
      <div class="project-info">
        <div><strong>레포지토리:</strong> {project.repo}</div>
        <div><strong>브랜치:</strong> {project.branch}</div>
        <div><strong>활성 슬롯:</strong> {project.active_slot}</div>
        <div><strong>경로 필터:</strong> {project.path_filter}</div>
      </div>
    </div>
  {/if}

  <!-- Build List -->
  <div class="card">
    <div class="card-header">
      <h2 class="card-title">빌드 내역</h2>
      <button on:click={triggerBuild} class="btn btn-primary btn-sm">빌드 시작</button>
    </div>

    {#if loading}
      <div class="loading">
        <div class="spinner"></div>
        <p>빌드 불러오는 중...</p>
      </div>
    {:else if error}
      <div class="empty-state">
        <h3>빌드 로딩 오류</h3>
        <p>{error}</p>
      </div>
    {:else if builds.length === 0}
      <div class="empty-state">
        <p>빌드 내역이 없습니다</p>
        <button on:click={triggerBuild} class="btn btn-primary mt-2">첫 번째 빌드 시작</button>
      </div>
    {:else}
      <ul class="build-list">
        {#each builds as build}
          <li class="build-item" on:click={() => showBuildDetail(build.id)} style="cursor: pointer;">
            <div class="build-info">
              <span class="build-number">#{build.build_number}</span>
              <span class="status-badge status-{build.status.toLowerCase()}">
                <span class="status-dot"></span>
                {build.status}
              </span>
              <span class="build-commit">{build.commit_hash.substring(0, 7)}</span>
              {#if build.commit_message}
                <span class="text-muted">{build.commit_message}</span>
              {/if}
            </div>
            <div class="build-time">{formatTimeAgo(build.created_at)}</div>
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  <!-- Build Detail -->
  {#if selectedBuild}
    <div class="card">
      <div class="card-header">
        <h2 class="card-title">빌드 #{selectedBuild.build_number}</h2>
        <button on:click={() => selectedBuild = null} class="btn btn-secondary btn-sm">닫기</button>
      </div>

      <div class="mb-2">
        <div class="flex flex-gap">
          <span class="status-badge status-{selectedBuild.status.toLowerCase()}">
            <span class="status-dot"></span>
            {selectedBuild.status}
          </span>
        </div>
        <div class="project-info mt-2">
          <div><strong>커밋:</strong> <span class="build-commit">{selectedBuild.commit_hash}</span></div>
          {#if selectedBuild.commit_message}
            <div><strong>메시지:</strong> {selectedBuild.commit_message}</div>
          {/if}
          {#if selectedBuild.author}
            <div><strong>작성자:</strong> {selectedBuild.author}</div>
          {/if}
          <div><strong>시작 시각:</strong> {new Date(selectedBuild.created_at).toLocaleString('ko-KR')}</div>
          {#if selectedBuild.finished_at}
            <div><strong>완료 시각:</strong> {new Date(selectedBuild.finished_at).toLocaleString('ko-KR')}</div>
          {/if}
        </div>
      </div>

      <h3>빌드 로그</h3>
      <div class="log-viewer">
        <div class="log-line">로그 파일: {selectedBuild.log_path}</div>
        <div class="log-line text-muted">로그 스트리밍은 아직 구현되지 않았습니다</div>
      </div>
    </div>
  {/if}
</div>
