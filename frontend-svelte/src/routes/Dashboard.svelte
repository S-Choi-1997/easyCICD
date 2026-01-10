<script>
  import { onMount } from 'svelte';
  import { link } from 'svelte-spa-router';
  import { projects, projectsLoading, projectsError, loadProjects, triggerBuild, deleteProject } from '../stores/projects';
  import { formatRelativeTime } from '../utils/dateFormatter';
  import Skeleton from '../components/Skeleton.svelte';
  import { fade } from 'svelte/transition';

  const API_BASE = '/api';
  let domain = null;

  onMount(async () => {
    await loadDomain();
    await loadProjects();
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

  async function handleTriggerBuild(projectId) {
    try {
      await triggerBuild(projectId);
    } catch (error) {
      alert('빌드를 시작할 수 없습니다: ' + error.message);
    }
  }

  async function handleDeleteProject(projectId, projectName) {
    if (!confirm(`"${projectName}" 프로젝트를 정말 삭제하시겠습니까?`)) {
      return;
    }

    try {
      await deleteProject(projectId);
    } catch (error) {
      alert('프로젝트를 삭제할 수 없습니다: ' + error.message);
    }
  }

  async function handleStartContainers(projectId, event) {
    event?.stopPropagation();
    try {
      const response = await fetch(`${API_BASE}/projects/${projectId}/containers/start`, {
        method: 'POST'
      });
      if (response.ok) {
        setTimeout(() => loadProjects(), 1000);
      } else {
        alert('컨테이너를 시작할 수 없습니다');
      }
    } catch (error) {
      alert('컨테이너를 시작할 수 없습니다: ' + error.message);
    }
  }

  async function handleStopContainers(projectId, event) {
    event?.stopPropagation();
    try {
      const response = await fetch(`${API_BASE}/projects/${projectId}/containers/stop`, {
        method: 'POST'
      });
      if (response.ok) {
        setTimeout(() => loadProjects(), 1000);
      } else {
        alert('컨테이너를 중지할 수 없습니다');
      }
    } catch (error) {
      alert('컨테이너를 중지할 수 없습니다: ' + error.message);
    }
  }

  async function handleRestartContainers(projectId, event) {
    event?.stopPropagation();
    try {
      const response = await fetch(`${API_BASE}/projects/${projectId}/containers/restart`, {
        method: 'POST'
      });
      if (response.ok) {
        setTimeout(() => loadProjects(), 1000);
      } else {
        alert('컨테이너를 재시작할 수 없습니다');
      }
    } catch (error) {
      alert('컨테이너를 재시작할 수 없습니다: ' + error.message);
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
      <span class="project-count">{$projects.length}개 프로젝트</span>
    </div>

    {#if $projectsLoading}
      <Skeleton type="project-card" count={3} />
    {:else if $projectsError}
      <div class="empty-state" transition:fade>
        <h3>프로젝트 로딩 오류</h3>
        <p>{$projectsError}</p>
        <button on:click={loadProjects} class="btn btn-primary mt-2">다시 시도</button>
      </div>
    {:else if $projects.length === 0}
      <div class="empty-state" transition:fade>
        <h3>프로젝트가 없습니다</h3>
        <p>첫 번째 프로젝트를 만들어보세요</p>
        <a href="/setup" use:link class="btn btn-primary mt-2">+ 새 프로젝트</a>
      </div>
    {:else}
      <div transition:fade>
        {#each $projects as project (project.id)}
          <div class="project-card" transition:fade>
            <div on:click={() => window.location.hash = `/build/${project.id}`}
                 on:keydown={(e) => e.key === 'Enter' && (window.location.hash = `/build/${project.id}`)}
                 role="button"
                 tabindex="0"
                 style="cursor: pointer;">
              <div class="project-header">
                <div>
                  <div class="project-name">{project.name}</div>
                  <a
                    href="{getProjectUrl(project.name)}"
                    target="_blank"
                    rel="noopener noreferrer"
                    class="project-url-link"
                    on:click|stopPropagation>
                    {getProjectUrl(project.name)}
                  </a>
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
                {#if project.updated_at}
                  <div><strong>마지막 업데이트:</strong> {formatRelativeTime(project.updated_at)}</div>
                {/if}
              </div>
            </div>

            <div class="project-actions">
              <a href="{getProjectUrl(project.name)}" target="_blank" rel="noopener noreferrer" class="btn btn-secondary btn-sm">
                🔗 열기
              </a>
              <button on:click|stopPropagation={() => handleTriggerBuild(project.id)} class="btn btn-primary btn-sm">
                ▶️ 빌드 시작
              </button>
              <div class="container-controls">
                <button
                  on:click={(e) => handleStartContainers(project.id, e)}
                  class="btn btn-success btn-sm"
                  disabled={!!(project.blue_container_id || project.green_container_id)}
                  title="컨테이너 시작">
                  ▶ 시작
                </button>
                <button
                  on:click={(e) => handleRestartContainers(project.id, e)}
                  class="btn btn-warning btn-sm"
                  disabled={!(project.blue_container_id || project.green_container_id)}
                  title="컨테이너 재시작">
                  🔄 재시작
                </button>
                <button
                  on:click={(e) => handleStopContainers(project.id, e)}
                  class="btn btn-danger btn-sm"
                  disabled={!(project.blue_container_id || project.green_container_id)}
                  title="컨테이너 중지">
                  ■ 중지
                </button>
              </div>
              <button on:click|stopPropagation={() => handleDeleteProject(project.id, project.name)} class="btn btn-danger btn-sm">
                🗑️ 삭제
              </button>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .project-count {
    color: var(--gray-600);
    font-size: 0.875rem;
  }

  .mt-2 {
    margin-top: 0.5rem;
  }

  .container-controls {
    display: flex;
    gap: 0.25rem;
    border: 1px solid var(--gray-300);
    border-radius: 0.375rem;
    padding: 0.125rem;
    background: var(--gray-50);
  }

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

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn:disabled:hover {
    transform: none;
    box-shadow: none;
  }

  .project-url-link {
    font-size: 0.875rem;
    color: var(--primary);
    text-decoration: none;
    padding: 0.125rem 0;
    display: inline-block;
  }

  .project-url-link:hover {
    text-decoration: underline;
  }
</style>
