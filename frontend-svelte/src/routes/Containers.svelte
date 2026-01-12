<script>
  import { onMount } from 'svelte';
  import { link } from 'svelte-spa-router';
  import { fade } from 'svelte/transition';
  import Skeleton from '../components/Skeleton.svelte';

  const API_BASE = '/api';

  let containers = [];
  let loading = true;
  let error = null;
  let showCreateModal = false;

  // Create form state
  let newContainer = {
    name: '',
    image: '',
    command: '',
    env_vars: '',
    volumes: ''
  };
  let creating = false;

  onMount(async () => {
    await loadContainers();
  });

  async function loadContainers() {
    loading = true;
    error = null;
    try {
      const response = await fetch(`${API_BASE}/containers`);
      if (!response.ok) throw new Error('컨테이너 목록을 불러올 수 없습니다');
      containers = await response.json();
    } catch (e) {
      error = e.message;
    } finally {
      loading = false;
    }
  }

  async function createContainer() {
    if (!newContainer.name.trim() || !newContainer.image.trim()) {
      alert('이름과 이미지는 필수입니다');
      return;
    }

    creating = true;
    try {
      // Parse env_vars and volumes
      let envVars = {};
      if (newContainer.env_vars.trim()) {
        newContainer.env_vars.split('\n').forEach(line => {
          const [key, ...valueParts] = line.split('=');
          if (key && valueParts.length > 0) {
            envVars[key.trim()] = valueParts.join('=').trim();
          }
        });
      }

      let volumes = [];
      if (newContainer.volumes.trim()) {
        volumes = newContainer.volumes.split('\n').map(v => v.trim()).filter(v => v);
      }

      const response = await fetch(`${API_BASE}/containers`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: newContainer.name.trim(),
          image: newContainer.image.trim(),
          command: newContainer.command.trim() || null,
          env_vars: Object.keys(envVars).length > 0 ? envVars : null,
          volumes: volumes.length > 0 ? volumes : null
        })
      });

      if (!response.ok) {
        const data = await response.json();
        throw new Error(data.error || '컨테이너를 생성할 수 없습니다');
      }

      // Reset form and close modal
      newContainer = { name: '', image: '', command: '', env_vars: '', volumes: '' };
      showCreateModal = false;
      await loadContainers();
    } catch (e) {
      alert(e.message);
    } finally {
      creating = false;
    }
  }

  async function startContainer(id) {
    try {
      const response = await fetch(`${API_BASE}/containers/${id}/start`, { method: 'POST' });
      if (!response.ok) {
        const data = await response.json();
        throw new Error(data.error || '컨테이너를 시작할 수 없습니다');
      }
      await loadContainers();
    } catch (e) {
      alert(e.message);
    }
  }

  async function stopContainer(id) {
    try {
      const response = await fetch(`${API_BASE}/containers/${id}/stop`, { method: 'POST' });
      if (!response.ok) {
        const data = await response.json();
        throw new Error(data.error || '컨테이너를 중지할 수 없습니다');
      }
      await loadContainers();
    } catch (e) {
      alert(e.message);
    }
  }

  async function deleteContainer(id, name) {
    if (!confirm(`"${name}" 컨테이너를 삭제하시겠습니까?`)) return;

    try {
      const response = await fetch(`${API_BASE}/containers/${id}`, { method: 'DELETE' });
      if (!response.ok) {
        const data = await response.json();
        throw new Error(data.error || '컨테이너를 삭제할 수 없습니다');
      }
      await loadContainers();
    } catch (e) {
      alert(e.message);
    }
  }

  function getStatusClass(status) {
    switch (status) {
      case 'running': return 'running';
      case 'stopped': return 'stopped';
      case 'created': return 'created';
      case 'error': return 'error';
      default: return 'unknown';
    }
  }

  function getStatusText(status) {
    switch (status) {
      case 'running': return '실행 중';
      case 'stopped': return '중지됨';
      case 'created': return '생성됨';
      case 'error': return '오류';
      default: return '알 수 없음';
    }
  }
</script>

<header>
  <div class="header-content">
    <a href="/" use:link style="text-decoration: none; color: inherit; cursor: pointer;">
      <h1>Easy CI/CD</h1>
    </a>
    <div class="header-actions">
      <a href="/" use:link class="btn btn-secondary">← 대시보드</a>
      <button on:click={() => showCreateModal = true} class="btn btn-primary">+ 새 컨테이너</button>
    </div>
  </div>
</header>

