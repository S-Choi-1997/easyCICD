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
      if (!response.ok) throw new Error('Failed to fetch projects');
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
      if (!response.ok) throw new Error('Failed to trigger build');

      const result = await response.json();
      alert(`Build #${result.build_id} triggered successfully!`);
      setTimeout(() => loadProjects(), 1000);
    } catch (err) {
      alert('Failed to trigger build: ' + err.message);
    }
  }

  async function deleteProject(projectId, projectName) {
    if (!confirm(`Are you sure you want to delete project "${projectName}"?`)) {
      return;
    }

    try {
      const response = await fetch(`${API_BASE}/projects/${projectId}`, {
        method: 'DELETE'
      });
      if (!response.ok) throw new Error('Failed to delete project');

      alert('Project deleted successfully!');
      loadProjects();
    } catch (err) {
      alert('Failed to delete project: ' + err.message);
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
      return 'Running';
    }
    return 'Not Deployed';
  }
</script>

<header>
  <div class="header-content">
    <h1>Lightweight CI/CD</h1>
    <div class="header-actions">
      <a href="/setup" use:link class="btn btn-primary">+ New Project</a>
    </div>
  </div>
</header>

<div class="container">
  <div class="card">
    <div class="card-header">
      <h2 class="card-title">Projects</h2>
    </div>

    {#if loading}
      <div class="loading">
        <div class="spinner"></div>
        <p>Loading projects...</p>
      </div>
    {:else if error}
      <div class="empty-state">
        <h3>Error loading projects</h3>
        <p>{error}</p>
      </div>
    {:else if projects.length === 0}
      <div class="empty-state">
        <h3>No projects yet</h3>
        <p>Create your first project to get started</p>
        <a href="/setup" use:link class="btn btn-primary mt-2">+ New Project</a>
      </div>
    {:else}
      {#each projects as project}
        <div class="project-card">
          <div class="project-header">
            <div>
              <div class="project-name">{project.name}</div>
              <a href="http://localhost:8080/{project.name}/" target="_blank" class="project-url">
                http://localhost:8080/{project.name}/
              </a>
            </div>
            <span class="status-badge status-{getStatusClass(project)}">
              <span class="status-dot"></span>
              {getStatusText(project)}
            </span>
          </div>

          <div class="project-info">
            <div><strong>Repo:</strong> {project.repo}</div>
            <div><strong>Branch:</strong> {project.branch}</div>
            <div><strong>Active Slot:</strong> {project.active_slot}</div>
          </div>

          <div class="project-actions">
            <a href="/build/{project.id}" use:link class="btn btn-secondary btn-sm">View Builds</a>
            <button on:click={() => triggerBuild(project.id)} class="btn btn-primary btn-sm">
              Trigger Build
            </button>
            <button on:click={() => deleteProject(project.id, project.name)} class="btn btn-danger btn-sm">
              Delete
            </button>
          </div>
        </div>
      {/each}
    {/if}
  </div>
</div>
