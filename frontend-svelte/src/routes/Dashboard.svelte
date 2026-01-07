<script>
  import { onMount } from 'svelte';
  import { link } from 'svelte-spa-router';

  const API_BASE = '/api';
  let projects = [];
  let loading = true;
  let error = null;

  onMount(async () => {
    await loadProjects();
  });

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
      if (!response.ok) throw new Error('빌드를 시작할 수 없습니다');

      const result = await response.json();
      alert(`빌드 #${result.build_id}가 시작되었습니다!`);
      setTimeout(() => loadProjects(), 1000);
    } catch (err) {
      alert('빌드 시작 실패: ' + err.message);
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
      if (!response.ok) throw new Error('프로젝트를 삭제할 수 없습니다');

      alert('프로젝트가 삭제되었습니다!');
      loadProjects();
    } catch (err) {
      alert('프로젝트 삭제 실패: ' + err.message);
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
</script>

<header>
  <div class="header-content">
    <h1>Easy CI/CD</h1>
    <div class="header-actions">
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
          <div class="project-header">
            <div>
              <div class="project-name">{project.name}</div>
              <a href="http://localhost:9999/{project.name}/" target="_blank" class="project-url">
                http://localhost:9999/{project.name}/
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
          </div>

          <div class="project-actions">
            <a href="/build/{project.id}" use:link class="btn btn-secondary btn-sm">빌드 내역</a>
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