<div class="container">
  <div class="card">
    <div class="card-header">
      <h2 class="card-title">독립 컨테이너</h2>
      <span class="container-count">{containers.length}개 컨테이너</span>
    </div>

    {#if loading}
      <Skeleton type="project-card" count={3} />
    {:else if error}
      <div class="empty-state" transition:fade>
        <h3>컨테이너 로딩 오류</h3>
        <p>{error}</p>
        <button on:click={loadContainers} class="btn btn-primary mt-2">다시 시도</button>
      </div>
    {:else if containers.length === 0}
      <div class="empty-state" transition:fade>
        <h3>컨테이너가 없습니다</h3>
        <p>데이터베이스, 캐시 등의 독립 컨테이너를 생성하세요</p>
        <button on:click={() => showCreateModal = true} class="btn btn-primary mt-2">+ 새 컨테이너</button>
      </div>
    {:else}
      <div transition:fade>
        {#each containers as container (container.id)}
          <div class="container-card" transition:fade>
            <div class="container-header">
              <div>
                <div class="container-name">{container.name}</div>
                <div class="container-image">{container.image}</div>
              </div>
              <span class="status-badge status-{getStatusClass(container.status)}">
                <span class="status-dot"></span>
                {getStatusText(container.status)}
              </span>
            </div>

            <div class="container-info">
              {#if container.port}
                <div><strong>포트:</strong> {container.port}</div>
              {/if}
              {#if container.container_id}
                <div><strong>컨테이너 ID:</strong> {container.container_id.substring(0, 12)}</div>
              {/if}
              {#if container.command}
                <div><strong>명령:</strong> <code>{container.command}</code></div>
              {/if}
            </div>

            <div class="container-actions">
              {#if container.status === 'running'}
                <button on:click={() => stopContainer(container.id)} class="btn btn-warning btn-sm">
                  ■ 중지
                </button>
              {:else}
                <button on:click={() => startContainer(container.id)} class="btn btn-success btn-sm">
                  ▶ 시작
                </button>
              {/if}
              <button
                on:click={() => deleteContainer(container.id, container.name)}
                class="btn btn-danger btn-sm"
                disabled={container.status === 'running'}
                title={container.status === 'running' ? '실행 중인 컨테이너는 삭제할 수 없습니다' : ''}
              >
                🗑️ 삭제
              </button>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<!-- Create Modal -->
{#if showCreateModal}
  <div class="modal-overlay" on:click={() => showCreateModal = false} on:keydown={(e) => e.key === 'Escape' && (showCreateModal = false)}>
    <div class="modal" on:click|stopPropagation on:keydown|stopPropagation role="dialog" aria-modal="true">
      <div class="modal-header">
        <h3>새 컨테이너 생성</h3>
        <button class="modal-close" on:click={() => showCreateModal = false}>&times;</button>
      </div>
      <div class="modal-body">
        <div class="form-group">
          <label for="name">이름 *</label>
          <input type="text" id="name" class="form-input" bind:value={newContainer.name} placeholder="my-redis" />
        </div>

        <div class="form-group">
          <label for="image">이미지 *</label>
          <input type="text" id="image" class="form-input" bind:value={newContainer.image} placeholder="redis:alpine" />
        </div>

        <div class="form-group">
          <label for="command">명령 (선택)</label>
          <input type="text" id="command" class="form-input" bind:value={newContainer.command} placeholder="redis-server --appendonly yes" />
        </div>

        <div class="form-group">
          <label for="env_vars">환경 변수 (선택, 줄바꿈으로 구분)</label>
          <textarea id="env_vars" class="form-input" bind:value={newContainer.env_vars} rows="3" placeholder="MYSQL_ROOT_PASSWORD=secret&#10;MYSQL_DATABASE=mydb"></textarea>
        </div>

        <div class="form-group">
          <label for="volumes">볼륨 (선택, 줄바꿈으로 구분)</label>
          <textarea id="volumes" class="form-input" bind:value={newContainer.volumes} rows="2" placeholder="/data/mysql:/var/lib/mysql"></textarea>
        </div>
      </div>
      <div class="modal-footer">
        <button class="btn btn-secondary" on:click={() => showCreateModal = false}>취소</button>
        <button class="btn btn-primary" on:click={createContainer} disabled={creating}>
          {creating ? '생성 중...' : '생성'}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 2rem 1rem;
  }

  .card {
    background: white;
    border-radius: 0.5rem;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
    overflow: hidden;
  }

  .card-header {
    padding: 1.5rem;
    border-bottom: 1px solid #e5e7eb;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .card-title {
    font-size: 1.5rem;
    font-weight: 700;
    margin: 0;
    color: #111827;
  }

  .container-count {
    color: #6b7280;
    font-size: 0.875rem;
  }

  .container-card {
    padding: 1.5rem;
    border-bottom: 1px solid #e5e7eb;
  }

  .container-card:last-child {
    border-bottom: none;
  }

  .container-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 1rem;
  }

  .container-name {
    font-size: 1.125rem;
    font-weight: 600;
    color: #111827;
  }

  .container-image {
    font-size: 0.875rem;
    color: #6b7280;
    font-family: monospace;
  }

  .container-info {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
    font-size: 0.875rem;
    color: #4b5563;
    margin-bottom: 1rem;
  }

  .container-info code {
    background: #f3f4f6;
    padding: 0.125rem 0.375rem;
    border-radius: 0.25rem;
    font-family: monospace;
    font-size: 0.75rem;
  }

  .container-actions {
    display: flex;
    gap: 0.5rem;
  }

  .status-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.375rem;
    padding: 0.25rem 0.75rem;
    border-radius: 9999px;
    font-size: 0.75rem;
    font-weight: 500;
  }

  .status-dot {
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 50%;
  }

  .status-running {
    background: #d1fae5;
    color: #065f46;
  }

  .status-running .status-dot {
    background: #10b981;
  }

  .status-stopped {
    background: #fee2e2;
    color: #991b1b;
  }

  .status-stopped .status-dot {
    background: #ef4444;
  }

  .status-created {
    background: #e0e7ff;
    color: #3730a3;
  }

  .status-created .status-dot {
    background: #6366f1;
  }

  .status-error {
    background: #fef3c7;
    color: #92400e;
  }

  .status-error .status-dot {
    background: #f59e0b;
  }

  .empty-state {
    text-align: center;
    padding: 3rem;
    color: #6b7280;
  }

  .empty-state h3 {
    font-size: 1.125rem;
    font-weight: 600;
    margin-bottom: 0.5rem;
    color: #111827;
  }

  .mt-2 {
    margin-top: 0.5rem;
  }

  /* Buttons */
  .btn {
    padding: 0.5rem 1rem;
    border: none;
    border-radius: 0.375rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s;
    text-decoration: none;
    display: inline-block;
  }

  .btn-sm {
    padding: 0.25rem 0.5rem;
    font-size: 0.875rem;
  }

  .btn-primary {
    background: #3b82f6;
    color: white;
  }

  .btn-primary:hover:not(:disabled) {
    background: #2563eb;
  }

  .btn-secondary {
    background: #6b7280;
    color: white;
  }

  .btn-secondary:hover {
    background: #4b5563;
  }

  .btn-success {
    background: #10b981;
    color: white;
  }

  .btn-success:hover:not(:disabled) {
    background: #059669;
  }

  .btn-warning {
    background: #f59e0b;
    color: white;
  }

  .btn-warning:hover:not(:disabled) {
    background: #d97706;
  }

  .btn-danger {
    background: #ef4444;
    color: white;
  }

  .btn-danger:hover:not(:disabled) {
    background: #dc2626;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Header */
  header {
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    color: white;
    padding: 1.5rem 2rem;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
  }

  .header-content {
    max-width: 1200px;
    margin: 0 auto;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  header h1 {
    margin: 0;
    font-size: 1.875rem;
    font-weight: 700;
  }

  .header-actions {
    display: flex;
    gap: 0.75rem;
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
    justify-content: center;
    align-items: center;
    z-index: 1000;
  }

  .modal {
    background: white;
    border-radius: 0.5rem;
    width: 90%;
    max-width: 500px;
    max-height: 90vh;
    overflow-y: auto;
  }

  .modal-header {
    padding: 1rem 1.5rem;
    border-bottom: 1px solid #e5e7eb;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .modal-header h3 {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 600;
  }

  .modal-close {
    background: none;
    border: none;
    font-size: 1.5rem;
    cursor: pointer;
    color: #6b7280;
    line-height: 1;
  }

  .modal-close:hover {
    color: #111827;
  }

  .modal-body {
    padding: 1.5rem;
  }

  .modal-footer {
    padding: 1rem 1.5rem;
    border-top: 1px solid #e5e7eb;
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }

  .form-group {
    margin-bottom: 1rem;
  }

  .form-group label {
    display: block;
    font-weight: 500;
    margin-bottom: 0.5rem;
    color: #374151;
  }

  .form-input {
    width: 100%;
    padding: 0.5rem 0.75rem;
    border: 1px solid #d1d5db;
    border-radius: 0.375rem;
    font-size: 0.875rem;
  }

  .form-input:focus {
    outline: none;
    border-color: #3b82f6;
    box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.1);
  }

  textarea.form-input {
    resize: vertical;
    font-family: monospace;
  }
</style>
