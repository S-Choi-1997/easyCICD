<script>
  import { onMount, onDestroy } from 'svelte';
  import { link, push } from 'svelte-spa-router';
  import { subscribe } from '../stores/websocket';

  export let params = {};
  const projectId = params.id;
  const API_BASE = '/api';

  let project = null;
  let builds = [];
  let loading = true;
  let activeTab = 'builds'; // 'builds' | 'runtime-logs'

  // Runtime logs
  let runtimeLogs = [];
  let runtimeWs = null;
  let runtimeLogsConnected = false;

  // WebSocket subscription
  let unsubscribeWs = null;

  // Build detail
  let selectedBuild = null;
  let buildLogs = [];
  let deployLogs = [];
  let showBuildLogs = true;
  let showDeployLogs = true;

  // Project edit mode
  let editMode = false;
  let editingProject = null;
  let saving = false;
  let saveError = '';

  onMount(async () => {
    await loadProject();
    await loadBuilds();

    // Subscribe to WebSocket for build status updates
    unsubscribeWs = subscribe('project-detail', (data) => {
      console.log('📡 [ProjectDetail] WebSocket 이벤트:', data.type, data);

      // Update builds list on build status change
      if (data.type === 'build_status' && data.project_id === parseInt(projectId)) {
        console.log('📡 [ProjectDetail] 빌드 상태 업데이트:', data.build_id, data.status);

        // Update specific build in the list
        builds = builds.map(build =>
          build.id === data.build_id
            ? { ...build, status: data.status }
            : build
        );
      }

      // Refresh builds list on new build or deployment
      if ((data.type === 'deployment' || data.type === 'build_queued') &&
          data.project_id === parseInt(projectId)) {
        console.log('📡 [ProjectDetail] 빌드 목록 새로고침');
        loadBuilds();
      }
    });
  });

  onDestroy(() => {
    disconnectRuntimeLogs();
    if (unsubscribeWs) {
      unsubscribeWs();
    }
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

  async function loadBuilds() {
    try {
      const response = await fetch(`${API_BASE}/builds?project_id=${projectId}`);
      if (response.ok) {
        builds = await response.json();
        // Sort by build_number descending
        builds.sort((a, b) => b.build_number - a.build_number);
      }
    } catch (error) {
      console.error('빌드 목록 로드 실패:', error);
    }
  }

  async function handleRollback(buildId, buildNumber) {
    if (!confirm(`빌드 #${buildNumber}로 롤백하시겠습니까?`)) return;

    try {
      const response = await fetch(`${API_BASE}/projects/${projectId}/rollback/${buildId}`, {
        method: 'POST'
      });

      if (response.ok) {
        const result = await response.json();
        alert(result.message || '롤백이 완료되었습니다');
        await loadProject();
        await loadBuilds();
      } else {
        const error = await response.json();
        alert('롤백 실패: ' + (error.error || '알 수 없는 오류'));
      }
    } catch (error) {
      alert('롤백 요청 실패: ' + error.message);
    }
  }

  async function showBuildDetail(build) {
    selectedBuild = build;
    buildLogs = [];
    deployLogs = [];

    // Load build logs
    try {
      const response = await fetch(`${API_BASE}/builds/${build.id}/build-logs`);
      if (response.ok) {
        const text = await response.text();
        if (text) {
          buildLogs = text.split('\n').filter(line => line.trim());
        }
      }
    } catch (error) {
      console.error('빌드 로그 로딩 실패:', error);
    }

    // Load deploy logs
    try {
      const response = await fetch(`${API_BASE}/builds/${build.id}/deploy-logs`);
      if (response.ok) {
        const text = await response.text();
        if (text) {
          deployLogs = text.split('\n').filter(line => line.trim());
        }
      }
    } catch (error) {
      console.error('배포 로그 로딩 실패:', error);
    }
  }

  function connectRuntimeLogs() {
    if (runtimeWs) {
      disconnectRuntimeLogs();
    }

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/api/projects/${projectId}/runtime-logs`;

    runtimeWs = new WebSocket(wsUrl);

    runtimeWs.onopen = () => {
      runtimeLogsConnected = true;
      runtimeLogs = ['연결되었습니다...'];
    };

    runtimeWs.onmessage = (event) => {
      const logLine = event.data;
      runtimeLogs = [...runtimeLogs, logLine];

      // 최대 500줄까지만 유지
      if (runtimeLogs.length > 500) {
        runtimeLogs = runtimeLogs.slice(-500);
      }

      // 자동 스크롤
      setTimeout(() => {
        const logsContainer = document.getElementById('runtime-logs-container');
        if (logsContainer) {
          logsContainer.scrollTop = logsContainer.scrollHeight;
        }
      }, 10);
    };

    runtimeWs.onerror = (error) => {
      console.error('WebSocket 에러:', error);
      runtimeLogsConnected = false;
    };

    runtimeWs.onclose = () => {
      runtimeLogsConnected = false;
      runtimeLogs = [...runtimeLogs, '연결이 종료되었습니다.'];
    };
  }

  function disconnectRuntimeLogs() {
    if (runtimeWs) {
      runtimeWs.close();
      runtimeWs = null;
      runtimeLogsConnected = false;
    }
  }

  function handleTabChange(tab) {
    activeTab = tab;

    if (tab === 'runtime-logs' && !runtimeLogsConnected) {
      connectRuntimeLogs();
    } else if (tab === 'builds' && runtimeLogsConnected) {
      disconnectRuntimeLogs();
    }
  }

  function getStatusColor(status) {
    const colors = {
      'Success': 'bg-green-100 text-green-800',
      'Failed': 'bg-red-100 text-red-800',
      'Building': 'bg-blue-100 text-blue-800',
      'Deploying': 'bg-yellow-100 text-yellow-800',
      'Queued': 'bg-gray-100 text-gray-800'
    };
    return colors[status] || 'bg-gray-100 text-gray-800';
  }

  // --- Project Edit Functions ---
  function startEdit() {
    editingProject = { ...project };

    // Convert JSON env vars to text format
    if (editingProject.build_env_vars) {
      try {
        const parsed = JSON.parse(editingProject.build_env_vars);
        editingProject.build_env_vars_text = Object.entries(parsed)
          .map(([k, v]) => `${k}=${v}`)
          .join('\n');
      } catch {
        editingProject.build_env_vars_text = '';
      }
    } else {
      editingProject.build_env_vars_text = '';
    }

    if (editingProject.runtime_env_vars) {
      try {
        const parsed = JSON.parse(editingProject.runtime_env_vars);
        editingProject.runtime_env_vars_text = Object.entries(parsed)
          .map(([k, v]) => `${k}=${v}`)
          .join('\n');
      } catch {
        editingProject.runtime_env_vars_text = '';
      }
    } else {
      editingProject.runtime_env_vars_text = '';
    }

    editMode = true;
    saveError = '';
  }

  function cancelEdit() {
    editMode = false;
    editingProject = null;
    saveError = '';
  }

  function parseEnvVars(envStr) {
    const result = {};
    if (!envStr || !envStr.trim()) return result;  // Return empty object, not null
    envStr.split('\n').forEach(line => {
      const trimmed = line.trim();
      if (!trimmed) return;
      const [key, ...valueParts] = trimmed.split('=');
      if (key && valueParts.length > 0) {
        result[key.trim()] = valueParts.join('=').trim();
      }
    });
    return result;  // Always return object (empty or with entries)
  }

  async function saveProject() {
    saving = true;
    saveError = '';

    try {
      const updateData = {
        name: editingProject.name,
        repo: editingProject.repo,
        branch: editingProject.branch,
        path_filter: editingProject.path_filter,
        build_image: editingProject.build_image,
        build_command: editingProject.build_command,
        cache_type: editingProject.cache_type,
        working_directory: editingProject.working_directory || null,
        runtime_image: editingProject.runtime_image,
        runtime_command: editingProject.runtime_command,
        health_check_url: editingProject.health_check_url,
        runtime_port: editingProject.runtime_port,
        build_env_vars: Object.keys(parseEnvVars(editingProject.build_env_vars_text)).length > 0
          ? JSON.stringify(parseEnvVars(editingProject.build_env_vars_text))
          : null,
        runtime_env_vars: Object.keys(parseEnvVars(editingProject.runtime_env_vars_text)).length > 0
          ? JSON.stringify(parseEnvVars(editingProject.runtime_env_vars_text))
          : null,
      };

      const response = await fetch(`${API_BASE}/projects/${projectId}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(updateData)
      });

      if (response.ok) {
        project = await response.json();
        editMode = false;
        editingProject = null;
      } else {
        // Try to parse as JSON, fallback to text
        const contentType = response.headers.get('content-type');
        if (contentType && contentType.includes('application/json')) {
          const error = await response.json();
          saveError = error.error || '저장에 실패했습니다';
        } else {
          const text = await response.text();
          saveError = text || '저장에 실패했습니다';
        }
      }
    } catch (error) {
      saveError = error.message || '알 수 없는 오류가 발생했습니다';
    } finally {
      saving = false;
    }
  }
