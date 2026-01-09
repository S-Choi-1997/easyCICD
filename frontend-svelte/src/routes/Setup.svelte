<script>
    import { onMount } from 'svelte';
    import { push } from 'svelte-spa-router';

    const API_BASE = '/api';

    // GitHub PAT
    let githubPAT = '';
    let patConfigured = false;
    let githubUsername = '';

    // Project settings
    let projectName = '';
    let selectedRepo = '';
    let selectedBranch = '';
    let pathFilter = '';
    let workflowPath = '.github/workflows/';  // Custom workflow path

    // Data from API
    let repositories = [];
    let branches = [];

    // Auto-detected configuration
    let detectedConfig = null;
    let showAdvanced = false;
    let detectionStatus = 'idle'; // 'idle', 'loading', 'success', 'failed'

    // TOML configuration for advanced settings
    let configToml = '';
    let tomlError = '';
    const tomlPlaceholder = `# 빌드 설정
build_image = "node:20"
build_command = "npm install && npm run build"

# 실행 설정
runtime_image = "nginx:alpine"
runtime_command = "nginx -g 'daemon off;'"
health_check_url = "/"`;

    onMount(async () => {
        await checkPATStatus();
        if (patConfigured) {
            await loadRepositories();
        }
    });

    async function checkPATStatus() {
        try {
            const response = await fetch(`${API_BASE}/settings/github-pat-status`);
            const data = await response.json();
            patConfigured = data.configured || false;
            githubUsername = data.github_username || '';
        } catch (error) {
            console.error('PAT 상태 확인 실패:', error);
        }
    }

    async function savePAT() {
        if (!githubPAT.trim()) {
            return;
        }

        try {
            const response = await fetch(`${API_BASE}/settings/github-pat`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ github_pat: githubPAT }),
            });

            const data = await response.json();

            if (response.ok) {
                patConfigured = true;
                githubUsername = data.github_username;
                await loadRepositories();
            }
        } catch (error) {
            console.error(error);
        }
    }

    async function loadRepositories() {
        try {
            const response = await fetch(`${API_BASE}/github/repositories`);
            const data = await response.json();
            repositories = data.repositories || [];
        } catch (error) {
            console.error('레포지토리 로드 실패:', error);
        }
    }

    async function onRepoChange() {
        if (!selectedRepo) return;

        const [owner, repo] = selectedRepo.split('/');
        try {
            const response = await fetch(
                `${API_BASE}/github/branches?owner=${owner}&repo=${repo}`
            );
            const data = await response.json();
            branches = data.branches || [];

            // Reset selections
            selectedBranch = '';
            detectedConfig = null;
        } catch (error) {
            console.error('브랜치 로드 실패:', error);
        }
    }

    async function onBranchChange() {
        if (!selectedRepo || !selectedBranch) return;

        // Auto-detect project configuration
        await detectProject();
    }

    async function detectProject() {
        if (!selectedRepo || !selectedBranch) {
            alert('레포지토리와 브랜치를 선택하세요.');
            return;
        }

        const [owner, repo] = selectedRepo.split('/');
        detectionStatus = 'loading';

        try {
            const params = new URLSearchParams({
                owner,
                repo,
                branch: selectedBranch,
            });

            if (pathFilter) {
                params.append('path_filter', pathFilter);
            }

            if (workflowPath && workflowPath !== '.github/workflows/') {
                params.append('workflow_path', workflowPath);
            }

            const response = await fetch(`${API_BASE}/github/detect-project?${params}`);
            const data = await response.json();

            if (response.ok) {
                detectedConfig = data;
                configToml = configToToml(data);
                detectionStatus = 'success';
            } else {
                detectedConfig = null;
                detectionStatus = 'failed';
                showAdvanced = true;
            }
        } catch (error) {
            console.error('프로젝트 감지 실패:', error);
            detectedConfig = null;
            detectionStatus = 'failed';
            showAdvanced = true;
        }
    }

    // Convert config object to TOML string
    function configToToml(config) {
        return `# 빌드 설정
build_image = "${config.build_image || ''}"
build_command = "${config.build_command || ''}"

# 실행 설정
runtime_image = "${config.runtime_image || ''}"
runtime_command = "${config.runtime_command || ''}"
health_check_url = "${config.health_check_url || ''}"
runtime_port = "${config.runtime_port || 8080}"`;
    }

    // Parse TOML string to config object (simple parser)
    function tomlToConfig(toml) {
        try {
            const config = {};
            const lines = toml.split('\n');
            for (const line of lines) {
                const trimmed = line.trim();
                if (!trimmed || trimmed.startsWith('#')) continue;

                const match = trimmed.match(/^(\w+)\s*=\s*"([^"]*)"\s*$/);
                if (match) {
                    config[match[1]] = match[2];
                }
            }

            // Validate required fields
            const required = ['build_image', 'build_command', 'runtime_image'];
            for (const field of required) {
                if (!config[field]) {
                    throw new Error(`필수 필드 누락: ${field}`);
                }
            }

            return config;
        } catch (error) {
            throw new Error(`TOML 파싱 오류: ${error.message}`);
        }
    }

    async function registerProject() {
        if (!projectName.trim() || !selectedRepo || !selectedBranch || (!detectedConfig && !showAdvanced)) {
            return;
        }

        let config;

        if (showAdvanced) {
            // Parse TOML
            try {
                config = tomlToConfig(configToml);
                tomlError = '';
            } catch (error) {
                tomlError = error.message;
                return;
            }
        } else {
            config = detectedConfig;
        }

        const projectData = {
            name: projectName,
            repo: `https://github.com/${selectedRepo}.git`,
            path_filter: pathFilter || '*',
            branch: selectedBranch,
            build_image: config.build_image,
            build_command: config.build_command,
            cache_type: config.cache_type || 'none',
            working_directory: config.working_directory || null,
            runtime_image: config.runtime_image,
            runtime_command: config.runtime_command || '',
            health_check_url: config.health_check_url || '/',
            runtime_port: config.runtime_port || 8080,
        };

        try {
            const response = await fetch(`${API_BASE}/projects`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(projectData),
            });

            if (response.ok) {
                push('/');
            }
        } catch (error) {
            console.error(error);
        }
    }
