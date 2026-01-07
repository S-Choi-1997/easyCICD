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
      if (!response.ok) throw new Error('Failed to fetch project');
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
      if (!response.ok) throw new Error('Failed to fetch builds');
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
      if (!response.ok) throw new Error('Failed to trigger build');

      const result = await response.json();
      alert(`Build #${result.build_id} triggered successfully!`);
      setTimeout(() => loadBuilds(), 1000);
    } catch (err) {
      alert('Failed to trigger build: ' + err.message);
    }
  }

  async function showBuildDetail(buildId) {
    try {
      const response = await fetch(`${API_BASE}/builds/${buildId}`);
      if (!response.ok) throw new Error('Failed to fetch build details');
      selectedBuild = await response.json();
    } catch (err) {
      alert('Failed to load build details: ' + err.message);
    }
  }

  function formatTimeAgo(dateString) {
    const date = new Date(dateString);
    const now = new Date();
    const seconds = Math.floor((now - date) / 1000);

    if (seconds < 60) return 'just now';
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
    if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
    return `${Math.floor(seconds / 86400)}d ago`;
  }
</script>

<header>
  <div class="header-content">
    <h1>Lightweight CI/CD</h1>
    <div class="header-actions">
      <a href="/" use:link class="btn btn-secondary">← Back to Dashboard</a>
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
        <div><strong>Repo:</strong> {project.repo}</div>
        <div><strong>Branch:</strong> {project.branch}</div>
        <div><strong>Active Slot:</strong> {project.active_slot}</div>
        <div><strong>Path Filter:</strong> {project.path_filter}</div>
      </div>
    </div>
  {/if}

  <!-- Build List -->
  <div class="card">
    <div class="card-header">
      <h2 class="card-title">Build History</h2>
      <button on:click={triggerBuild} class="btn btn-primary btn-sm">Trigger Build</button>
    </div>

    {#if loading}
      <div class="loading">
        <div class="spinner"></div>
        <p>Loading builds...</p>
      </div>
    {:else if error}
      <div class="empty-state">
        <h3>Error loading builds</h3>
        <p>{error}</p>
      </div>
    {:else if builds.length === 0}
      <div class="empty-state">
        <p>No builds yet</p>
        <button on:click={triggerBuild} class="btn btn-primary mt-2">Trigger First Build</button>
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
        <h2 class="card-title">Build #{selectedBuild.build_number}</h2>
        <button on:click={() => selectedBuild = null} class="btn btn-secondary btn-sm">Close</button>
      </div>

      <div class="mb-2">
        <div class="flex flex-gap">
          <span class="status-badge status-{selectedBuild.status.toLowerCase()}">
            <span class="status-dot"></span>
            {selectedBuild.status}
          </span>
        </div>
        <div class="project-info mt-2">
          <div><strong>Commit:</strong> <span class="build-commit">{selectedBuild.commit_hash}</span></div>
          {#if selectedBuild.commit_message}
            <div><strong>Message:</strong> {selectedBuild.commit_message}</div>
          {/if}
          {#if selectedBuild.author}
            <div><strong>Author:</strong> {selectedBuild.author}</div>
          {/if}
          <div><strong>Created:</strong> {new Date(selectedBuild.created_at).toLocaleString()}</div>
          {#if selectedBuild.finished_at}
            <div><strong>Finished:</strong> {new Date(selectedBuild.finished_at).toLocaleString()}</div>
          {/if}
        </div>
      </div>

      <h3>Build Log</h3>
      <div class="log-viewer">
        <div class="log-line">Log file: {selectedBuild.log_path}</div>
        <div class="log-line text-muted">Log streaming not yet implemented in this version</div>
      </div>
    </div>
  {/if}
</div>
