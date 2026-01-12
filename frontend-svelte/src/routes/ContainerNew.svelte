<script>
  import { link, push } from 'svelte-spa-router';

  const API_BASE = '/api';

  let name = '';
  let image = '';
  let containerPort = '';
  let command = '';
  let envVars = '';
  let persistData = false;
  let creating = false;
  let error = '';

  async function createContainer() {
    if (!name.trim() || !image.trim() || !containerPort) {
      error = '이름, 이미지, 컨테이너 포트는 필수입니다';
      return;
    }

    const portNum = parseInt(String(containerPort));
    if (isNaN(portNum) || portNum < 1 || portNum > 65535) {
      error = '유효한 포트 번호를 입력하세요 (1-65535)';
      return;
    }

    creating = true;
    error = '';

    try {
      // Parse env_vars
      let parsedEnvVars = {};
      if (envVars.trim()) {
        envVars.split('\n').forEach(line => {
          const [key, ...valueParts] = line.split('=');
          if (key && valueParts.length > 0) {
            parsedEnvVars[key.trim()] = valueParts.join('=').trim();
          }
        });
      }

      const response = await fetch(`${API_BASE}/containers`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: name.trim(),
          image: image.trim(),
          container_port: portNum,
          command: command.trim() || null,
          env_vars: Object.keys(parsedEnvVars).length > 0 ? parsedEnvVars : null,
          persist_data: persistData
        })
      });

      if (!response.ok) {
        const data = await response.json();
        throw new Error(data.error || '컨테이너를 생성할 수 없습니다');
      }

      push('/');
    } catch (e) {
      error = e.message;
    } finally {
      creating = false;
    }
  }
</script>

<header>
  <div class="header-content">
    <a href="/" use:link style="text-decoration: none; color: inherit;">
      <h1>Easy CI/CD</h1>
    </a>
    <div class="header-actions">
      <a href="/" use:link class="btn btn-secondary">← 돌아가기</a>
    </div>
  </div>
</header>

<div class="container">
  <div class="card">
    <div class="card-header">
      <h2 class="card-title">📦 새 컨테이너</h2>
    </div>

    <div class="form-content">
      {#if error}
        <div class="error-message">{error}</div>
      {/if}

      <div class="form-group">
        <label for="name">이름 *</label>
        <input type="text" id="name" bind:value={name} placeholder="my-redis" class="form-input" />
        <span class="form-help">컨테이너 이름 (영문, 숫자, 하이픈만)</span>
      </div>

      <div class="form-group">
        <label for="image">이미지 *</label>
        <input type="text" id="image" bind:value={image} placeholder="redis:alpine" class="form-input" />
        <span class="form-help">Docker Hub 이미지 (예: mysql:8, postgres:15, redis:alpine)</span>
      </div>

      <div class="form-group">
        <label for="containerPort">컨테이너 포트 *</label>
        <input type="number" id="containerPort" bind:value={containerPort} placeholder="3000" class="form-input" min="1" max="65535" />
        <span class="form-help">컨테이너 내부에서 사용할 포트 (외부 포트는 자동 할당)</span>
      </div>

      <div class="form-group">
        <label for="command">명령 (선택)</label>
        <input type="text" id="command" bind:value={command} placeholder="redis-server --appendonly yes" class="form-input" />
        <span class="form-help">컨테이너 시작 시 실행할 명령</span>
      </div>

      <div class="form-group">
        <label for="envVars">환경 변수 (선택)</label>
        <textarea id="envVars" bind:value={envVars} rows="3" placeholder="MYSQL_ROOT_PASSWORD=secret&#10;MYSQL_DATABASE=mydb" class="form-input"></textarea>
        <span class="form-help">줄바꿈으로 구분, KEY=VALUE 형식</span>
      </div>

      <div class="form-group">
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={persistData} class="form-checkbox" />
          <span>데이터 영구 저장</span>
        </label>
        <span class="form-help">체크하면 컨테이너 데이터가 /data 경로에 영구 저장됩니다</span>
      </div>

      <div class="form-actions">
        <a href="/" use:link class="btn btn-secondary">취소</a>
        <button on:click={createContainer} class="btn btn-primary" disabled={creating}>
          {creating ? '생성 중...' : '컨테이너 생성'}
        </button>
      </div>
    </div>
  </div>
</div>

<style>
  .container {
    max-width: 600px;
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
  }

  .card-title {
    font-size: 1.25rem;
    font-weight: 700;
    margin: 0;
    color: #111827;
  }

  .form-content {
    padding: 1.5rem;
  }

  .form-group {
    margin-bottom: 1.25rem;
  }

  .form-group label {
    display: block;
    font-weight: 500;
    margin-bottom: 0.375rem;
    color: #374151;
    font-size: 0.875rem;
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

  .form-help {
    font-size: 0.75rem;
    color: #6b7280;
    margin-top: 0.25rem;
    display: block;
  }

  .checkbox-label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    cursor: pointer;
    font-weight: 500;
    color: #374151;
  }

  .form-checkbox {
    width: 1.125rem;
    height: 1.125rem;
    cursor: pointer;
  }

  .form-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.75rem;
    margin-top: 1.5rem;
    padding-top: 1.5rem;
    border-top: 1px solid #e5e7eb;
  }

  .error-message {
    background: #fee2e2;
    color: #991b1b;
    padding: 0.75rem 1rem;
    border-radius: 0.375rem;
    margin-bottom: 1rem;
    font-size: 0.875rem;
  }

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

  .btn-primary {
    background: #3b82f6;
    color: white;
  }

  .btn-primary:hover:not(:disabled) {
    background: #2563eb;
  }

  .btn-primary:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .btn-secondary {
    background: #6b7280;
    color: white;
  }

  .btn-secondary:hover {
    background: #4b5563;
  }

</style>
