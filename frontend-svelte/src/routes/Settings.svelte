<script>
  import { onMount } from 'svelte';
  import { link } from 'svelte-spa-router';

  const API_BASE = '/api';

  let domain = '';
  let domainConfigured = false;
  let loading = true;
  let saving = false;

  onMount(async () => {
    await loadDomain();
  });

  async function loadDomain() {
    loading = true;
    try {
      const response = await fetch(`${API_BASE}/settings/domain`);
      const data = await response.json();
      domainConfigured = data.configured || false;
      domain = data.domain || '';
    } catch (error) {
      console.error('도메인 로드 실패:', error);
    } finally {
      loading = false;
    }
  }

  async function saveDomain() {
    if (!domain.trim()) {
      return;
    }

    saving = true;
    try {
      const response = await fetch(`${API_BASE}/settings/domain`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ domain: domain.trim() }),
      });

      if (response.ok) {
        domainConfigured = true;
      }
    } catch (error) {
      console.error(error);
    } finally {
      saving = false;
    }
  }
</script>

<header>
  <div class="header-content">
    <a href="/" use:link style="text-decoration: none; color: inherit; cursor: pointer;">
      <h1>Easy CI/CD</h1>
    </a>
    <div class="header-actions">
      <a href="/" use:link class="btn btn-secondary">← 대시보드로 돌아가기</a>
    </div>
  </div>
</header>

<div class="container">
  <div class="card">
    <div class="card-header">
      <h2 class="card-title">설정</h2>
    </div>

    {#if loading}
      <div class="loading">
        <div class="spinner"></div>
        <p>설정 불러오는 중...</p>
      </div>
    {:else}
      <div class="form-section">
        <h3>도메인 설정</h3>
        <p class="text-muted mb-2">
          서브도메인 라우팅에 사용할 기본 도메인을 설정합니다.
        </p>
        <p class="text-muted mb-3">
          프로젝트별 서브도메인은 <code>{프로젝트명}-app.{기본도메인}</code> 형식으로 생성됩니다.<br>
          예: <code>albl.cloud</code> 입력 시 → <code>myproject-app.albl.cloud</code>
        </p>

        <div class="form-group">
          <label for="domain">기본 도메인 (선택사항)</label>
          <input
            type="text"
            id="domain"
            class="form-input"
            bind:value={domain}
            placeholder="albl.cloud"
          />
        </div>

        <button
          on:click={saveDomain}
          class="btn btn-primary"
          disabled={saving}
        >
          {saving ? '저장 중...' : '도메인 저장'}
        </button>

        {#if domainConfigured && !saving}
          <div class="alert alert-success mt-3">
            ✓ 도메인이 설정되어 있습니다: <strong>{domain}</strong>
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .container {
    max-width: 800px;
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
    font-size: 1.5rem;
    font-weight: 700;
    margin: 0;
    color: #111827;
  }

  .form-section {
    padding: 1.5rem;
  }

  .form-section h3 {
    font-size: 1.125rem;
    font-weight: 600;
    margin-bottom: 0.5rem;
    color: #111827;
  }

  .form-group {
    margin-bottom: 1rem;
  }

  label {
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

  .text-muted {
    color: #6b7280;
    font-size: 0.875rem;
  }

  .mb-2 {
    margin-bottom: 0.5rem;
  }

  .mb-3 {
    margin-bottom: 0.75rem;
  }

  .mt-3 {
    margin-top: 0.75rem;
  }

  code {
    background: #f3f4f6;
    padding: 0.125rem 0.375rem;
    border-radius: 0.25rem;
    font-family: monospace;
    font-size: 0.875rem;
    color: #1f2937;
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

  .alert {
    padding: 0.75rem 1rem;
    border-radius: 0.375rem;
    font-size: 0.875rem;
  }

  .alert-success {
    background: #d1fae5;
    color: #065f46;
    border: 1px solid #6ee7b7;
  }

  .loading {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 3rem;
    color: #6b7280;
  }

  .spinner {
    border: 3px solid #f3f4f6;
    border-top-color: #3b82f6;
    border-radius: 50%;
    width: 2rem;
    height: 2rem;
    animation: spin 1s linear infinite;
    margin-bottom: 1rem;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

</style>
