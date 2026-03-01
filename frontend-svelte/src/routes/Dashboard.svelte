<script>
  import { onMount, onDestroy } from 'svelte';
  import { link } from 'svelte-spa-router';
  import { projects, projectsLoading, projectsError, loadProjects, triggerBuild, deleteProject } from '../stores/projects';
  import { formatRelativeTime } from '../utils/dateFormatter';
  import Skeleton from '../components/Skeleton.svelte';
  import ErrorModal from '../components/ErrorModal.svelte';
  import Terminal from '../components/Terminal.svelte';
  import { fade } from 'svelte/transition';
  import { subscribe } from '../stores/websocket';
  import { logout } from '../stores/auth';

  const API_BASE = '/api';
  let domain = null;
  let tcpDomain = null;
  let serverIp = null;
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
  let showTerminalModal = false;
  let terminalContainerId = null;

  // Container transition states
  let containerTransitions = new Map(); // { id: 'starting' | 'stopping' }
  let now = new Date(); // 실시간 갱신을 위한 현재 시간

  // Error modal state
  let showErrorModal = false;
  let errorModalTitle = '오류 발생';
  let errorModalMessage = '';
  let errorModalDetails = '';


  // 10초 간격으로 현재 시간 갱신 → 상대 시간 표시 실시간 갱신
  let tickInterval;
  onMount(async () => {
    await Promise.all([loadDomain(), loadTcpDomain(), loadServerIp(), loadProjects(), loadContainers()]);
    tickInterval = setInterval(() => { now = new Date(); }, 10000);

    // Subscribe to WebSocket messages for real-time updates
    unsubscribeWs = subscribe('dashboard', (data) => {
      if (data.type === 'container_log' && data.container_db_id === currentContainerId && showLogsModal) {
        currentLogs = [...currentLogs, data.line];
      }

      if (data.type === 'standalone_container_status') {
        const index = containers.findIndex(c => c.id === data.container_db_id);
        if (index !== -1) {
          containers = containers.map((c, i) =>
            i === index ? {
              ...c,
              status: data.status,
              container_id: data.docker_id,
            } : c
          );

          if (containerTransitions.has(data.container_db_id)) {
            containerTransitions.delete(data.container_db_id);
            containerTransitions = new Map(containerTransitions);
          }
        } else {
          loadContainers();
        }
      }

      if (data.type === 'container_status') {
        loadProjects();
      }

      if (data.type === 'project_container_status') {
        projects.update(projectList => {
          return projectList.map(proj => {
            if (proj.id === data.project_id) {
              const updates = { ...proj };
              if (data.slot === 'Blue') {
                updates.blue_container_id = data.status === 'running' ? data.docker_id : null;
              } else if (data.slot === 'Green') {
                updates.green_container_id = data.status === 'running' ? data.docker_id : null;
              }
              return updates;
            }
            return proj;
          });
        });
      }

      if (data.type === 'build_status') {
        projects.update(projectList => {
          return projectList.map(proj => {
            if (proj.id === data.project_id) {
              return { ...proj, last_build_status: data.status };
            }
            return proj;
          });
        });
      }

      if (data.type === 'deployment') {
        projects.update(projectList => {
          return projectList.map(proj => {
            if (proj.id === data.project_id) {
              return { ...proj, active_slot: data.slot };
            }
            return proj;
          });
        });
      }
    });
  });

  onDestroy(() => {
    if (unsubscribeWs) {
      unsubscribeWs();
    }
    if (tickInterval) {
      clearInterval(tickInterval);
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

  async function loadTcpDomain() {
    try {
      const response = await fetch(`${API_BASE}/settings/tcp-domain`);
      const data = await response.json();
      if (data.configured) {
        tcpDomain = data.tcp_domain;
      }
    } catch (error) {
      console.error('TCP 도메인 로드 실패:', error);
    }
  }

  async function loadServerIp() {
    try {
      const response = await fetch(`${API_BASE}/settings/server-ip`);
      const data = await response.json();
      serverIp = data.server_ip;
    } catch (error) {
      console.error('서버 IP 로드 실패:', error);
    }
  }

  async function loadContainers() {
    containersLoading = true;
    try {
      const response = await fetch(`${API_BASE}/containers`);
      if (response.ok) {
        containers = await response.json();
      }
    } catch (error) {
      console.error('컨테이너 로드 실패:', error);
    } finally {
      containersLoading = false;
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
    if (containerTransitions.has(id)) return;

    containerTransitions.set(id, 'starting');
    containerTransitions = new Map(containerTransitions);

    const timeoutId = setTimeout(() => {
      if (containerTransitions.has(id)) {
        containerTransitions.delete(id);
        containerTransitions = new Map(containerTransitions);
      }
    }, 30000);

    try {
      const response = await fetch(`${API_BASE}/containers/${id}/start`, { method: 'POST' });

      if (!response.ok) {
        const data = await response.json();
        clearTimeout(timeoutId);
        containerTransitions.delete(id);
        containerTransitions = new Map(containerTransitions);

        showErrorModal = true;
        errorModalTitle = '컨테이너 시작 실패';
        errorModalMessage = data.error || '컨테이너를 시작할 수 없습니다';
        errorModalDetails = `컨테이너 ID: ${id}\nHTTP 상태: ${response.status}`;
      } else {
        clearTimeout(timeoutId);
      }
    } catch (error) {
      clearTimeout(timeoutId);
      containerTransitions.delete(id);
      containerTransitions = new Map(containerTransitions);

      showErrorModal = true;
      errorModalTitle = '컨테이너 시작 실패';
      errorModalMessage = '네트워크 오류가 발생했습니다';
      errorModalDetails = error.message;
    }
  }

  async function handleContainerStop(id) {
    if (containerTransitions.has(id)) return;

    containerTransitions.set(id, 'stopping');
    containerTransitions = new Map(containerTransitions);

    const timeoutId = setTimeout(() => {
      if (containerTransitions.has(id)) {
        containerTransitions.delete(id);
        containerTransitions = new Map(containerTransitions);
      }
    }, 30000);

    try {
      const response = await fetch(`${API_BASE}/containers/${id}/stop`, { method: 'POST' });

      if (!response.ok) {
        const data = await response.json();
        clearTimeout(timeoutId);
        containerTransitions.delete(id);
        containerTransitions = new Map(containerTransitions);

        showErrorModal = true;
        errorModalTitle = '컨테이너 중지 실패';
        errorModalMessage = data.error || '컨테이너를 중지할 수 없습니다';
        errorModalDetails = `컨테이너 ID: ${id}\nHTTP 상태: ${response.status}`;
      } else {
        clearTimeout(timeoutId);
      }
    } catch (error) {
      clearTimeout(timeoutId);
      containerTransitions.delete(id);
      containerTransitions = new Map(containerTransitions);

      showErrorModal = true;
      errorModalTitle = '컨테이너 중지 실패';
      errorModalMessage = '네트워크 오류가 발생했습니다';
      errorModalDetails = error.message;
    }
  }

  async function handleContainerDelete(id, name) {
    if (!confirm(`"${name}" 컨테이너를 삭제하시겠습니까?`)) return;
    try {
      const response = await fetch(`${API_BASE}/containers/${id}`, { method: 'DELETE' });
      if (response.ok) {
        containers = containers.filter(c => c.id !== id);
      } else {
        alert('컨테이너를 삭제할 수 없습니다');
      }
    } catch (error) {
      alert('컨테이너를 삭제할 수 없습니다: ' + error.message);
    }
  }

  async function handleViewLogs(id, name) {
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
    // active_slot에 해당하는 컨테이너만 확인
    if (project.active_slot === 'Blue') {
      return !!project.blue_container_id;
    } else if (project.active_slot === 'Green') {
      return !!project.green_container_id;
    }
    return false;
  }

  function getProjectUrl(projectName) {
    // Remove protocol if present (e.g., "https://cicd.albl.cloud" -> "cicd.albl.cloud")
    let baseDomain = domain || 'albl.cloud';
    baseDomain = baseDomain.replace(/^https?:\/\//, '');
    const protocol = baseDomain && !baseDomain.includes('localhost') ? 'https' : 'http';
    return `${protocol}://${projectName}-app.${baseDomain}/`;
  }

  function getContainerUrl(container) {
    // HTTP인 경우 기존 방식대로 서브도메인 사용
    // Remove protocol if present
    let baseDomain = domain || 'albl.cloud';
    baseDomain = baseDomain.replace(/^https?:\/\//, '');
    const protocol = baseDomain && !baseDomain.includes('localhost') ? 'https' : 'http';
    return `${protocol}://${container.name}.${baseDomain}/`;
  }

  function getTcpUrls(container) {
    // TCP 컨테이너는 도메인과 IP 두 개를 반환
    const urls = [];
    if (tcpDomain) {
      urls.push(`${tcpDomain}:${container.port}`);
    }
    // 서버 실제 IP 추가
    if (serverIp && serverIp !== 'localhost') {
      urls.push(`${serverIp}:${container.port}`);
    }
    return urls;
  }

  function isContainerTcp(container) {
    return container.protocol_type === 'tcp';
  }

  function handleContainerClick(event, container) {
    if (event.target.closest('button')) return;

    currentContainer = container;
    showContainerDetailModal = true;
  }

  $: totalCount = $projects.length + containers.length;
  $: loading = $projectsLoading || containersLoading;

</script>

<header>
  <div class="header-content">
    <a href="/" use:link style="text-decoration: none; color: inherit;">
      <h1>Easy CI/CD</h1>
    </a>
    <div class="header-actions">
      <button class="btn btn-secondary" on:click={logout} title="로그아웃">
        로그아웃
      </button>
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
                    <span class="status-badge slot-badge {project.active_slot?.toLowerCase() || 'blue'} {isProjectRunning(project) ? 'running' : 'stopped'}">
                      {project.active_slot || 'Blue'}
                    </span>
                  </div>
                </div>
                <div class="item-actions">
                  <button type="button" on:click|stopPropagation={() => handleTriggerBuild(project.id)} class="btn btn-primary btn-sm" title="빌드">
                    빌드
                  </button>
                  {#if isProjectRunning(project)}
                    <button type="button" on:click|stopPropagation={() => handleProjectRestart(project.id)} class="btn btn-secondary btn-sm" title="재시작">
                      재시작
                    </button>
                    <button type="button" on:click|stopPropagation={() => handleProjectStop(project.id)} class="btn btn-danger btn-sm" title="중지">
                      중지
                    </button>
                  {:else}
                    <button type="button" on:click|stopPropagation={() => handleProjectStart(project.id)} class="btn btn-success btn-sm" title="시작">
                      시작
                    </button>
                  {/if}
                  <button type="button" on:click|stopPropagation={() => handleDeleteProject(project.id, project.name)} class="btn btn-outline btn-sm" title="삭제">
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
                  <span>{formatRelativeTime(project.updated_at, now)}</span>
                {/if}
              </div>
              <a href="{getProjectUrl(project.name)}" target="_blank" rel="noopener noreferrer"
                 class="item-url" on:click|stopPropagation>
                {getProjectUrl(project.name)}
              </a>
            </div>
          </div>
        {/each}

        <!-- 독립 컨테이너 -->
        {#each containers as container (container.id)}
          <div class="item-card" transition:fade>
            <div class="item-main">
              <div class="item-header">
                <div class="item-info">
                  <span class="item-type container">CONTAINER</span>
                  <span class="item-name" on:click={(e) => handleContainerClick(e, container)} style="cursor: pointer;">{container.name}</span>
                  <div class="status-badges">
                    {#if container.status === 'pulling'}
                      <span class="status-badge transitioning">이미지 풀링 중...</span>
                    {:else if container.status === 'starting' || containerTransitions.get(container.id) === 'starting'}
                      <span class="status-badge transitioning">시작 중...</span>
                    {:else if containerTransitions.get(container.id) === 'stopping'}
                      <span class="status-badge transitioning">중지 중...</span>
                    {:else if container.status === 'running'}
                      <span class="status-badge running">Running</span>
                    {:else}
                      <span class="status-badge stopped">Stopped</span>
                    {/if}
                  </div>
                </div>
                <div class="item-actions">
                  <button type="button" on:click|stopPropagation={() => handleViewLogs(container.id, container.name)}
                          class="btn btn-secondary btn-sm"
                          title="로그"
                          disabled={container.status !== 'running' || containerTransitions.has(container.id)}>
                    로그
                  </button>
                  <button type="button" on:click|stopPropagation={() => { terminalContainerId = container.id; showTerminalModal = true; }}
                          class="btn btn-secondary btn-sm"
                          title="터미널"
                          disabled={container.status !== 'running' || containerTransitions.has(container.id)}>
                    터미널
                  </button>
                  {#if container.status === 'running'}
                    <button type="button" on:click|stopPropagation={() => handleContainerStop(container.id)}
                            class="btn btn-danger btn-sm"
                            title="중지"
                            disabled={containerTransitions.has(container.id)}>
                      {containerTransitions.get(container.id) === 'stopping' ? '중지 중...' : '중지'}
                    </button>
                  {:else if container.status === 'pulling' || container.status === 'starting'}
                    <button type="button"
                            class="btn btn-secondary btn-sm"
                            title="진행 중"
                            disabled>
                      {container.status === 'pulling' ? '풀링 중...' : '시작 중...'}
                    </button>
                  {:else}
                    <button type="button" on:click|stopPropagation={() => handleContainerStart(container.id)}
                            class="btn btn-success btn-sm"
                            title="시작"
                            disabled={containerTransitions.has(container.id)}>
                      {containerTransitions.get(container.id) === 'starting' ? '시작 중...' : '시작'}
                    </button>
                  {/if}
                  <button type="button" on:click|stopPropagation={() => handleContainerDelete(container.id, container.name)}
                          class="btn btn-outline btn-sm" title="삭제"
                          disabled={container.status === 'running' || container.status === 'pulling' || container.status === 'starting' || containerTransitions.has(container.id)}>
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
                {#if isContainerTcp(container)}
                  <div class="tcp-urls">
                    {#each getTcpUrls(container) as url}
                      <span class="item-url tcp-url">{url}</span>
                    {/each}
                  </div>
                {:else}
                  <a href="{getContainerUrl(container)}" target="_blank" rel="noopener noreferrer"
                     class="item-url" on:click|stopPropagation>
                    {getContainerUrl(container)}
                  </a>
                {/if}
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
            <span>{formatRelativeTime(currentContainer.created_at, now)}</span>
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

<!-- Terminal Modal -->
{#if showTerminalModal && terminalContainerId}
  <div class="modal-overlay" on:click={() => showTerminalModal = false} transition:fade>
    <div class="modal-content terminal-modal" on:click|stopPropagation>
      <Terminal
        containerId={terminalContainerId}
        onClose={() => { showTerminalModal = false; terminalContainerId = null; }}
      />
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
    flex: 0 1 auto;
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
    font-size: 0.675rem;
    color: var(--primary);
    text-decoration: none;
    margin-top: 0.25rem;
    display: inline-block;
  }

  .item-url:hover {
    text-decoration: underline;
  }

  .tcp-urls {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    margin-top: 0.25rem;
  }

  .item-url.tcp-url {
    color: var(--gray-700);
    font-family: 'Courier New', monospace;
    font-size: 0.7rem;
    background: var(--gray-100);
    padding: 0.125rem 0.5rem;
    border-radius: 0.25rem;
    cursor: text;
    user-select: all;
  }

  .item-url.tcp-url:hover {
    background: var(--gray-200);
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

  /* 슬롯 배지 - Stopped 상태 (어둡게) */
  .status-badge.slot-badge.blue.stopped {
    background: #93c5fd;
    color: #1e3a8a;
  }

  .status-badge.slot-badge.green.stopped {
    background: #86efac;
    color: #065f46;
  }

  /* 슬롯 배지 - Running 상태 (밝게) */
  .status-badge.slot-badge.blue.running {
    background: #3b82f6;
    color: white;
  }

  .status-badge.slot-badge.green.running {
    background: #10b981;
    color: white;
  }

  /* 빌드 상태 - 연한 색상 (정보 전달용) */
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

  .status-badge.build-status.unknown {
    background: #f3f4f6;
    color: #4b5563;
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

  .status-badge.transitioning {
    background: #f59e0b;
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

  .modal-content.terminal-modal {
    max-width: 1000px;
    width: 90%;
    height: 70vh;
    max-height: 70vh;
    padding: 0;
    background: #1e1e1e;
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

  .btn-debug {
    background: #fbbf24;
    color: #78350f;
    font-size: 0.75rem;
    padding: 0.375rem 0.75rem;
  }

  .btn-debug:hover {
    background: #f59e0b;
  }

</style>

<ErrorModal
  bind:show={showErrorModal}
  title={errorModalTitle}
  message={errorModalMessage}
  details={errorModalDetails}
/>
