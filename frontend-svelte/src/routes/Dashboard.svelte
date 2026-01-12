<script>
  import { onMount, onDestroy } from 'svelte';
  import { link } from 'svelte-spa-router';
  import { projects, projectsLoading, projectsError, loadProjects, triggerBuild, deleteProject } from '../stores/projects';
  import { formatRelativeTime } from '../utils/dateFormatter';
  import Skeleton from '../components/Skeleton.svelte';
  import { fade } from 'svelte/transition';
  import { subscribe } from '../stores/websocket';

  const API_BASE = '/api';
  let domain = null;
  let containers = [];
  let containersLoading = true;
  let showAddMenu = false;
  let showLogsModal = false;
  let currentLogs = [];
  let currentContainerName = '';
  let currentContainerId = null;
  let unsubscribeWs = null;
  let showContainerDetailModal = false;
  let currentContainer = null;

  onMount(async () => {
    await Promise.all([loadDomain(), loadProjects(), loadContainers()]);

    // Subscribe to WebSocket messages for real-time updates
    unsubscribeWs = subscribe('dashboard-containers', (data) => {
      console.log('📡 [WebSocket] 받은 이벤트:', data.type, data);

      // Handle container log events
      if (data.type === 'container_log' && data.container_db_id === currentContainerId && showLogsModal) {
        console.log('📡 [WebSocket] 컨테이너 로그 추가');
        currentLogs = [...currentLogs, data.line];
      }

      // Handle container status updates
      if (data.type === 'standalone_container_status') {
        console.log('📡 [WebSocket] 컨테이너 상태 업데이트, ID:', data.container_db_id, '상태:', data.status);
        const index = containers.findIndex(c => c.id === data.container_db_id);
        console.log('📡 [WebSocket] 컨테이너 인덱스:', index, '현재 컨테이너 개수:', containers.length);
        if (index !== -1) {
          const newStatus = data.status; // Use status as-is (lowercase: 'running' or 'stopped')
          console.log('📡 [WebSocket] 상태 변경:', containers[index].status, '->', newStatus);

          // Create a new array to trigger Svelte reactivity
          containers = containers.map((c, i) =>
            i === index ? {
              ...c,
              status: newStatus,
              container_id: data.docker_id,
            } : c
          );

          console.log('📡 [WebSocket] containers 배열 업데이트 완료, 새 배열 생성됨');
        } else {
          console.warn('📡 [WebSocket] 컨테이너를 찾을 수 없음, 전체 목록 다시 로드');
          loadContainers();
        }
      }
    });
  });

  onDestroy(() => {
    if (unsubscribeWs) {
      unsubscribeWs();
    }
  });

  async function loadDomain() {
    try {
      const response = await fetch(`${API_BASE}/settings/domain`);
      const data = await response.json();
      if (data.configured) {
        domain = data.domain;
      }
    } catch (error) {
      console.error('도메인 로드 실패:', error);
    }
  }

  async function loadContainers() {
    console.log('📦 [loadContainers] 컨테이너 목록 로드 시작');
    containersLoading = true;
    try {
      const response = await fetch(`${API_BASE}/containers`);
      console.log('📦 [loadContainers] API 응답:', response.status, response.ok);
      if (response.ok) {
        const newContainers = await response.json();
        console.log('📦 [loadContainers] 받은 데이터:', newContainers);

        // 각 컨테이너의 상태를 자세히 출력
        newContainers.forEach((c, idx) => {
          console.log(`📦 [Container ${idx}] ID=${c.id}, Name=${c.name}, Status=${c.status}, ContainerID=${c.container_id}`);
        });

        containers = newContainers;
        console.log('📦 [loadContainers] containers 변수 업데이트 완료, 개수:', containers.length);
      }
    } catch (error) {
      console.error('❌ [loadContainers] 컨테이너 로드 실패:', error);
    } finally {
      containersLoading = false;
      console.log('📦 [loadContainers] 로딩 완료');
    }
  }

  async function handleTriggerBuild(projectId) {
    try {
      await triggerBuild(projectId);
    } catch (error) {
      alert('빌드를 시작할 수 없습니다: ' + error.message);
    }
  }

  async function handleDeleteProject(projectId, projectName) {
    if (!confirm(`"${projectName}" 프로젝트를 삭제하시겠습니까?`)) return;
    try {
      await deleteProject(projectId);
    } catch (error) {
      alert('프로젝트를 삭제할 수 없습니다: ' + error.message);
    }
  }

  // 프로젝트 컨테이너 제어
  async function handleProjectStart(projectId) {
    try {
      const response = await fetch(`${API_BASE}/projects/${projectId}/containers/start`, { method: 'POST' });
      if (response.ok) setTimeout(() => loadProjects(), 1000);
      else alert('컨테이너를 시작할 수 없습니다');
    } catch (error) {
      alert('컨테이너를 시작할 수 없습니다: ' + error.message);
    }
  }

  async function handleProjectStop(projectId) {
    try {
      const response = await fetch(`${API_BASE}/projects/${projectId}/containers/stop`, { method: 'POST' });
      if (response.ok) setTimeout(() => loadProjects(), 1000);
      else alert('컨테이너를 중지할 수 없습니다');
    } catch (error) {
      alert('컨테이너를 중지할 수 없습니다: ' + error.message);
    }
  }

  async function handleProjectRestart(projectId) {
    try {
      const response = await fetch(`${API_BASE}/projects/${projectId}/containers/restart`, { method: 'POST' });
      if (response.ok) setTimeout(() => loadProjects(), 1000);
      else alert('컨테이너를 재시작할 수 없습니다');
    } catch (error) {
      alert('컨테이너를 재시작할 수 없습니다: ' + error.message);
    }
  }

  // 독립 컨테이너 제어
  async function handleContainerStart(id) {
    console.log('🚀 [handleContainerStart] 시작 버튼 클릭됨, ID:', id);
    try {
      const response = await fetch(`${API_BASE}/containers/${id}/start`, { method: 'POST' });
      console.log('🚀 [handleContainerStart] API 응답:', response.status, response.ok);
      if (response.ok) {
        console.log('✅ [handleContainerStart] 성공, WebSocket 이벤트로 UI 업데이트 대기');
        // WebSocket event will update the UI automatically
      } else {
        alert('컨테이너를 시작할 수 없습니다');
      }
    } catch (error) {
      console.error('❌ [handleContainerStart] 에러:', error);
      alert('컨테이너를 시작할 수 없습니다: ' + error.message);
    }
  }

  async function handleContainerStop(id) {
    console.log('🛑 [handleContainerStop] 중지 버튼 클릭됨, ID:', id);
    try {
      const response = await fetch(`${API_BASE}/containers/${id}/stop`, { method: 'POST' });
      console.log('🛑 [handleContainerStop] API 응답:', response.status, response.ok);
      if (response.ok) {
        console.log('✅ [handleContainerStop] 성공, WebSocket 이벤트로 UI 업데이트 대기');
        // WebSocket event will update the UI automatically
      } else {
        alert('컨테이너를 중지할 수 없습니다');
      }
    } catch (error) {
      console.error('❌ [handleContainerStop] 에러:', error);
      alert('컨테이너를 중지할 수 없습니다: ' + error.message);
    }
  }

  async function handleContainerDelete(id, name) {
    console.log('🗑️ [handleContainerDelete] 삭제 버튼 클릭됨, ID:', id, 'Name:', name);
    if (!confirm(`"${name}" 컨테이너를 삭제하시겠습니까?`)) {
      console.log('❌ [handleContainerDelete] 사용자가 취소함');
      return;
    }
    try {
      const response = await fetch(`${API_BASE}/containers/${id}`, { method: 'DELETE' });
      console.log('🗑️ [handleContainerDelete] API 응답:', response.status, response.ok);
      if (response.ok) {
        console.log('✅ [handleContainerDelete] 성공, 컨테이너 목록에서 제거');
        // Remove from local state immediately
        containers = containers.filter(c => c.id !== id);
      } else {
        alert('컨테이너를 삭제할 수 없습니다');
      }
    } catch (error) {
      console.error('❌ [handleContainerDelete] 에러:', error);
      alert('컨테이너를 삭제할 수 없습니다: ' + error.message);
    }
  }

  async function handleViewLogs(id, name) {
    console.log('📋 [handleViewLogs] 로그 버튼 클릭됨, ID:', id, 'Name:', name);
    currentContainerId = id;
    currentContainerName = name;
    currentLogs = ['로그를 불러오는 중...'];
    showLogsModal = true;

    try {
      const response = await fetch(`${API_BASE}/containers/${id}/logs`);
      if (response.ok) {
        const data = await response.json();
        currentLogs = data.logs.length > 0 ? data.logs : ['컨테이너가 시작되면 로그가 여기에 표시됩니다...'];
      } else {
        currentLogs = ['로그를 불러올 수 없습니다'];
      }
    } catch (error) {
      currentLogs = [`오류: ${error.message}`];
    }

    // Auto-scroll to bottom when logs update
    setTimeout(scrollLogsToBottom, 100);
  }

  function scrollLogsToBottom() {
    const logViewer = document.querySelector('.log-viewer');
    if (logViewer) {
      logViewer.scrollTop = logViewer.scrollHeight;
    }
  }

  // Auto-scroll when new logs arrive
  $: if (currentLogs.length > 0 && showLogsModal) {
    setTimeout(scrollLogsToBottom, 50);
  }

  function isProjectRunning(project) {
    return !!(project.blue_container_id || project.green_container_id);
  }

  function getProjectUrl(projectName) {
    const baseDomain = domain || 'albl.cloud';
    const protocol = domain && !domain.includes('localhost') ? 'https' : 'http';
    return `${protocol}://${projectName}-app.${baseDomain}/`;
  }

  function getContainerUrl(containerName) {
    const baseDomain = domain || 'albl.cloud';
    const protocol = domain && !domain.includes('localhost') ? 'https' : 'http';
    return `${protocol}://${containerName}.${baseDomain}/`;
  }

  function handleContainerClick(container) {
    console.log('🐳 [handleContainerClick] 컨테이너 카드 클릭됨, Name:', container.name, 'ID:', container.id);
    currentContainer = container;
    showContainerDetailModal = true;
  }

  $: totalCount = $projects.length + containers.length;
  $: loading = $projectsLoading || containersLoading;

  // 컨테이너 배열이 변경될 때마다 상태 로그 출력
  $: {
    console.log('🔄 [Reactive] containers 배열 업데이트됨, 총 개수:', containers.length);
    containers.forEach((c, idx) => {
      console.log(`🔄 [Reactive Container ${idx}] ID=${c.id}, Name=${c.name}, Status=${c.status}`);
    });
  }
