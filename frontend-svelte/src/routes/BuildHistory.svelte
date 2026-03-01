<script>
  import { onMount, onDestroy } from 'svelte';
  import { link } from 'svelte-spa-router';
  import { fade } from 'svelte/transition';
  import { formatRelativeTime, formatAbsoluteTime, formatDuration } from '../utils/dateFormatter';
  import { formatCommitHash, formatCommitMessage, getCommitUrl } from '../utils/commitParser';
  import Skeleton from '../components/Skeleton.svelte';

  export let params = {};
  const projectId = params.id;
  const API_BASE = '/api';

  let project = null;
  let builds = [];
  let selectedBuild = null;
  let loading = true;
  let error = null;
  let ws = null;
  let buildLogs = [];
  let deployLogs = [];
  let isStreaming = false;
  let showBuildLogs = true;
  let showDeployLogs = true;

  onMount(async () => {
    await loadProjectInfo();
    await loadBuilds();
    connectWebSocket();
  });

  onDestroy(() => {
    if (ws) {
      ws.close();
    }
  });

  function connectWebSocket() {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/ws`;

    ws = new WebSocket(wsUrl);

    ws.onopen = () => {};

    ws.onmessage = (event) => {
      const data = JSON.parse(event.data);
      handleWebSocketMessage(data);
    };

    ws.onerror = (err) => {
      console.error('WebSocket error:', err);
    };

    ws.onclose = () => {
      setTimeout(connectWebSocket, 3000);
    };
  }

  function handleWebSocketMessage(data) {
    if (data.type === 'Log' && selectedBuild && data.build_id === selectedBuild.id) {
      // 로그를 빌드/배포 단계로 분리
      const logLine = data.line || '';
      const isDeployLog = logLine.includes('[DEPLOY]') || logLine.includes('Deploying') || selectedBuild.status === 'Deploying';

      if (isDeployLog) {
        deployLogs = [...deployLogs, logLine];
      } else {
        buildLogs = [...buildLogs, logLine];
      }

      isStreaming = true;

      // Auto scroll to bottom
      setTimeout(() => {
        const logViewers = document.querySelectorAll('.log-viewer');
        logViewers.forEach(viewer => {
          if (viewer) viewer.scrollTop = viewer.scrollHeight;
        });
      }, 10);
    } else if (data.type === 'BuildStatus' && data.project_id === parseInt(projectId)) {
      // Update builds list immediately for real-time status badge update
      builds = builds.map(build =>
        build.id === data.build_id
          ? { ...build, status: data.status }
          : build
      );

      // Update selected build status
      if (selectedBuild && data.build_id === selectedBuild.id) {
        selectedBuild = {...selectedBuild, status: data.status};

        // Stop streaming when build completes
        if (data.status === 'Success' || data.status === 'Failed') {
          isStreaming = false;
        }
      }

      // Reload full builds list (for any other changes like finished_at)
      loadBuilds();
    }
  }

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
      if (response.ok) {
        setTimeout(() => loadBuilds(), 1000);
      }
    } catch (err) {
      console.error(err);
    }
  }

  async function showBuildDetail(buildId) {
    try {
      const response = await fetch(`${API_BASE}/builds/${buildId}`);
      if (!response.ok) throw new Error('빌드 상세 정보를 가져올 수 없습니다');
      selectedBuild = await response.json();

      // Reset logs and fetch from log files
      buildLogs = [];
      deployLogs = [];
      isStreaming = selectedBuild.status === 'Building' || selectedBuild.status === 'Deploying' || selectedBuild.status === 'Queued';

      // Fetch existing logs from files
      await Promise.all([
        loadBuildLogs(buildId),
        loadDeployLogs(buildId)
      ]);
    } catch (err) {
      console.error(err);
    }
  }

  async function loadBuildLogs(buildId) {
    try {
      const response = await fetch(`${API_BASE}/builds/${buildId}/build-logs`);
      if (response.ok) {
        const text = await response.text();
        if (text) {
          buildLogs = text.split('\n').filter(line => line.trim());
        }
      }
    } catch (err) {
      console.error('빌드 로그 로딩 실패:', err);
    }
  }

  async function loadDeployLogs(buildId) {
    try {
      const response = await fetch(`${API_BASE}/builds/${buildId}/deploy-logs`);
      if (response.ok) {
        const text = await response.text();
        if (text) {
          deployLogs = text.split('\n').filter(line => line.trim());
        }
      }
    } catch (err) {
      console.error('배포 로그 로딩 실패:', err);
    }
  }

  function getCommitLink(build) {
    if (!project) return null;
    return getCommitUrl(project.repo, build.commit_hash);
  }
</script>

<header>
  <div class="header-content">
    <a href="/" use:link style="text-decoration: none; color: inherit; cursor: pointer;">
      <h1>Easy CI/CD</h1>
    </a>
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
      <div class="loading" transition:fade>
        <Skeleton type="build-list" count={5} />
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
      <ul class="build-list" transition:fade>
        {#each builds as build (build.id)}
          <li class="build-item" on:click={() => showBuildDetail(build.id)} style="cursor: pointer;" transition:fade>
            <div class="build-info">
              <span class="build-number">#{build.build_number}</span>
              <span class="status-badge status-{build.status.toLowerCase()}">
                <span class="status-dot"></span>
                {build.status}
              </span>
              {#if getCommitLink(build)}
                <a href={getCommitLink(build)} target="_blank" rel="noopener noreferrer" class="build-commit-link" on:click|stopPropagation>
                  {formatCommitHash(build.commit_hash)}
                </a>
              {:else}
                <span class="build-commit">{formatCommitHash(build.commit_hash)}</span>
              {/if}
              {#if build.commit_message}
                <span class="text-muted">{formatCommitMessage(build.commit_message)}</span>
              {/if}
            </div>
            <div class="build-time" title={formatAbsoluteTime(build.created_at)}>
              {formatRelativeTime(build.created_at)}
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  <!-- Build Detail -->
  {#if selectedBuild}
    <div class="card" transition:fade>
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
          <div>
            <strong>커밋:</strong>
            {#if getCommitLink(selectedBuild)}
              <a href={getCommitLink(selectedBuild)} target="_blank" rel="noopener noreferrer" class="build-commit-link">
                {formatCommitHash(selectedBuild.commit_hash)}
              </a>
            {:else}
              <span class="build-commit">{formatCommitHash(selectedBuild.commit_hash)}</span>
            {/if}
          </div>
          {#if selectedBuild.commit_message}
            <div><strong>메시지:</strong> {formatCommitMessage(selectedBuild.commit_message)}</div>
          {/if}
          {#if selectedBuild.author}
            <div><strong>작성자:</strong> {selectedBuild.author}</div>
          {/if}
          <div>
            <strong>시작 시각:</strong>
            <span title={formatAbsoluteTime(selectedBuild.created_at)}>
              {formatRelativeTime(selectedBuild.created_at)} ({formatAbsoluteTime(selectedBuild.created_at)})
            </span>
          </div>
          {#if selectedBuild.finished_at}
            <div>
              <strong>완료 시각:</strong>
              <span title={formatAbsoluteTime(selectedBuild.finished_at)}>
                {formatRelativeTime(selectedBuild.finished_at)} ({formatAbsoluteTime(selectedBuild.finished_at)})
              </span>
            </div>
            <div>
              <strong>소요 시간:</strong> {formatDuration(selectedBuild.created_at, selectedBuild.finished_at)}
            </div>
          {:else if selectedBuild.status === 'Building' || selectedBuild.status === 'Queued'}
            <div>
              <strong>진행 시간:</strong> {formatDuration(selectedBuild.created_at, new Date().toISOString())}
            </div>
          {/if}
        </div>
      </div>

      <!-- Build Logs -->
      <div class="log-section">
        <div class="log-header" on:click={() => showBuildLogs = !showBuildLogs} style="cursor: pointer;">
          <h3>
            {showBuildLogs ? '▼' : '▶'} 빌드 로그
            {#if buildLogs.length > 0}
              <span class="log-count">({buildLogs.length}줄)</span>
            {/if}
          </h3>
          {#if isStreaming && selectedBuild.status === 'Building'}
            <span class="streaming-badge">🔴 실시간 스트리밍</span>
          {/if}
        </div>
        {#if showBuildLogs}
          <div class="log-viewer" transition:fade>
            {#if buildLogs.length === 0}
              <div class="log-line text-muted">
                {#if isStreaming}
                  빌드를 시작하는 중...
                {:else}
                  빌드 로그가 없습니다
                {/if}
              </div>
            {:else}
              {#each buildLogs as log, idx}
                <div class="log-line">
                  <span class="log-number">{idx + 1}</span>
                  <span class="log-content">{log}</span>
                </div>
              {/each}
            {/if}
          </div>
        {/if}
      </div>

      <!-- Deploy Logs -->
      <div class="log-section">
        <div class="log-header" on:click={() => showDeployLogs = !showDeployLogs} style="cursor: pointer;">
          <h3>
            {showDeployLogs ? '▼' : '▶'} 배포 로그
            {#if deployLogs.length > 0}
              <span class="log-count">({deployLogs.length}줄)</span>
            {/if}
          </h3>
          {#if isStreaming && selectedBuild.status === 'Deploying'}
            <span class="streaming-badge">🔴 실시간 스트리밍</span>
          {/if}
        </div>
        {#if showDeployLogs}
          <div class="log-viewer" transition:fade>
            {#if deployLogs.length === 0}
              <div class="log-line text-muted">
                배포 로그가 없습니다
              </div>
            {:else}
              {#each deployLogs as log, idx}
                <div class="log-line">
                  <span class="log-number">{idx + 1}</span>
                  <span class="log-content">{log}</span>
                </div>
              {/each}
            {/if}
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .log-section {
    margin-bottom: 1.5rem;
  }

  .log-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 1rem;
    background: var(--gray-100);
    border-radius: 0.375rem;
    margin-bottom: 0.5rem;
    user-select: none;
  }

  .log-header:hover {
    background: var(--gray-200);
  }

  .log-header h3 {
    margin: 0;
    font-size: 1rem;
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .log-count {
    font-size: 0.75rem;
    color: var(--gray-600);
    font-weight: normal;
  }

  .streaming-badge {
    font-size: 0.875rem;
    font-weight: 500;
    padding: 0.25rem 0.75rem;
    background: #fef2f2;
    color: #dc2626;
    border-radius: 9999px;
    animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
  }

  @keyframes pulse {
    0%, 100% {
      opacity: 1;
    }
    50% {
      opacity: 0.5;
    }
  }

  .log-viewer {
    background: #1e1e1e;
    color: #d4d4d4;
    font-family: 'Courier New', Consolas, monospace;
    font-size: 0.875rem;
    padding: 1rem;
    border-radius: 0.375rem;
    max-height: 500px;
    overflow-y: auto;
    line-height: 1.5;
  }

  .log-line {
    display: flex;
    margin-bottom: 0.25rem;
  }

  .log-number {
    color: #6b7280;
    min-width: 3rem;
    text-align: right;
    margin-right: 1rem;
    user-select: none;
  }

  .log-content {
    flex: 1;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .text-muted {
    color: #9ca3af;
  }

  .build-commit-link {
    color: #3b82f6;
    text-decoration: none;
    font-family: 'Courier New', Consolas, monospace;
    font-weight: 500;
    padding: 0.125rem 0.25rem;
    border-radius: 0.25rem;
    transition: background-color 0.15s;
  }

  .build-commit-link:hover {
    background-color: #eff6ff;
    text-decoration: underline;
  }
</style>
