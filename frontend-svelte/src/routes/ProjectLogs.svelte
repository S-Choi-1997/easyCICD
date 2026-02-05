<script>
  import { onMount, onDestroy } from 'svelte';

  export let params = {};
  const projectId = params.id;
  const API_BASE = '/api';

  let project = null;
  let logs = [];
  let ws = null;
  let connected = false;
  let loading = true;

  onMount(async () => {
    await loadProject();
    connectLogs();
  });

  onDestroy(() => {
    disconnectLogs();
  });

  async function loadProject() {
    try {
      const response = await fetch(`${API_BASE}/projects/${projectId}`);
      if (response.ok) {
        project = await response.json();
      }
    } catch (error) {
      console.error('프로젝트 로드 실패:', error);
    } finally {
      loading = false;
    }
  }

  function connectLogs() {
    if (ws) {
      disconnectLogs();
    }

    logs = ['연결 중...'];

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/api/projects/${projectId}/runtime-logs?tail=all`;

    ws = new WebSocket(wsUrl);

    ws.onopen = () => {
      connected = true;
      logs = ['연결되었습니다. 로그를 수신합니다...'];
    };

    ws.onmessage = (event) => {
      const logLine = event.data;
      logs = [...logs, logLine];

      // 자동 스크롤
      setTimeout(() => {
        const container = document.getElementById('fullscreen-logs');
        if (container) {
          container.scrollTop = container.scrollHeight;
        }
      }, 10);
    };

    ws.onerror = (error) => {
      console.error('WebSocket 에러:', error);
      connected = false;
    };

    ws.onclose = () => {
      connected = false;
      logs = [...logs, '연결이 종료되었습니다.'];
    };
  }

  function disconnectLogs() {
    if (ws) {
      ws.close();
      ws = null;
      connected = false;
    }
  }
</script>

<div class="fullscreen-logs-page">
  <div class="logs-header">
    <div class="logs-title">
      <h2>{project ? project.name : '...'} - 런타임 로그</h2>
      {#if connected}
        <span class="status-badge status-connected">연결됨</span>
      {:else}
        <span class="status-badge status-disconnected">연결 안됨</span>
      {/if}
      <span class="log-count">{logs.length}줄</span>
    </div>
    <div class="logs-actions">
      {#if !connected}
        <button on:click={connectLogs} class="btn btn-primary btn-sm">연결</button>
      {:else}
        <button on:click={disconnectLogs} class="btn btn-danger btn-sm">연결 종료</button>
      {/if}
      <button on:click={() => logs = []} class="btn btn-secondary btn-sm">지우기</button>
    </div>
  </div>

  <div id="fullscreen-logs" class="logs-container">
    {#each logs as log, i}
      <div class="log-line"><span class="line-num">{i + 1}</span>{log}</div>
    {:else}
      <div class="log-empty">로그가 없습니다.</div>
    {/each}
  </div>
</div>

<style>
  .fullscreen-logs-page {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: #1e1e1e;
    color: #d4d4d4;
  }

  .logs-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 1rem;
    background: #2d2d2d;
    border-bottom: 1px solid #404040;
    flex-shrink: 0;
  }

  .logs-title {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .logs-title h2 {
    font-size: 1rem;
    font-weight: 600;
    color: #e0e0e0;
    margin: 0;
  }

  .status-badge {
    font-size: 0.75rem;
    padding: 0.125rem 0.5rem;
    border-radius: 9999px;
    font-weight: 500;
  }

  .status-connected {
    background: #16a34a22;
    color: #4ade80;
  }

  .status-disconnected {
    background: #dc262622;
    color: #f87171;
  }

  .log-count {
    font-size: 0.75rem;
    color: #888;
  }

  .logs-actions {
    display: flex;
    gap: 0.5rem;
  }

  .logs-container {
    flex: 1;
    overflow-y: auto;
    padding: 0.5rem 0;
    font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
    font-size: 0.8125rem;
    line-height: 1.5;
  }

  .log-line {
    padding: 0 1rem;
    white-space: pre-wrap;
    word-break: break-all;
  }

  .log-line:hover {
    background: #2a2a2a;
  }

  .line-num {
    display: inline-block;
    width: 4rem;
    text-align: right;
    margin-right: 1rem;
    color: #555;
    user-select: none;
  }

  .log-empty {
    padding: 2rem;
    text-align: center;
    color: #666;
  }

  .btn {
    padding: 0.25rem 0.75rem;
    border: none;
    border-radius: 0.25rem;
    cursor: pointer;
    font-size: 0.8125rem;
    font-weight: 500;
  }

  .btn-primary {
    background: #2563eb;
    color: white;
  }

  .btn-primary:hover {
    background: #1e40af;
  }

  .btn-danger {
    background: #dc2626;
    color: white;
  }

  .btn-danger:hover {
    background: #b91c1c;
  }

  .btn-secondary {
    background: #404040;
    color: #d4d4d4;
  }

  .btn-secondary:hover {
    background: #525252;
  }

  .btn-sm {
    padding: 0.25rem 0.625rem;
    font-size: 0.75rem;
  }
</style>
