<script>
  import { onMount, onDestroy } from 'svelte';
  import { link } from 'svelte-spa-router';

  const API_BASE = '/api';
  let projects = [];
  let loading = true;
  let error = null;
  let domain = null;
  let ws = null;

  onMount(async () => {
    await loadDomain();
    await loadProjects();
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

    ws.onopen = () => {
      console.log('WebSocket connected');
    };

    ws.onmessage = (event) => {
      const data = JSON.parse(event.data);
      handleWebSocketMessage(data);
    };

    ws.onerror = (err) => {
      console.error('WebSocket error:', err);
    };

    ws.onclose = () => {
      console.log('WebSocket closed, reconnecting...');
      setTimeout(connectWebSocket, 3000);
    };
  }

  function handleWebSocketMessage(data) {
    if (data.type === 'BuildStatus') {
      // Reload projects when any build status changes
      loadProjects();
    }
  }

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

  async function loadProjects() {
    loading = true;
    error = null;

    try {
      const response = await fetch(`${API_BASE}/projects`);
      if (!response.ok) throw new Error('프로젝트 목록을 가져올 수 없습니다');
      projects = await response.json();
    } catch (err) {
      error = err.message;
    } finally {
      loading = false;
    }
  }

  async function triggerBuild(projectId) {
    try {
      const response = await fetch(`${API_BASE}/projects/${projectId}/builds`, {
        method: 'POST'
      });
      if (response.ok) {
        setTimeout(() => loadProjects(), 1000);
      }
    } catch (err) {
      console.error(err);
    }
  }

  async function deleteProject(projectId, projectName) {
    if (!confirm(`"${projectName}" 프로젝트를 정말 삭제하시겠습니까?`)) {
      return;
    }

    try {
      const response = await fetch(`${API_BASE}/projects/${projectId}`, {
        method: 'DELETE'
      });
      if (response.ok) {
        loadProjects();
      }
    } catch (err) {
      console.error(err);
    }
  }

  function getStatusClass(project) {
    if (project.blue_container_id || project.green_container_id) {
      return 'running';
    }
    return 'queued';
  }

  function getStatusText(project) {
    if (project.blue_container_id || project.green_container_id) {
      return '실행 중';
    }
    return '배포 안됨';
  }

  function getProjectUrl(projectName) {
    const baseDomain = domain || 'albl.cloud';
    const protocol = domain && !domain.includes('localhost') ? 'https' : 'http';

    // Use subdomain-based routing: projectname-app.albl.cloud
    return `${protocol}://${projectName}-app.${baseDomain}/`;
  }
</script>

<header>
  <div class="header-content">
    <a href="/" use:link style="text-decoration: none; color: inherit; cursor: pointer;">
      <h1>Easy CI/CD</h1>
    </a>
    <div class="header-actions">
      <a href="/settings" use:link class="btn btn-secondary">⚙️ 설정</a>
      <a href="/setup" use:link class="btn btn-primary">+ 새 프로젝트</a>
    </div>
  </div>
</header>

<div class="container">
  <div class="card">
    <div class="card-header">
      <h2 class="card-title">프로젝트 목록</h2>
    </div>

    {#if loading}
      <div class="loading">
        <div class="spinner"></div>
        <p>프로젝트 불러오는 중...</p>
      </div>
    {:else if error}
      <div class="empty-state">
        <h3>프로젝트 로딩 오류</h3>
        <p>{error}</p>
      </div>
    {:else if projects.length === 0}
      <div class="empty-state">
        <h3>프로젝트가 없습니다</h3>
        <p>첫 번째 프로젝트를 만들어보세요</p>
        <a href="/setup" use:link class="btn btn-primary mt-2">+ 새 프로젝트</a>
      </div>
    {:else}
      {#each projects as project}
        <div class="project-card">
          <div on:click={() => window.location.hash = `/build/${project.id}`} style="cursor: pointer;">
            <div class="project-header">
              <div>
                <div class="project-name">{project.name}</div>
                <div class="project-url">
                  {getProjectUrl(project.name)}
                </div>
              </div>
              <span class="status-badge status-{getStatusClass(project)}">
                <span class="status-dot"></span>
                {getStatusText(project)}
              </span>
            </div>

            <div class="project-info">
              <div><strong>레포지토리:</strong> {project.repo}</div>
              <div><strong>브랜치:</strong> {project.branch}</div>
              <div><strong>활성 슬롯:</strong> {project.active_slot}</div>
            </div>
          </div>

          <div class="project-actions">
            <a href="{getProjectUrl(project.name)}" target="_blank" class="btn btn-secondary btn-sm">🔗 열기</a>
            <button on:click={() => triggerBuild(project.id)} class="btn btn-primary btn-sm">
              빌드 시작
            </button>
            <button on:click={() => deleteProject(project.id, project.name)} class="btn btn-danger btn-sm">
              삭제
            </button>
          </div>
        </div>
      {/each}
    {/if}
  </div>
</div>
