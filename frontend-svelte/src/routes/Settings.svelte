<script>
  import { onMount } from 'svelte';
  import { link } from 'svelte-spa-router';

  const API_BASE = '/api';

  let domain = '';
  let domainConfigured = false;
  let tcpDomain = '';
  let tcpDomainConfigured = false;
  let webhookUrl = '';
  let webhookUrlConfigured = false;
  let loading = true;
  let saving = false;
  let savingTcp = false;
  let savingWebhook = false;

  // Whitelist state
  let allowedEmails = [];
  let newEmail = '';
  let addingEmail = false;
  let removingEmail = null;
  let whitelistError = '';

  onMount(async () => {
    await loadSettings();
  });

  async function loadSettings() {
    loading = true;
    try {
      const [domainRes, tcpDomainRes, webhookUrlRes, emailsRes] = await Promise.all([
        fetch(`${API_BASE}/settings/domain`),
        fetch(`${API_BASE}/settings/tcp-domain`),
        fetch(`${API_BASE}/settings/webhook-url`),
        fetch(`/admin/allowed-emails`)
      ]);

      const domainData = await domainRes.json();
      domainConfigured = domainData.configured || false;
      domain = domainData.domain || '';

      const tcpDomainData = await tcpDomainRes.json();
      tcpDomainConfigured = tcpDomainData.configured || false;
      tcpDomain = tcpDomainData.tcp_domain || '';

      const webhookUrlData = await webhookUrlRes.json();
      webhookUrlConfigured = webhookUrlData.configured || false;
      webhookUrl = webhookUrlData.webhook_url || '';

      const emailsData = await emailsRes.json();
      allowedEmails = emailsData.emails || [];
    } catch (error) {
      console.error('설정 로드 실패:', error);
    } finally {
      loading = false;
    }
  }

  async function addEmail() {
    if (!newEmail.trim()) return;

    whitelistError = '';
    addingEmail = true;

    try {
      const response = await fetch(`/admin/allowed-emails`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email: newEmail.trim() }),
      });

      const data = await response.json();

      if (response.ok) {
        allowedEmails = data.emails || [];
        newEmail = '';
      } else {
        whitelistError = data.error || '이메일 추가 실패';
      }
    } catch (error) {
      whitelistError = '서버 오류';
    } finally {
      addingEmail = false;
    }
  }

  async function removeEmail(email) {
    whitelistError = '';
    removingEmail = email;

    try {
      const response = await fetch(`/admin/allowed-emails`, {
        method: 'DELETE',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email }),
      });

      const data = await response.json();

      if (response.ok) {
        allowedEmails = data.emails || [];
      } else {
        whitelistError = data.error || '이메일 삭제 실패';
      }
    } catch (error) {
      whitelistError = '서버 오류';
    } finally {
      removingEmail = null;
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

  async function saveTcpDomain() {
    if (!tcpDomain.trim()) {
      return;
    }

    savingTcp = true;
    try {
      const response = await fetch(`${API_BASE}/settings/tcp-domain`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ tcp_domain: tcpDomain.trim() }),
      });

      if (response.ok) {
        tcpDomainConfigured = true;
      }
    } catch (error) {
      console.error(error);
    } finally {
      savingTcp = false;
    }
  }

  async function saveWebhookUrl() {
    if (!webhookUrl.trim()) {
      return;
    }

    savingWebhook = true;
    try {
      const response = await fetch(`${API_BASE}/settings/webhook-url`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ webhook_url: webhookUrl.trim() }),
      });

      if (response.ok) {
        webhookUrlConfigured = true;
      }
    } catch (error) {
      console.error(error);
    } finally {
      savingWebhook = false;
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
          프로젝트별 서브도메인은 <code>[프로젝트명]-app.[기본도메인]</code> 형식으로 생성됩니다.<br>
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

      <hr class="divider" />

      <div class="form-section">
        <h3>TCP 도메인 설정</h3>
        <p class="text-muted mb-2">
          Redis, MySQL 등 TCP 프로토콜 서비스 접속에 사용할 도메인을 설정합니다.
        </p>
        <p class="text-muted mb-3">
          DuckDNS 등으로 서버 IP에 연결한 도메인을 입력하세요.<br>
          예: <code>myserver.duckdns.org</code> → <code>myserver.duckdns.org:15000</code>
        </p>

        <div class="form-group">
          <label for="tcpDomain">TCP 도메인 (선택사항)</label>
          <input
            type="text"
            id="tcpDomain"
            class="form-input"
            bind:value={tcpDomain}
            placeholder="myserver.duckdns.org"
          />
        </div>

        <button
          on:click={saveTcpDomain}
          class="btn btn-primary"
          disabled={savingTcp}
        >
          {savingTcp ? '저장 중...' : 'TCP 도메인 저장'}
        </button>

        {#if tcpDomainConfigured && !savingTcp}
          <div class="alert alert-success mt-3">
            ✓ TCP 도메인이 설정되어 있습니다: <strong>{tcpDomain}</strong>
          </div>
        {/if}
      </div>

      <hr class="divider" />

      <div class="form-section">
        <h3>웹훅 URL 설정</h3>
        <p class="text-muted mb-2">
          GitHub 웹훅 등록에 사용할 URL을 설정합니다.
        </p>
        <p class="text-muted mb-3">
          Cloudflare Tunnel 등을 통해 외부에서 접근 가능한 URL을 입력하세요.<br>
          예: <code>https://cicd.example.com</code>
        </p>

        <div class="form-group">
          <label for="webhookUrl">웹훅 URL (선택사항)</label>
          <input
            type="text"
            id="webhookUrl"
            class="form-input"
            bind:value={webhookUrl}
            placeholder="https://cicd.example.com"
          />
        </div>

        <button
          on:click={saveWebhookUrl}
          class="btn btn-primary"
          disabled={savingWebhook}
        >
          {savingWebhook ? '저장 중...' : '웹훅 URL 저장'}
        </button>

        {#if webhookUrlConfigured && !savingWebhook}
          <div class="alert alert-success mt-3">
            ✓ 웹훅 URL이 설정되어 있습니다: <strong>{webhookUrl}</strong>
          </div>
        {/if}
      </div>

      <hr class="divider" />

      <div class="form-section">
        <h3>접근 허용 이메일 (화이트리스트)</h3>
        <p class="text-muted mb-2">
          Google 로그인을 허용할 이메일 주소를 등록합니다.
        </p>
        <p class="text-muted mb-3">
          <strong>비어있으면 모든 Google 계정으로 로그인 가능합니다.</strong><br>
          이메일을 등록하면 등록된 이메일만 로그인할 수 있습니다.
        </p>

        {#if whitelistError}
          <div class="alert alert-error mb-3">
            {whitelistError}
          </div>
        {/if}

        <div class="email-input-row">
          <input
            type="email"
            class="form-input"
            bind:value={newEmail}
            placeholder="user@example.com"
            on:keydown={(e) => e.key === 'Enter' && addEmail()}
            disabled={addingEmail}
          />
          <button
            on:click={addEmail}
            class="btn btn-primary"
            disabled={addingEmail || !newEmail.trim()}
          >
            {addingEmail ? '추가 중...' : '추가'}
          </button>
        </div>

        {#if allowedEmails.length === 0}
          <div class="alert alert-warning mt-3">
            화이트리스트가 비어있습니다. 모든 Google 계정으로 로그인이 가능합니다.
          </div>
        {:else}
          <div class="email-list mt-3">
            <p class="text-muted mb-2">등록된 이메일 ({allowedEmails.length}개):</p>
            {#each allowedEmails as email}
              <div class="email-item">
                <span class="email-address">{email}</span>
                <button
                  class="btn-remove"
                  on:click={() => removeEmail(email)}
                  disabled={removingEmail === email}
                  title="삭제"
                >
                  {removingEmail === email ? '...' : '×'}
                </button>
              </div>
            {/each}
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

  .divider {
    border: none;
    border-top: 1px solid #e5e7eb;
    margin: 1.5rem 0;
  }

  .email-input-row {
    display: flex;
    gap: 0.5rem;
  }

  .email-input-row .form-input {
    flex: 1;
  }

  .email-list {
    background: #f9fafb;
    border-radius: 0.5rem;
    padding: 1rem;
  }

  .email-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.5rem 0.75rem;
    background: white;
    border: 1px solid #e5e7eb;
    border-radius: 0.375rem;
    margin-bottom: 0.5rem;
  }

  .email-item:last-child {
    margin-bottom: 0;
  }

  .email-address {
    font-family: monospace;
    font-size: 0.875rem;
    color: #374151;
  }

  .btn-remove {
    background: transparent;
    border: none;
    color: #ef4444;
    font-size: 1.25rem;
    cursor: pointer;
    padding: 0.25rem 0.5rem;
    border-radius: 0.25rem;
    transition: all 0.2s;
  }

  .btn-remove:hover:not(:disabled) {
    background: #fee2e2;
  }

  .btn-remove:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .alert-warning {
    background: #fef3c7;
    color: #92400e;
    border: 1px solid #fcd34d;
  }

  .alert-error {
    background: #fee2e2;
    color: #dc2626;
    border: 1px solid #fca5a5;
  }
</style>