</script>

<header>
  <div class="header-content">
    <a href="/" use:link style="text-decoration: none; color: inherit;">
      <h1>Easy CI/CD</h1>
    </a>
    <div class="header-actions">
      <a href="/settings" use:link class="btn btn-secondary">설정</a>
      <div class="dropdown">
        <button class="btn btn-primary" on:click={() => showAddMenu = !showAddMenu}>
          + 추가
        </button>
        {#if showAddMenu}
          <div class="dropdown-menu" on:mouseleave={() => showAddMenu = false}>
            <a href="/setup" use:link class="dropdown-item" on:click={() => showAddMenu = false}>
              프로젝트
            </a>
            <a href="/containers/new" use:link class="dropdown-item" on:click={() => showAddMenu = false}>
              컨테이너
            </a>
          </div>
        {/if}
      </div>
    </div>
  </div>
</header>

<div class="container">
  <div class="card">
    <div class="card-header">
      <h2 class="card-title">컨테이너 목록</h2>
      <span class="item-count">{totalCount}개</span>
    </div>

    {#if loading}
      <Skeleton type="project-card" count={3} />
    {:else if $projectsError}
      <div class="empty-state" transition:fade>
        <h3>로딩 오류</h3>
        <p>{$projectsError}</p>
        <button on:click={() => { loadProjects(); loadContainers(); }} class="btn btn-primary mt-2">다시 시도</button>
      </div>
    {:else if totalCount === 0}
      <div class="empty-state" transition:fade>
        <h3>컨테이너가 없습니다</h3>
        <p>프로젝트나 컨테이너를 추가하세요</p>
        <div class="empty-actions">
          <a href="/setup" use:link class="btn btn-primary">프로젝트 추가</a>
          <a href="/containers/new" use:link class="btn btn-secondary">컨테이너 추가</a>
        </div>
      </div>
    {:else}
      <div class="items-list" transition:fade>
        <!-- 프로젝트 (빌드 가능한 컨테이너) -->
        {#each $projects as project (project.id)}
          <div class="item-card" transition:fade>
            <div class="item-main" on:click={() => window.location.hash = `/project/${project.id}`}
                 on:keydown={(e) => e.key === 'Enter' && (window.location.hash = `/project/${project.id}`)}
                 role="button" tabindex="0">
              <div class="item-header">
                <div class="item-info">
                  <span class="item-type project">PROJECT</span>
                  <span class="item-name">{project.name}</span>
                  <div class="status-badges">
                    <span class="status-badge build-status {project.last_build_status?.toLowerCase() || 'unknown'}">
                      {project.last_build_status || 'N/A'}
                    </span>
                    <span class="status-badge deploy-status {isProjectRunning(project) ? 'running' : 'stopped'}">
                      {isProjectRunning(project) ? 'Running' : 'Stopped'}
                    </span>
                  </div>
                </div>
                <div class="item-actions">
                  <button on:click|stopPropagation={() => handleTriggerBuild(project.id)} class="btn btn-primary btn-sm" title="빌드">
                    빌드
                  </button>
                  {#if isProjectRunning(project)}
                    <button on:click|stopPropagation={() => handleProjectRestart(project.id)} class="btn btn-secondary btn-sm" title="재시작">
                      재시작
                    </button>
                    <button on:click|stopPropagation={() => handleProjectStop(project.id)} class="btn btn-danger btn-sm" title="중지">
                      중지
                    </button>
                  {:else}
                    <button on:click|stopPropagation={() => handleProjectStart(project.id)} class="btn btn-success btn-sm" title="시작">
                      시작
                    </button>
                  {/if}
                  <button on:click|stopPropagation={() => handleDeleteProject(project.id, project.name)} class="btn btn-outline btn-sm" title="삭제">
                    삭제
                  </button>
                </div>
              </div>
              <div class="item-details">
                <span>{project.repo}</span>
                <span>·</span>
                <span>{project.branch}</span>
                {#if project.updated_at}
                  <span>·</span>
                  <span>{formatRelativeTime(project.updated_at)}</span>
                {/if}
              </div>
              {#if isProjectRunning(project)}
                <a href="{getProjectUrl(project.name)}" target="_blank" rel="noopener noreferrer"
                   class="item-url" on:click|stopPropagation>
                  {getProjectUrl(project.name)}
                </a>
              {/if}
            </div>
          </div>
        {/each}

        <!-- 독립 컨테이너 -->
        {#each containers as container (container.id)}
          <div class="item-card clickable" on:click={() => handleContainerClick(container)} transition:fade>
            <div class="item-main">
              <div class="item-header">
                <div class="item-info">
                  <span class="item-type container">CONTAINER</span>
                  <span class="item-name">{container.name}</span>
                  <span class="status-badge {container.status === 'running' ? 'running' : 'stopped'}">
                    {container.status === 'running' ? 'Running' : 'Stopped'}
                  </span>
                </div>
                <div class="item-actions">
                  <button on:click|stopPropagation={() => handleViewLogs(container.id, container.name)}
                          class="btn btn-secondary btn-sm"
                          title="로그"
                          disabled={container.status !== 'running'}>
                    로그
                  </button>
                  {#if container.status === 'running'}
                    <button on:click|stopPropagation={() => handleContainerStop(container.id)} class="btn btn-danger btn-sm" title="중지">
                      중지
                    </button>
                  {:else}
                    <button on:click|stopPropagation={() => handleContainerStart(container.id)} class="btn btn-success btn-sm" title="시작">
                      시작
                    </button>
                  {/if}
                  <button on:click|stopPropagation={() => handleContainerDelete(container.id, container.name)}
                          class="btn btn-outline btn-sm" title="삭제"
                          disabled={container.status === 'running'}>
                    삭제
                  </button>
                </div>
              </div>
              <div class="item-details">
                <span>{container.image}</span>
                {#if container.port}
                  <span>·</span>
                  <span>외부 포트: {container.port}</span>
                  {#if container.container_port}
                    <span>→ {container.container_port}</span>
                  {/if}
                {/if}
                {#if container.persist_data}
                  <span>·</span>
                  <span>영구 저장</span>
                {/if}
              </div>
              {#if container.status === 'running'}
                <a href="{getContainerUrl(container.name)}" target="_blank" rel="noopener noreferrer"
                   class="item-url" on:click|stopPropagation>
                  {getContainerUrl(container.name)}
                </a>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<!-- Logs Modal -->
{#if showLogsModal}
  <div class="modal-overlay" on:click={() => showLogsModal = false} transition:fade>
    <div class="modal-content" on:click|stopPropagation>
      <div class="modal-header">
        <h3>{currentContainerName} 로그</h3>
        <button on:click={() => showLogsModal = false} class="btn-close">✕</button>
      </div>
      <div class="modal-body">
        <div class="log-viewer">
          {#each currentLogs as log}
            <div class="log-line">{log}</div>
          {/each}
        </div>
      </div>
      <div class="modal-footer">
        <span style="color: var(--gray-600); font-size: 0.875rem;">
          실시간 스트리밍 중... {currentLogs.length}줄
        </span>
        <button on:click={() => showLogsModal = false} class="btn btn-primary">
          닫기
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Container Detail Modal -->
{#if showContainerDetailModal && currentContainer}
  <div class="modal-overlay" on:click={() => showContainerDetailModal = false} transition:fade>
    <div class="modal-content detail-modal" on:click|stopPropagation>
      <div class="modal-header">
        <h3>{currentContainer.name}</h3>
        <button on:click={() => showContainerDetailModal = false} class="btn-close">✕</button>
      </div>
      <div class="modal-body">
        <div class="detail-section">
          <div class="detail-row">
            <span class="detail-label">상태:</span>
            <span class="status-badge {currentContainer.status === 'Running' ? 'running' : 'stopped'}">
              {currentContainer.status === 'Running' ? '실행 중' : '중지'}
            </span>
          </div>
          <div class="detail-row">
            <span class="detail-label">이미지:</span>
            <span>{currentContainer.image}</span>
          </div>
          <div class="detail-row">
            <span class="detail-label">포트 매핑:</span>
            <span>{currentContainer.port} → {currentContainer.container_port || currentContainer.port}</span>
          </div>
          {#if currentContainer.container_id}
            <div class="detail-row">
              <span class="detail-label">Docker ID:</span>
              <span class="mono-text">{currentContainer.container_id.substring(0, 12)}</span>
            </div>
          {/if}
          <div class="detail-row">
            <span class="detail-label">영구 저장:</span>
            <span>{currentContainer.persist_data ? '✓ 활성화' : '✗ 비활성화'}</span>
          </div>
          {#if currentContainer.command}
            <div class="detail-row">
              <span class="detail-label">커맨드:</span>
              <span class="mono-text">{currentContainer.command}</span>
            </div>
          {/if}
          {#if currentContainer.env_vars}
            <div class="detail-row">
              <span class="detail-label">환경 변수:</span>
              <div class="env-vars">
                {#each Object.entries(currentContainer.env_vars) as [key, value]}
                  <div class="env-var">
                    <span class="env-key">{key}:</span>
                    <span class="env-value">{value}</span>
                  </div>
                {/each}
              </div>
            </div>
          {/if}
          <div class="detail-row">
            <span class="detail-label">생성 시간:</span>
            <span>{currentContainer.created_at}</span>
          </div>
        </div>
      </div>
      <div class="modal-footer">
        <button on:click={() => showContainerDetailModal = false} class="btn btn-primary">
          닫기
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .item-count {
    color: var(--gray-600);
    font-size: 0.875rem;
  }

  .items-list {
    display: flex;
    flex-direction: column;
  }

  .item-card {
    display: flex;
    flex-direction: column;
    padding: 1.25rem 1.5rem;
    border-bottom: 1px solid var(--gray-200);
    gap: 0.75rem;
    transition: background 0.15s;
  }

  .item-card:last-child {
    border-bottom: none;
  }

  .item-card:hover {
    background: var(--gray-50);
  }

  .item-main {
    flex: 1;
    cursor: pointer;
    min-width: 0;
  }

  .item-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }

  .item-info {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    min-width: 0;
    flex: 1;
  }

  .item-type {
    font-size: 0.625rem;
    font-weight: 600;
    padding: 0.125rem 0.375rem;
    border-radius: 0.25rem;
    text-transform: uppercase;
    flex-shrink: 0;
  }

  .item-type.project {
    background: #dbeafe;
    color: #1d4ed8;
  }

  .item-type.container {
    background: #f3e8ff;
    color: #7c3aed;
  }

  .item-name {
    font-weight: 600;
    font-size: 1rem;
    color: var(--gray-900);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .item-details {
    font-size: 0.813rem;
    color: var(--gray-600);
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .item-url {
    font-size: 0.75rem;
    color: var(--primary);
    text-decoration: none;
    margin-top: 0.25rem;
    display: inline-block;
  }

  .item-url:hover {
    text-decoration: underline;
  }

  .item-actions {
    display: flex;
    gap: 0.5rem;
    flex-shrink: 0;
    align-items: center;
  }

  .item-actions .btn {
    min-width: 60px;
    text-align: center;
  }

  .status-badges {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }

  .status-badge {
    font-size: 0.688rem;
    font-weight: 600;
    padding: 0.25rem 0.625rem;
    border-radius: 0.25rem;
    flex-shrink: 0;
    text-transform: uppercase;
    letter-spacing: 0.025em;
  }

  /* 배포 상태 */
  .status-badge.deploy-status.running {
    background: #10b981;
    color: white;
  }

  .status-badge.deploy-status.stopped {
    background: #6b7280;
    color: white;
  }

  /* 빌드 상태 */
  .status-badge.build-status.success {
    background: #2563eb;
    color: white;
  }

  .status-badge.build-status.building,
  .status-badge.build-status.queued {
    background: #f59e0b;
    color: white;
  }

  .status-badge.build-status.failed {
    background: #dc2626;
    color: white;
  }

  .status-badge.build-status.unknown {
    background: #9ca3af;
    color: white;
  }

  /* 단일 상태 배지 (컨테이너) */
  .status-badge.running {
    background: #10b981;
    color: white;
  }

  .status-badge.stopped {
    background: #6b7280;
    color: white;
  }

  .empty-state {
    text-align: center;
    padding: 3rem 1rem;
    color: var(--gray-600);
  }

  .empty-state h3 {
    font-size: 1.125rem;
    font-weight: 600;
    margin-bottom: 0.5rem;
    color: var(--gray-800);
  }

  .empty-actions {
    display: flex;
    gap: 0.75rem;
    justify-content: center;
    margin-top: 1rem;
  }

  .mt-2 {
    margin-top: 0.5rem;
  }

  /* Buttons */
  .btn-success {
    background: #10b981;
    color: white;
    border: none;
  }

  .btn-success:hover:not(:disabled) {
    background: #059669;
  }

  .btn-outline {
    background: transparent;
    border: 1px solid var(--gray-300);
    color: var(--gray-600);
  }

  .btn-outline:hover:not(:disabled) {
    background: var(--gray-100);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Dropdown */
  .dropdown {
    position: relative;
  }

  .dropdown-menu {
    position: absolute;
    top: 100%;
    right: 0;
    margin-top: 0.25rem;
    background: white;
    border-radius: 0.5rem;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    min-width: 140px;
    z-index: 100;
    overflow: hidden;
  }

  .dropdown-item {
    display: block;
    padding: 0.625rem 1rem;
    color: var(--gray-700);
    text-decoration: none;
    font-size: 0.875rem;
  }

  .dropdown-item:hover {
    background: var(--gray-100);
  }

  /* Modal */
  .modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .modal-content {
    background: white;
    border-radius: 0.5rem;
    box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.1);
    max-width: 800px;
    width: 90%;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1.5rem;
    border-bottom: 1px solid var(--gray-200);
  }

  .modal-header h3 {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 600;
    color: var(--gray-900);
  }

  .btn-close {
    background: none;
    border: none;
    font-size: 1.5rem;
    cursor: pointer;
    color: var(--gray-400);
    padding: 0;
    width: 2rem;
    height: 2rem;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 0.25rem;
  }

  .btn-close:hover {
    background: var(--gray-100);
    color: var(--gray-600);
  }

  .modal-body {
    flex: 1;
    overflow: auto;
    padding: 1.5rem;
  }

  .log-viewer {
    background: #1e1e1e;
    color: #d4d4d4;
    padding: 1rem;
    border-radius: 0.375rem;
    font-family: 'Courier New', monospace;
    font-size: 0.813rem;
    line-height: 1.5;
    overflow-x: auto;
    max-height: 50vh;
  }

  .log-line {
    white-space: pre-wrap;
    word-break: break-all;
    margin-bottom: 0.25rem;
  }

  .modal-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.75rem;
    padding: 1.5rem;
    border-top: 1px solid var(--gray-200);
  }

  /* Container Detail Modal */
  .detail-modal {
    max-width: 600px;
  }

  .detail-section {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .detail-row {
    display: flex;
    gap: 1rem;
    align-items: flex-start;
  }

  .detail-label {
    font-weight: 600;
    color: var(--gray-700);
    min-width: 100px;
    flex-shrink: 0;
  }

  .mono-text {
    font-family: 'Courier New', monospace;
    font-size: 0.875rem;
    background: var(--gray-100);
    padding: 0.125rem 0.375rem;
    border-radius: 0.25rem;
  }

  .env-vars {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    flex: 1;
  }

  .env-var {
    display: flex;
    gap: 0.5rem;
    padding: 0.5rem;
    background: var(--gray-50);
    border-radius: 0.25rem;
    font-family: 'Courier New', monospace;
    font-size: 0.813rem;
  }

  .env-key {
    font-weight: 600;
    color: var(--primary);
  }

  .env-value {
    color: var(--gray-700);
  }

</style>