</script>

<div class="container">
    <h1>프로젝트 등록</h1>

    <!-- GitHub PAT Section -->
    <section class="pat-section">
        <h2>GitHub 연동</h2>
        {#if patConfigured}
            <div class="status-badge connected">
                ✓ 연결됨 ({githubUsername})
            </div>
        {:else}
            <div class="status-badge disconnected">
                × 연결 안됨
            </div>
            <div class="input-group">
                <input
                    type="password"
                    bind:value={githubPAT}
                    placeholder="GitHub Personal Access Token"
                    class="input-full"
                />
                <button on:click={savePAT} class="btn-primary">PAT 저장</button>
            </div>
            <p class="help-text">
                <a href="https://github.com/settings/tokens/new?scopes=repo,read:user" target="_blank">
                    GitHub PAT 생성하기 →
                </a>
            </p>
        {/if}
    </section>

    {#if patConfigured}
        <!-- Project Setup Section -->
        <section class="project-section">
            <h2>프로젝트 설정</h2>

            <!-- Project Name -->
            <div class="form-group">
                <label>프로젝트 이름</label>
                <input
                    type="text"
                    bind:value={projectName}
                    placeholder="my-awesome-app"
                    class="input-short"
                />
            </div>

            <!-- Repository Selection -->
            <div class="form-group">
                <label>레포지토리</label>
                <select bind:value={selectedRepo} on:change={onRepoChange} class="select-full">
                    <option value="">레포지토리 선택...</option>
                    {#each repositories as repo}
                        <option value={repo.full_name}>
                            {repo.full_name} {repo.private ? '🔒' : ''}
                        </option>
                    {/each}
                </select>
            </div>

            <!-- Branch Selection -->
            {#if branches.length > 0}
                <div class="form-group">
                    <label>브랜치</label>
                    <select bind:value={selectedBranch} on:change={onBranchChange} class="select-medium">
                        <option value="">브랜치 선택...</option>
                        {#each branches as branch}
                            <option value={branch.name}>
                                {branch.name} {branch.protected ? '🛡️' : ''}
                            </option>
                        {/each}
                    </select>
                </div>
            {/if}

            <!-- Path Filter (Optional) -->
            <div class="form-group">
                <label>경로 필터 (선택사항)</label>
                <input
                    type="text"
                    bind:value={pathFilter}
                    placeholder="backend/ 또는 frontend/ (모노레포용)"
                    class="input-medium"
                />
                <p class="help-text">비워두면 전체 레포지토리 대상</p>
            </div>

            <!-- Workflow Path (Optional) -->
            <div class="form-group">
                <label>워크플로우 경로 (선택사항)</label>
                <input
                    type="text"
                    bind:value={workflowPath}
                    placeholder=".github/workflows/"
                    class="input-medium"
                />
                <p class="help-text">GitHub Actions 워크플로우가 다른 위치에 있는 경우 수정</p>
            </div>

            <!-- Auto-detect Button with Status -->
            {#if selectedRepo && selectedBranch}
                <div class="detect-container">
                    <button on:click={detectProject} class="btn-detect" disabled={detectionStatus === 'loading'}>
                        🔍 자동 감지
                    </button>
                    {#if detectionStatus === 'idle'}
                        <span class="status-icon idle">○</span>
                    {:else if detectionStatus === 'loading'}
                        <span class="status-icon loading">⟳</span>
                    {:else if detectionStatus === 'success'}
                        <span class="status-icon success">✓</span>
                    {:else if detectionStatus === 'failed'}
                        <span class="status-icon failed">✗</span>
                    {/if}
                </div>
            {/if}

            <!-- Detected Configuration Display -->
            {#if detectedConfig}
                <div class="detected-config">
                    <h3>✓ 감지된 설정</h3>
                    <div class="config-item">
                        <strong>프로젝트 타입:</strong> {detectedConfig.project_type}
                    </div>
                    <div class="config-item">
                        <strong>빌드 이미지:</strong> {detectedConfig.build_image}
                    </div>
                    <div class="config-item">
                        <strong>빌드 명령어:</strong> {detectedConfig.build_command}
                    </div>
                    <div class="config-item">
                        <strong>실행 이미지:</strong> {detectedConfig.runtime_image}
                    </div>

                    <button on:click={() => showAdvanced = !showAdvanced} class="btn-toggle">
                        {showAdvanced ? '▼ 고급 설정 숨기기' : '▶ 고급 설정 보기'}
                    </button>
                </div>
            {/if}

            <!-- Advanced Settings (TOML format) -->
            {#if showAdvanced}
                <div class="advanced-section">
                    <h3>고급 설정</h3>
                    <p class="help-text">
                        YML처럼 간단한 형식으로 설정을 수정할 수 있습니다. 주석(#)도 사용 가능합니다.
                    </p>
                    <textarea
                        bind:value={configToml}
                        class="config-textarea"
                        rows="9"
                        placeholder={tomlPlaceholder}
                    ></textarea>
                    {#if tomlError}
                        <div class="error-message">{tomlError}</div>
                    {/if}
                    <div class="help-text" style="margin-top: 0.5rem;">
                        <strong>예시:</strong><br>
                        <code>build_image</code>: 빌드할 Docker 이미지 (예: node:20, python:3.11)<br>
                        <code>build_command</code>: 빌드 명령어<br>
                        <code>runtime_image</code>: 실행할 Docker 이미지<br>
                        <code>runtime_command</code>: 실행 명령어<br>
                        <code>health_check_url</code>: 헬스체크 경로
                    </div>
                </div>
            {/if}

            <!-- Register Button -->
            {#if detectedConfig || showAdvanced}
                <div class="actions">
                    <button on:click={registerProject} class="btn-success">
                        프로젝트 등록
                    </button>
                    <button on:click={() => push('/')} class="btn-secondary">
                        취소
                    </button>
                </div>
            {/if}
        </section>
    {/if}
</div>

<style>
    .container {
        max-width: 800px;
        margin: 2rem auto;
        padding: 0 1rem;
    }

    h1 {
        font-size: 2rem;
        margin-bottom: 2rem;
        color: var(--gray-900);
    }

    h2 {
        font-size: 1.5rem;
        margin-bottom: 1rem;
        color: var(--gray-800);
    }

    h3 {
        font-size: 1.25rem;
        margin-bottom: 1rem;
        color: var(--gray-700);
    }

    section {
        background: white;
        padding: 1.5rem;
        border-radius: 0.5rem;
        box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
        margin-bottom: 2rem;
    }

    .status-badge {
        display: inline-block;
        padding: 0.5rem 1rem;
        border-radius: 0.375rem;
        font-weight: 500;
        margin-bottom: 1rem;
    }

    .status-badge.connected {
        background: #d1fae5;
        color: #065f46;
    }

    .status-badge.disconnected {
        background: #fee2e2;
        color: #991b1b;
    }

    .form-group {
        margin-bottom: 1.5rem;
    }

    label {
        display: block;
        font-weight: 500;
        margin-bottom: 0.5rem;
        color: var(--gray-700);
    }

    input, select {
        padding: 0.5rem;
        border: 1px solid var(--gray-300);
        border-radius: 0.375rem;
        font-size: 1rem;
    }

    .input-full, .select-full {
        width: 100%;
    }

    .input-medium, .select-medium {
        width: 60%;
    }

    .input-short, .select-short {
        width: 40%;
    }

    .input-group {
        display: flex;
        gap: 0.5rem;
        margin-bottom: 1rem;
    }

    .help-text {
        font-size: 0.875rem;
        color: var(--gray-600);
        margin-top: 0.25rem;
    }

    .help-text a {
        color: var(--primary);
        text-decoration: none;
    }

    .help-text a:hover {
        text-decoration: underline;
    }

    button {
        padding: 0.5rem 1rem;
        border: none;
        border-radius: 0.375rem;
        font-weight: 500;
        cursor: pointer;
        transition: all 0.2s;
    }

    .btn-primary {
        background: var(--primary);
        color: white;
    }

    .btn-primary:hover {
        background: var(--primary-dark);
    }

    .detect-container {
        display: flex;
        align-items: center;
        gap: 1rem;
        margin: 1rem 0;
    }

    .btn-detect {
        background: #3b82f6;
        color: white;
        font-size: 1.125rem;
        padding: 0.75rem 1.5rem;
    }

    .btn-detect:hover:not(:disabled) {
        background: #2563eb;
    }

    .btn-detect:disabled {
        opacity: 0.6;
        cursor: not-allowed;
    }

    .status-icon {
        font-size: 1.5rem;
        font-weight: bold;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 2rem;
        height: 2rem;
        border-radius: 50%;
    }

    .status-icon.idle {
        color: #9ca3af;
        border: 2px solid #9ca3af;
    }

    .status-icon.loading {
        color: #3b82f6;
        animation: spin 1s linear infinite;
    }

    .status-icon.success {
        color: #10b981;
        background: #d1fae5;
        border: 2px solid #10b981;
    }

    .status-icon.failed {
        color: #ef4444;
        background: #fee2e2;
        border: 2px solid #ef4444;
    }

    @keyframes spin {
        from {
            transform: rotate(0deg);
        }
        to {
            transform: rotate(360deg);
        }
    }

    .btn-toggle {
        background: var(--gray-200);
        color: var(--gray-700);
        margin-top: 1rem;
    }

    .btn-toggle:hover {
        background: var(--gray-300);
    }

    .btn-success {
        background: #10b981;
        color: white;
        font-size: 1.125rem;
        padding: 0.75rem 2rem;
    }

    .btn-success:hover {
        background: #059669;
    }

    .btn-secondary {
        background: var(--gray-300);
        color: var(--gray-700);
        padding: 0.75rem 2rem;
    }

    .btn-secondary:hover {
        background: var(--gray-400);
    }

    .detected-config {
        background: #f0fdf4;
        border: 2px solid #10b981;
        border-radius: 0.5rem;
        padding: 1.5rem;
        margin: 1.5rem 0;
    }

    .config-item {
        padding: 0.5rem 0;
        border-bottom: 1px solid #d1fae5;
    }

    .config-item:last-child {
        border-bottom: none;
    }

    .advanced-section {
        background: var(--gray-50);
        padding: 1.5rem;
        border-radius: 0.5rem;
        margin-top: 1.5rem;
    }

    .actions {
        display: flex;
        gap: 1rem;
        margin-top: 2rem;
        justify-content: center;
    }

    .config-textarea {
        width: 100%;
        font-family: 'Courier New', monospace;
        font-size: 0.875rem;
        padding: 1rem;
        border: 1px solid var(--gray-300);
        border-radius: 0.375rem;
        background: #f9fafb;
        resize: vertical;
    }

    .config-textarea:focus {
        outline: none;
        border-color: var(--primary);
        box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.1);
    }

    .error-message {
        margin-top: 0.5rem;
        padding: 0.75rem;
        background: #fee2e2;
        color: #991b1b;
        border-radius: 0.375rem;
        font-size: 0.875rem;
    }
</style>
