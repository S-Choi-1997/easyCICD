<script>
    import { onMount } from 'svelte';
    import { link, push } from 'svelte-spa-router';

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
    let workingDirectory = '';
    let runtimePort = '';
    let runtimePortPlaceholder = '8080';
    const tomlPlaceholder = `# 빌드 설정
build_image = "node:20"
build_command = "npm install && npm run build"
working_directory = ""

# 실행 설정
runtime_image = "nginx:alpine"
runtime_command = "nginx -g 'daemon off;'"
runtime_port = "8080"
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

    async function deletePAT() {
        if (!confirm('GitHub PAT를 삭제하시겠습니까? 레포지토리 목록을 다시 불러올 수 없게 됩니다.')) {
            return;
        }

        try {
            const response = await fetch(`${API_BASE}/settings/github-pat`, {
                method: 'DELETE',
            });

            if (response.ok) {
                patConfigured = false;
                githubUsername = '';
                repositories = [];
                branches = [];
                selectedRepo = '';
                selectedBranch = '';
                detectedConfig = null;
            }
        } catch (error) {
            console.error('PAT 삭제 실패:', error);
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

            if (workingDirectory) {
                params.append('path_filter', workingDirectory);
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
                // Update runtime port placeholder with detected value
                if (data.runtime_port) {
                    runtimePortPlaceholder = String(data.runtime_port);
                }
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
working_directory = "${config.working_directory || ''}"

# 실행 설정
runtime_image = "${config.runtime_image || ''}"
runtime_command = "${config.runtime_command || ''}"
runtime_port = "${config.runtime_port || 8080}"
health_check_url = "${config.health_check_url || ''}"`;
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
            working_directory: workingDirectory || config.working_directory || null,
            runtime_image: config.runtime_image,
            runtime_command: config.runtime_command || '',
            health_check_url: config.health_check_url || '/',
            runtime_port: runtimePort ? parseInt(runtimePort) : (config.runtime_port || parseInt(runtimePortPlaceholder) || 8080),
        };

        try {
            const response = await fetch(`${API_BASE}/projects`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(projectData),
            });

            if (response.ok) {
                const newProject = await response.json();

                // 프로젝트 등록 성공 시 자동으로 첫 빌드 트리거
                try {
                    await fetch(`${API_BASE}/projects/${newProject.id}/builds`, {
                        method: 'POST'
                    });
                } catch (buildError) {
                    console.error('자동 빌드 시작 실패:', buildError);
                }

                // 대시보드로 이동
                push('/');
            }
        } catch (error) {
            console.error(error);
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
        </div>
    </div>
</header>

<div class="container">
    <div class="card">
        <div class="card-header">
            <h2 class="card-title">프로젝트 등록</h2>
        </div>

        <div class="form-content">
            <!-- GitHub PAT Section -->
            <div class="section-box">
                <div class="section-header">
                    <h3>GitHub 연동</h3>
                    {#if patConfigured}
                        <div class="pat-connected-section">
                            <span class="status-badge connected">✓ 연결됨 ({githubUsername})</span>
                            <button on:click={deletePAT} class="btn btn-danger btn-sm">PAT 삭제</button>
                        </div>
                    {/if}
                </div>
                {#if !patConfigured}
                    <span class="status-badge disconnected">× 연결 안됨</span>
                    <div class="input-group">
                        <input
                            type="password"
                            bind:value={githubPAT}
                            placeholder="GitHub Personal Access Token"
                            class="form-input"
                        />
                        <button on:click={savePAT} class="btn btn-primary">PAT 저장</button>
                    </div>
                    <p class="form-help">
                        <a href="https://github.com/settings/tokens/new?scopes=repo,read:user" target="_blank">
                            GitHub PAT 생성하기 →
                        </a>
                    </p>
                {/if}
            </div>

            {#if patConfigured}
                <!-- Project Setup Section -->
                <div class="section-box">
                    <h3>프로젝트 설정</h3>

                    <!-- Project Name -->
                    <div class="form-group">
                        <label for="projectName">프로젝트 이름 *</label>
                        <input
                            type="text"
                            id="projectName"
                            bind:value={projectName}
                            placeholder="my-awesome-app"
                            class="form-input"
                        />
                        <span class="form-help">프로젝트를 구분할 이름입니다.</span>
                    </div>

                    <!-- Repository Selection -->
                    <div class="form-group">
                        <label for="repo">레포지토리 *</label>
                        <select id="repo" bind:value={selectedRepo} on:change={onRepoChange} class="form-input">
                            <option value="">레포지토리 선택...</option>
                            {#each repositories as repo}
                                <option value={repo.full_name}>
                                    {repo.full_name} {repo.private ? '🔒' : ''}
                                </option>
                            {/each}
                        </select>
                    </div>

                    <!-- Branch Selection -->
                    <div class="form-group">
                        <label for="branch">브랜치 *</label>
                        <select id="branch" bind:value={selectedBranch} on:change={onBranchChange} class="form-input" disabled={branches.length === 0}>
                            <option value="">브랜치 선택...</option>
                            {#each branches as branch}
                                <option value={branch.name}>
                                    {branch.name} {branch.protected ? '🛡️' : ''}
                                </option>
                            {/each}
                        </select>
                        {#if branches.length === 0 && selectedRepo}
                            <span class="form-help">레포지토리를 선택하면 브랜치 목록이 로드됩니다</span>
                        {/if}
                    </div>

                    <!-- Working Directory (Optional) -->
                    <div class="form-group">
                        <label for="workingDir">빌드 실행 디렉토리 (선택)</label>
                        <input
                            type="text"
                            id="workingDir"
                            bind:value={workingDirectory}
                            placeholder="예: packages/backend"
                            class="form-input"
                        />
                        <span class="form-help">
                            레포지토리 안에 여러 프로젝트가 있을 때 사용합니다.<br>
                            예를 들어 frontend/, backend/ 폴더가 있다면 backend를 입력하세요.<br>
                            비워두면 레포지토리 최상위에서 빌드합니다.
                        </span>
                    </div>

                    <!-- Path Filter (Optional) -->
                    <div class="form-group">
                        <label for="pathFilter">빌드 트리거 경로 (선택)</label>
                        <input
                            type="text"
                            id="pathFilter"
                            bind:value={pathFilter}
                            placeholder="예: backend/** 또는 src/**"
                            class="form-input"
                        />
                        <span class="form-help">
                            이 경로의 파일이 변경될 때만 자동 빌드됩니다.<br>
                            비워두면 어떤 파일이 바뀌어도 빌드가 실행됩니다.
                        </span>
                    </div>

                    <!-- Workflow Path (Optional) -->
                    <div class="form-group">
                        <label for="workflowPath">워크플로우 파일 경로 (선택)</label>
                        <input
                            type="text"
                            id="workflowPath"
                            bind:value={workflowPath}
                            placeholder=".github/workflows/"
                            class="form-input"
                        />
                        <span class="form-help">
                            GitHub Actions 설정 파일이 있는 폴더입니다.<br>
                            대부분의 경우 기본값을 그대로 사용하면 됩니다.
                        </span>
                    </div>

                    <!-- Runtime Port -->
                    <div class="form-group">
                        <label for="port">애플리케이션 포트 (선택)</label>
                        <input
                            type="number"
                            id="port"
                            bind:value={runtimePort}
                            placeholder={runtimePortPlaceholder}
                            class="form-input"
                            style="width: 120px;"
                            min="1"
                            max="65535"
                        />
                        <span class="form-help">
                            앱이 실행될 때 사용하는 포트 번호입니다.<br>
                            모르겠다면 비워두세요. 자동으로 감지합니다.
                        </span>
                    </div>

                    <!-- Auto-detect Button with Status -->
                    {#if selectedRepo && selectedBranch}
                        <div class="detect-container">
                            <button on:click={detectProject} class="btn btn-primary" disabled={detectionStatus === 'loading'}>
                                {detectionStatus === 'loading' ? '감지 중...' : '🔍 자동 감지'}
                            </button>
                            {#if detectionStatus === 'success'}
                                <span class="status-icon success">✓</span>
                            {:else if detectionStatus === 'failed'}
                                <span class="status-icon failed">✗</span>
                            {/if}
                        </div>
                    {/if}

                    <!-- Detected Configuration Display -->
                    {#if detectedConfig}
                        <div class="detected-config">
                            <h4>✓ 감지된 설정</h4>
                            <div class="config-grid">
                                <div class="config-item">
                                    <span class="config-label">프로젝트 타입</span>
                                    <span class="config-value">{detectedConfig.project_type}</span>
                                </div>
                                <div class="config-item">
                                    <span class="config-label">빌드 이미지</span>
                                    <span class="config-value">{detectedConfig.build_image}</span>
                                </div>
                                <div class="config-item">
                                    <span class="config-label">빌드 명령어</span>
                                    <span class="config-value">{detectedConfig.build_command}</span>
                                </div>
                                <div class="config-item">
                                    <span class="config-label">실행 이미지</span>
                                    <span class="config-value">{detectedConfig.runtime_image}</span>
                                </div>
                            </div>
                            <button on:click={() => showAdvanced = !showAdvanced} class="btn btn-secondary btn-sm" style="margin-top: 1rem;">
                                {showAdvanced ? '▼ 고급 설정 숨기기' : '▶ 고급 설정 보기'}
                            </button>
                        </div>
                    {/if}

                    <!-- Advanced Settings (TOML format) -->
                    {#if showAdvanced}
                        <div class="advanced-section">
                            <h4>고급 설정</h4>
                            <span class="form-help" style="margin-bottom: 0.75rem; display: block;">
                                설정을 직접 수정할 수 있습니다. 주석(#)도 사용 가능합니다.
                            </span>
                            <textarea
                                bind:value={configToml}
                                class="form-input config-textarea"
                                rows="9"
                                placeholder={tomlPlaceholder}
                            ></textarea>
                            {#if tomlError}
                                <div class="error-message">{tomlError}</div>
                            {/if}
                        </div>
                    {/if}

                    <!-- Register Button -->
                    {#if detectedConfig || showAdvanced}
                        <div class="form-actions">
                            <button on:click={() => push('/')} class="btn btn-secondary">취소</button>
                            <button on:click={registerProject} class="btn btn-primary">프로젝트 등록</button>
                        </div>
                    {/if}
                </div>
            {/if}
        </div>
    </div>
</div>

<style>
    /* Container - 다른 페이지와 통일 */
    .container {
        max-width: 800px;
        margin: 0 auto;
        padding: 2rem 1rem;
    }

    /* Card - app.css 기반 */
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

    /* Section Box */
    .section-box {
        margin-bottom: 2rem;
        padding-bottom: 1.5rem;
        border-bottom: 1px solid #e5e7eb;
    }

    .section-box:last-child {
        margin-bottom: 0;
        padding-bottom: 0;
        border-bottom: none;
    }

    .section-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 1rem;
    }

    .section-header h3 {
        margin: 0;
    }

    h3 {
        font-size: 1.125rem;
        font-weight: 600;
        margin-bottom: 1rem;
        color: #111827;
    }

    h4 {
        font-size: 1rem;
        font-weight: 600;
        margin-bottom: 0.75rem;
        color: #111827;
    }

    /* Form Elements */
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

    select.form-input:disabled {
        background-color: #f3f4f6;
        color: #6b7280;
        cursor: not-allowed;
    }

    .form-help {
        font-size: 0.75rem;
        color: #6b7280;
        margin-top: 0.25rem;
        display: block;
    }

    .form-help a {
        color: #3b82f6;
        text-decoration: none;
    }

    .form-help a:hover {
        text-decoration: underline;
    }

    /* Input Group */
    .input-group {
        display: flex;
        gap: 0.5rem;
        margin-bottom: 0.5rem;
    }

    .input-group .form-input {
        flex: 1;
    }

    /* Status Badge */
    .status-badge {
        display: inline-block;
        padding: 0.25rem 0.5rem;
        border-radius: 0.25rem;
        font-weight: 500;
        font-size: 0.75rem;
    }

    .status-badge.connected {
        background: #d1fae5;
        color: #065f46;
    }

    .status-badge.disconnected {
        background: #fee2e2;
        color: #991b1b;
        margin-bottom: 0.75rem;
    }

    .pat-connected-section {
        display: flex;
        align-items: center;
        gap: 0.75rem;
    }

    /* Detect Container */
    .detect-container {
        display: flex;
        align-items: center;
        gap: 0.75rem;
        margin: 1.5rem 0;
        padding: 1rem;
        background: #f9fafb;
        border-radius: 0.375rem;
    }

    .status-icon {
        font-size: 1.25rem;
        font-weight: bold;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 1.75rem;
        height: 1.75rem;
        border-radius: 50%;
    }

    .status-icon.success {
        color: #10b981;
        background: #d1fae5;
    }

    .status-icon.failed {
        color: #ef4444;
        background: #fee2e2;
    }

    /* Detected Config */
    .detected-config {
        background: #f0fdf4;
        border: 1px solid #10b981;
        border-radius: 0.5rem;
        padding: 1.25rem;
        margin: 1.5rem 0;
    }

    .detected-config h4 {
        color: #065f46;
        margin-bottom: 1rem;
    }

    .config-grid {
        display: grid;
        gap: 0.5rem;
    }

    .config-item {
        display: flex;
        padding: 0.5rem;
        background: white;
        border-radius: 0.25rem;
    }

    .config-label {
        font-weight: 500;
        color: #374151;
        min-width: 100px;
        font-size: 0.813rem;
    }

    .config-value {
        color: #111827;
        font-family: monospace;
        font-size: 0.813rem;
        word-break: break-all;
    }

    /* Advanced Section */
    .advanced-section {
        background: #f9fafb;
        padding: 1.25rem;
        border-radius: 0.5rem;
        margin-top: 1.5rem;
        border: 1px solid #e5e7eb;
    }

    .config-textarea {
        font-family: 'Courier New', monospace;
        resize: vertical;
        min-height: 180px;
    }

    /* Form Actions */
    .form-actions {
        display: flex;
        justify-content: flex-end;
        gap: 0.75rem;
        margin-top: 1.5rem;
        padding-top: 1.5rem;
        border-top: 1px solid #e5e7eb;
    }

    /* Error Message */
    .error-message {
        margin-top: 0.5rem;
        padding: 0.75rem 1rem;
        background: #fee2e2;
        color: #991b1b;
        border-radius: 0.375rem;
        font-size: 0.875rem;
    }
</style>
