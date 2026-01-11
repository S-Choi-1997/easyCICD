<script>
  import { onMount } from 'svelte';
  import { link } from 'svelte-spa-router';
  import { projects, projectsLoading, projectsError, loadProjects, triggerBuild, deleteProject } from '../stores/projects';
  import { formatRelativeTime } from '../utils/dateFormatter';
  import Skeleton from '../components/Skeleton.svelte';
  import { fade } from 'svelte/transition';

  const API_BASE = '/api';
  let domain = null;
  let containers = [];
  let containersLoading = true;
  let showAddMenu = false;

  onMount(async () => {
    await Promise.all([loadDomain(), loadProjects(), loadContainers()]);
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
    try {
      const response = await fetch(`${API_BASE}/containers/${id}/start`, { method: 'POST' });
      if (response.ok) await loadContainers();
      else alert('컨테이너를 시작할 수 없습니다');
    } catch (error) {
      alert('컨테이너를 시작할 수 없습니다: ' + error.message);
    }
  }

  async function handleContainerStop(id) {
    try {
      const response = await fetch(`${API_BASE}/containers/${id}/stop`, { method: 'POST' });
      if (response.ok) await loadContainers();
      else alert('컨테이너를 중지할 수 없습니다');
    } catch (error) {
      alert('컨테이너를 중지할 수 없습니다: ' + error.message);
    }
  }

  async function handleContainerDelete(id, name) {
    if (!confirm(`"${name}" 컨테이너를 삭제하시겠습니까?`)) return;
    try {
      const response = await fetch(`${API_BASE}/containers/${id}`, { method: 'DELETE' });
      if (response.ok) await loadContainers();
      else alert('컨테이너를 삭제할 수 없습니다');
    } catch (error) {
      alert('컨테이너를 삭제할 수 없습니다: ' + error.message);
    }
  }

  function isProjectRunning(project) {
    return !!(project.blue_container_id || project.green_container_id);
  }

  function getProjectUrl(projectName) {
    const baseDomain = domain || 'albl.cloud';
    const protocol = domain && !domain.includes('localhost') ? 'https' : 'http';
    return `${protocol}://${projectName}-app.${baseDomain}/`;
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
      <a href="/settings" use:link class="btn btn-secondary">⚙️ 설정</a>
      <div class="dropdown">
        <button class="btn btn-primary" on:click={() => showAddMenu = !showAddMenu}>
          + 추가
        </button>
        {#if showAddMenu}
          <div class="dropdown-menu" on:mouseleave={() => showAddMenu = false}>
            <a href="/setup" use:link class="dropdown-item" on:click={() => showAddMenu = false}>
              🚀 프로젝트
            </a>
            <a href="/containers/new" use:link class="dropdown-item" on:click={() => showAddMenu = false}>
              📦 컨테이너
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
          <a href="/setup" use:link class="btn btn-primary">🚀 프로젝트 추가</a>
          <a href="/containers/new" use:link class="btn btn-secondary">📦 컨테이너 추가</a>
        </div>
      </div>
    {:else}
      <div class="items-list" transition:fade>
        <!-- 프로젝트 (빌드 가능한 컨테이너) -->
        {#each $projects as project (project.id)}
          <div class="item-card" transition:fade>
            <div class="item-main" on:click={() => window.location.hash = `/build/${project.id}`}
                 on:keydown={(e) => e.key === 'Enter' && (window.location.hash = `/build/${project.id}`)}
                 role="button" tabindex="0">
              <div class="item-header">
                <div class="item-info">
                  <span class="item-type project">프로젝트</span>
                  <span class="item-name">{project.name}</span>
                </div>
                <span class="status-badge {isProjectRunning(project) ? 'running' : 'stopped'}">
                  {isProjectRunning(project) ? '실행 중' : '중지'}
                </span>
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
            <div class="item-actions">
              <button on:click|stopPropagation={() => handleTriggerBuild(project.id)} class="btn btn-primary btn-sm" title="빌드">
                🔨 빌드
              </button>
              {#if isProjectRunning(project)}
                <button on:click|stopPropagation={() => handleProjectRestart(project.id)} class="btn btn-warning btn-sm" title="재시작">
                  🔄
                </button>
                <button on:click|stopPropagation={() => handleProjectStop(project.id)} class="btn btn-danger btn-sm" title="중지">
                  ■
                </button>
              {:else}
                <button on:click|stopPropagation={() => handleProjectStart(project.id)} class="btn btn-success btn-sm" title="시작">
                  ▶
                </button>
              {/if}
              <button on:click|stopPropagation={() => handleDeleteProject(project.id, project.name)} class="btn btn-outline btn-sm" title="삭제">
                🗑️
              </button>
            </div>
          </div>
        {/each}

        <!-- 독립 컨테이너 -->
        {#each containers as container (container.id)}
          <div class="item-card" transition:fade>
            <div class="item-main">
              <div class="item-header">
                <div class="item-info">
                  <span class="item-type container">컨테이너</span>
                  <span class="item-name">{container.name}</span>
                </div>
                <span class="status-badge {container.status === 'Running' ? 'running' : 'stopped'}">
                  {container.status === 'Running' ? '실행 중' : '중지'}
                </span>
              </div>
              <div class="item-details">
                <span>{container.image}</span>
                {#if container.port}
                  <span>·</span>
                  <span>포트 {container.port}</span>
                {/if}
              </div>
            </div>
            <div class="item-actions">
              {#if container.status === 'Running'}
                <button on:click={() => handleContainerStop(container.id)} class="btn btn-danger btn-sm" title="중지">
                  ■
                </button>
              {:else}
                <button on:click={() => handleContainerStart(container.id)} class="btn btn-success btn-sm" title="시작">
                  ▶
                </button>
              {/if}
              <button on:click={() => handleContainerDelete(container.id, container.name)}
                      class="btn btn-outline btn-sm" title="삭제"
                      disabled={container.status === 'Running'}>
                🗑️
              </button>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

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
    justify-content: space-between;
    align-items: center;
    padding: 1rem 1.5rem;
    border-bottom: 1px solid var(--gray-200);
    gap: 1rem;
  }

  .item-card:last-child {
    border-bottom: none;
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
    margin-bottom: 0.25rem;
  }

  .item-info {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    min-width: 0;
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
    gap: 0.375rem;
    flex-shrink: 0;
  }

  .status-badge {
    font-size: 0.75rem;
    font-weight: 500;
    padding: 0.25rem 0.5rem;
    border-radius: 9999px;
    flex-shrink: 0;
  }

  .status-badge.running {
    background: #dcfce7;
    color: #166534;
  }

  .status-badge.stopped {
    background: #f3f4f6;
    color: #6b7280;
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

  .btn-warning {
    background: #f59e0b;
    color: white;
    border: none;
  }

  .btn-warning:hover:not(:disabled) {
    background: #d97706;
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
</style>