</script>

<div class="container">
  <!-- Header -->
  <div style="margin-bottom: 1.5rem;">
    <a href="/" use:link class="project-url" style="display: inline-block; margin-bottom: 0.5rem;">
      ← 대시보드로 돌아가기
    </a>

    {#if loading}
      <div class="loading">로딩 중...</div>
    {:else if project}
      <div style="display: flex; justify-content: space-between; align-items: flex-start;">
        <div>
          <h1 style="font-size: 2rem; font-weight: 600; color: var(--gray-900); margin-bottom: 0.5rem;">{project.name}</h1>
          <p class="text-muted">
            {project.repo} ({project.branch})
          </p>
        </div>
        <button on:click={startEdit} class="btn btn-secondary">
          설정 수정
        </button>
      </div>
    {/if}
  </div>

  <!-- Tabs -->
  <div class="card">
    <div class="tabs">
      <button
        on:click={() => handleTabChange('builds')}
        class="tab {activeTab === 'builds' ? 'tab-active' : ''}"
      >
        빌드 히스토리
      </button>
      <button
        on:click={() => handleTabChange('runtime-logs')}
        class="tab {activeTab === 'runtime-logs' ? 'tab-active' : ''}"
      >
        런타임 로그
        {#if runtimeLogsConnected}
          <span class="status-badge status-success" style="margin-left: 0.5rem; font-size: 0.75rem;">
            연결됨
          </span>
        {/if}
      </button>
    </div>

    <!-- Tab Content -->
    {#if activeTab === 'builds'}
      <!-- Builds Tab -->
      <div class="card-header" style="border-top: 1px solid var(--gray-200); margin: 0 -1.5rem; padding: 1rem 1.5rem;">
        <h3 class="card-title">빌드 히스토리</h3>
      </div>

      <ul class="build-list">
        {#each builds as build}
          <li class="build-item" style="cursor: pointer;" on:click={() => showBuildDetail(build)}>
            <div class="build-info">
              <span class="build-number">#{build.build_number}</span>
              <span class="status-badge build-status {build.status.toLowerCase()}">
                {build.status}
              </span>
              {#if build.deployed_slot}
                <span class="status-badge slot-badge {build.deployed_slot.toLowerCase()} {project && project.active_slot === build.deployed_slot && (build.deployed_slot === 'Blue' ? project.blue_container_id : project.green_container_id) ? 'running' : 'stopped'}">
                  {build.deployed_slot}
                </span>
              {/if}
            </div>
            <div style="flex: 1; margin: 0 1rem; min-width: 0;">
              <div class="build-commit" title="{build.commit_message || build.commit_hash}">
                {build.commit_message || build.commit_hash}
              </div>
              {#if build.author}
                <div class="text-xs text-muted">by {build.author}</div>
              {/if}
              <div class="build-time">
                {new Date(build.started_at).toLocaleString('ko-KR')}
              </div>
            </div>

            <div style="display: flex; gap: 0.5rem;">
              {#if build.status === 'Success' && build.deployed_slot}
                <button
                  on:click|stopPropagation={() => handleRollback(build.id, build.build_number)}
                  class="btn btn-primary btn-sm"
                >
                  롤백
                </button>
              {/if}
              <button
                on:click|stopPropagation={() => showBuildDetail(build)}
                class="btn btn-secondary btn-sm"
              >
                로그 보기
              </button>
            </div>
          </li>
        {:else}
          <li style="padding: 2rem; text-align: center; color: var(--gray-600);">
            빌드 히스토리가 없습니다
          </li>
        {/each}
      </ul>

      <!-- Build Detail Modal -->
      {#if selectedBuild}
        <div class="modal-overlay" on:click={() => selectedBuild = null}>
          <div class="modal-content" on:click|stopPropagation style="max-width: 900px; max-height: 80vh; overflow-y: auto;">
            <div class="modal-header">
              <h3>빌드 #{selectedBuild.build_number} 상세</h3>
              <button on:click={() => selectedBuild = null} class="btn btn-secondary btn-sm">닫기</button>
            </div>

            <div style="padding: 1.5rem;">
              <!-- Build Info -->
              <div style="display: grid; gap: 0.5rem; margin-bottom: 1.5rem; font-size: 0.875rem;">
                <div><strong>상태:</strong> <span class="status-badge status-{selectedBuild.status.toLowerCase()}">{selectedBuild.status}</span></div>
                <div><strong>커밋:</strong> {selectedBuild.commit_message || selectedBuild.commit_hash}</div>
                {#if selectedBuild.author}
                  <div><strong>작성자:</strong> {selectedBuild.author}</div>
                {/if}
                <div><strong>시작:</strong> {new Date(selectedBuild.started_at).toLocaleString('ko-KR')}</div>
                {#if selectedBuild.finished_at}
                  <div><strong>완료:</strong> {new Date(selectedBuild.finished_at).toLocaleString('ko-KR')}</div>
                {/if}
              </div>

              <!-- Build Logs -->
              <div style="margin-bottom: 1.5rem;">
                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.5rem; padding: 0.75rem; background: var(--gray-100); border-radius: 0.375rem; cursor: pointer;" on:click={() => showBuildLogs = !showBuildLogs}>
                  <h4 style="margin: 0;">{showBuildLogs ? '▼' : '▶'} 빌드 로그 ({buildLogs.length}줄)</h4>
                </div>
                {#if showBuildLogs}
                  <div class="log-viewer">
                    {#if buildLogs.length === 0}
                      <div style="color: var(--gray-600);">빌드 로그가 없습니다</div>
                    {:else}
                      {#each buildLogs as log, idx}
                        <div class="log-line">
                          <span style="color: var(--gray-600); margin-right: 1rem;">{idx + 1}</span>
                          {log}
                        </div>
                      {/each}
                    {/if}
                  </div>
                {/if}
              </div>

              <!-- Deploy Logs -->
              <div>
                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.5rem; padding: 0.75rem; background: var(--gray-100); border-radius: 0.375rem; cursor: pointer;" on:click={() => showDeployLogs = !showDeployLogs}>
                  <h4 style="margin: 0;">{showDeployLogs ? '▼' : '▶'} 배포 로그 ({deployLogs.length}줄)</h4>
                </div>
                {#if showDeployLogs}
                  <div class="log-viewer">
                    {#if deployLogs.length === 0}
                      <div style="color: var(--gray-600);">배포 로그가 없습니다</div>
                    {:else}
                      {#each deployLogs as log, idx}
                        <div class="log-line">
                          <span style="color: var(--gray-600); margin-right: 1rem;">{idx + 1}</span>
                          {log}
                        </div>
                      {/each}
                    {/if}
                  </div>
                {/if}
              </div>
            </div>
          </div>
        </div>
      {/if}
    {:else if activeTab === 'runtime-logs'}
      <!-- Runtime Logs Tab -->
      <div class="card-header" style="border-top: 1px solid var(--gray-200); margin: 0 -1.5rem; padding: 1rem 1.5rem; display: flex; justify-content: space-between; align-items: center;">
        <h3 class="card-title">런타임 로그</h3>
        <div style="display: flex; gap: 0.5rem;">
          {#if !runtimeLogsConnected}
            <button
              on:click={connectRuntimeLogs}
              class="btn btn-primary btn-sm"
            >
              연결
            </button>
          {:else}
            <button
              on:click={disconnectRuntimeLogs}
              class="btn btn-danger btn-sm"
            >
              연결 종료
            </button>
          {/if}
          <button
            on:click={() => runtimeLogs = []}
            class="btn btn-secondary btn-sm"
          >
            지우기
          </button>
        </div>
      </div>

      <div
        id="runtime-logs-container"
        class="log-viewer"
      >
        {#each runtimeLogs as log}
          <div class="log-line">{log}</div>
        {:else}
          <div style="color: var(--gray-600);">로그가 없습니다. 연결 버튼을 클릭하세요.</div>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Project Edit Modal -->
  {#if editMode && editingProject}
    <div class="modal-overlay" on:click={cancelEdit}>
      <div class="modal-content" on:click|stopPropagation style="max-width: 700px; max-height: 85vh; overflow-y: auto;">
        <div class="modal-header">
          <h3>프로젝트 설정 수정</h3>
          <button on:click={cancelEdit} class="btn btn-secondary btn-sm">닫기</button>
        </div>

        <div style="padding: 1.5rem;">
          {#if saveError}
            <div class="error-message" style="margin-bottom: 1rem; padding: 0.75rem; background: #fee2e2; color: #991b1b; border-radius: 0.375rem;">
              {saveError}
            </div>
          {/if}

          <!-- 기본 설정 -->
          <div class="form-group">
            <label for="edit-name">프로젝트 이름</label>
            <input type="text" id="edit-name" bind:value={editingProject.name} class="form-input" />
          </div>

          <div class="form-group">
            <label for="edit-branch">브랜치</label>
            <input type="text" id="edit-branch" bind:value={editingProject.branch} class="form-input" />
          </div>

          <div class="form-group">
            <label for="edit-path-filter">빌드 트리거 경로</label>
            <input type="text" id="edit-path-filter" bind:value={editingProject.path_filter} class="form-input" />
            <span class="form-help">변경된 파일 경로가 이 패턴과 일치할 때만 빌드 트리거 (* = 모든 파일)</span>
          </div>

          <div class="form-group">
            <label for="edit-working-dir">빌드 실행 디렉토리</label>
            <input type="text" id="edit-working-dir" bind:value={editingProject.working_directory} class="form-input" placeholder="(루트)" />
            <span class="form-help">모노레포에서 특정 디렉토리에서 빌드할 때 사용</span>
          </div>

          <!-- 빌드 설정 -->
          <h4 style="margin: 1.5rem 0 1rem; padding-top: 1rem; border-top: 1px solid var(--gray-200);">빌드 설정</h4>

          <div class="form-group">
            <label for="edit-build-image">빌드 이미지</label>
            <input type="text" id="edit-build-image" bind:value={editingProject.build_image} class="form-input" />
          </div>

          <div class="form-group">
            <label for="edit-build-command">빌드 명령어</label>
            <input type="text" id="edit-build-command" bind:value={editingProject.build_command} class="form-input" />
          </div>

          <div class="form-group">
            <label for="edit-build-env">빌드 환경변수</label>
            <textarea
              id="edit-build-env"
              bind:value={editingProject.build_env_vars_text}
              rows="3"
              class="form-input"
              style="font-family: monospace; font-size: 0.875rem;"
              placeholder="KEY=VALUE (줄바꿈으로 구분)"
            ></textarea>
            <span class="form-help">기본 환경변수: CI=true, SKIP_PREFLIGHT_CHECK=true</span>
          </div>

          <!-- 런타임 설정 -->
          <h4 style="margin: 1.5rem 0 1rem; padding-top: 1rem; border-top: 1px solid var(--gray-200);">런타임 설정</h4>

          <div class="form-group">
            <label for="edit-runtime-image">런타임 이미지</label>
            <input type="text" id="edit-runtime-image" bind:value={editingProject.runtime_image} class="form-input" />
          </div>

          <div class="form-group">
            <label for="edit-runtime-command">런타임 명령어</label>
            <input type="text" id="edit-runtime-command" bind:value={editingProject.runtime_command} class="form-input" />
          </div>

          <div class="form-group">
            <label for="edit-runtime-port">런타임 포트</label>
            <input type="number" id="edit-runtime-port" bind:value={editingProject.runtime_port} class="form-input" style="width: 120px;" />
            <span class="form-help">컨테이너 내부에서 앱이 리슨하는 포트</span>
          </div>

          <div class="form-group">
            <label for="edit-health-check">헬스체크 URL</label>
            <input type="text" id="edit-health-check" bind:value={editingProject.health_check_url} class="form-input" />
          </div>

          <div class="form-group">
            <label for="edit-runtime-env">런타임 환경변수</label>
            <textarea
              id="edit-runtime-env"
              bind:value={editingProject.runtime_env_vars_text}
              rows="3"
              class="form-input"
              style="font-family: monospace; font-size: 0.875rem;"
              placeholder="KEY=VALUE (줄바꿈으로 구분)"
            ></textarea>
            <span class="form-help">기본 환경변수: PORT=(런타임포트)</span>
          </div>

          <!-- 저장 버튼 -->
          <div style="display: flex; justify-content: flex-end; gap: 0.75rem; margin-top: 1.5rem; padding-top: 1rem; border-top: 1px solid var(--gray-200);">
            <button on:click={cancelEdit} class="btn btn-secondary">취소</button>
            <button on:click={saveProject} class="btn btn-primary" disabled={saving}>
              {saving ? '저장 중...' : '저장'}
            </button>
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  /* 빌드 상태 배지 - 연한 색상 (정보 전달용) */
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

  .status-badge.build-status.deploying {
    background: #fef3c7;
    color: #92400e;
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

  /* 커밋 메시지 한 줄로 제한 */
  .build-commit {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 400px;
  }
</style>
