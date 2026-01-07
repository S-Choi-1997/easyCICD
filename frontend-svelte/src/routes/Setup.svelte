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

    // Data from API
    let repositories = [];
    let branches = [];

    // Auto-detected configuration
    let detectedConfig = null;
    let showAdvanced = false;

    // Manual overrides (when user wants to customize)
    let manualConfig = {
        build_image: '',
        build_command: '',
        cache_type: '',
        runtime_image: '',
        runtime_command: '',
        health_check_url: ''
    };

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
            alert('GitHub PAT을 입력하세요.');
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
                alert(`PAT 저장 완료! (${githubUsername})`);
                await loadRepositories();
            } else {
                alert(`PAT 저장 실패: ${data.error}`);
            }
        } catch (error) {
            alert('PAT 저장 중 오류 발생');
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

        try {
            const params = new URLSearchParams({
                owner,
                repo,
                branch: selectedBranch,
            });

            if (pathFilter) {
                params.append('path_filter', pathFilter);
            }

            const response = await fetch(`${API_BASE}/github/detect-project?${params}`);
            const data = await response.json();

            if (response.ok) {
                detectedConfig = data;
                // Initialize manual config with detected values
                manualConfig = { ...data };
                alert(`프로젝트 타입 감지 완료: ${data.project_type}`);
            } else {
                alert(`자동 감지 실패: ${data.error}\n수동으로 설정하세요.`);
                showAdvanced = true;
            }
        } catch (error) {
            console.error('프로젝트 감지 실패:', error);
            alert('프로젝트 감지 중 오류 발생');
        }
    }

    async function registerProject() {
        if (!projectName.trim()) {
            alert('프로젝트 이름을 입력하세요.');
            return;
        }

        if (!selectedRepo || !selectedBranch) {
            alert('레포지토리와 브랜치를 선택하세요.');
            return;
        }

        if (!detectedConfig && !showAdvanced) {
            alert('먼저 자동 감지를 실행하세요.');
            return;
        }

        const config = showAdvanced ? manualConfig : detectedConfig;

        const projectData = {
            name: projectName,
            repo: `https://github.com/${selectedRepo}.git`,
            path_filter: pathFilter || '*',
            branch: selectedBranch,
            build_image: config.build_image,
            build_command: config.build_command,
            cache_type: config.cache_type,
            runtime_image: config.runtime_image,
            runtime_command: config.runtime_command,
            health_check_url: config.health_check_url,
        };

        try {
            const response = await fetch(`${API_BASE}/projects`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(projectData),
            });

            if (response.ok) {
                alert('프로젝트 등록 완료!');
                push('/');
            } else {
                const data = await response.json();
                alert(`프로젝트 등록 실패: ${data.error || '알 수 없는 오류'}`);
            }
        } catch (error) {
            alert('프로젝트 등록 중 오류 발생');
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

            <!-- Auto-detect Button -->
            {#if selectedRepo && selectedBranch}
                <button on:click={detectProject} class="btn-detect">
                    🔍 자동 감지
                </button>
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

            <!-- Advanced Settings -->
            {#if showAdvanced}
                <div class="advanced-section">
                    <h3>고급 설정 (수동 조정)</h3>

                    <div class="form-group">
                        <label>빌드 이미지</label>
                        <input type="text" bind:value={manualConfig.build_image} class="input-full" />
                    </div>

                    <div class="form-group">
                        <label>빌드 명령어</label>
                        <input type="text" bind:value={manualConfig.build_command} class="input-full" />
                    </div>

                    <div class="form-group">
                        <label>캐시 타입</label>
                        <select bind:value={manualConfig.cache_type} class="select-short">
                            <option value="none">없음</option>
                            <option value="gradle">Gradle</option>
                            <option value="maven">Maven</option>
                            <option value="npm">npm</option>
                            <option value="pip">pip</option>
                            <option value="rust">Rust</option>
                            <option value="go">Go</option>
                        </select>
                    </div>

                    <div class="form-group">
                        <label>실행 이미지</label>
                        <input type="text" bind:value={manualConfig.runtime_image} class="input-full" />
                    </div>

                    <div class="form-group">
                        <label>실행 명령어</label>
                        <input type="text" bind:value={manualConfig.runtime_command} class="input-full" />
                    </div>

                    <div class="form-group">
                        <label>헬스체크 URL</label>
                        <input type="text" bind:value={manualConfig.health_check_url} class="input-short" />
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

    .btn-detect {
        background: #3b82f6;
        color: white;
        font-size: 1.125rem;
        padding: 0.75rem 1.5rem;
        margin: 1rem 0;
    }

    .btn-detect:hover {
        background: #2563eb;
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
</style>
