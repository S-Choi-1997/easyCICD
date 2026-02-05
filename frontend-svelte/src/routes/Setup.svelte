<script>
    import { onMount } from 'svelte';
    import { link, push } from 'svelte-spa-router';

    const API_BASE = '/api';

    // GitHub PAT (multiple PAT support)
    let pats = [];
    let selectedPatId = null;
    let newPatLabel = '';
    let newPatToken = '';
    let showNewPatForm = false;
    let patSaving = false;

    // Discord webhooks
    let discordWebhooks = [];
    let selectedDiscordWebhookId = null;

    // Legacy PAT (backward compat)
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

    // Environment variables
    let buildEnvVars = '';
    let runtimeEnvVars = '';
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
        await loadPats();
        await loadDiscordWebhooks();
        if (pats.length > 0) {
            selectedPatId = pats[0].id;
            patConfigured = true;
            githubUsername = pats[0].github_username || '';
            await loadRepositories();
        } else {
            // Fallback: check legacy PAT
            await checkPATStatus();
            if (patConfigured) {
                await loadRepositories();
            }
        }
    });

    async function loadPats() {
        try {
            const response = await fetch(`${API_BASE}/github/pats`);
            const data = await response.json();
            pats = data.pats || [];
        } catch (error) {
            console.error('PAT 목록 로드 실패:', error);
        }
    }

    async function loadDiscordWebhooks() {
        try {
            const response = await fetch(`${API_BASE}/discord-webhooks`);
            const data = await response.json();
            discordWebhooks = (data.webhooks || []).filter(w => w.enabled);
        } catch (error) {
            console.error('Discord 웹훅 로드 실패:', error);
        }
    }

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

    async function createPat() {
        if (!newPatLabel.trim() || !newPatToken.trim()) return;
        patSaving = true;

        try {
            const response = await fetch(`${API_BASE}/github/pats`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ label: newPatLabel, token: newPatToken }),
            });

            const data = await response.json();
            if (response.ok) {
                await loadPats();
                selectedPatId = data.id;
                patConfigured = true;
                githubUsername = data.github_username || '';
                newPatLabel = '';
                newPatToken = '';
                showNewPatForm = false;
                await loadRepositories();
            } else {
                alert(data.error || 'PAT 저장 실패');
            }
        } catch (error) {
            console.error('PAT 생성 실패:', error);
        } finally {
            patSaving = false;
        }
    }

    async function deletePat(patId) {
        if (!confirm('이 PAT를 삭제하시겠습니까?')) return;

        try {
            const response = await fetch(`${API_BASE}/github/pats/${patId}`, {
                method: 'DELETE',
            });

            if (response.ok) {
                await loadPats();
                if (pats.length > 0) {
                    selectedPatId = pats[0].id;
                    githubUsername = pats[0].github_username || '';
                    await loadRepositories();
                } else {
                    selectedPatId = null;
                    patConfigured = false;
                    githubUsername = '';
                    repositories = [];
                    branches = [];
                    selectedRepo = '';
                    selectedBranch = '';
                    detectedConfig = null;
                }
            } else {
                const data = await response.json();
                alert(data.error || 'PAT 삭제 실패');
            }
        } catch (error) {
            console.error('PAT 삭제 실패:', error);
        }
    }

    async function onPatChange() {
        if (!selectedPatId) return;
        const pat = pats.find(p => p.id === selectedPatId);
        if (pat) {
            githubUsername = pat.github_username || '';
        }
        // 레포지토리 목록은 이미 모든 PAT의 합이므로 다시 로드할 필요 없음
        // 단, 브랜치 등 선택은 리셋
        branches = [];
        selectedRepo = '';
        selectedBranch = '';
        detectedConfig = null;
    }

    // 레포지토리별 PAT ID 매핑 (레포지토리가 어떤 PAT로 접근 가능한지 저장)
    let repoToPatMap = new Map();

    async function loadRepositories() {
        try {
            // 모든 PAT의 레포지토리를 합쳐서 가져오기
            const allRepos = new Map(); // full_name을 키로 사용하여 중복 제거
            repoToPatMap = new Map();

            if (pats.length === 0) {
                // PAT가 없으면 레거시 글로벌 PAT 사용
                const response = await fetch(`${API_BASE}/github/repositories`);
                const data = await response.json();
                (data.repositories || []).forEach(repo => {
                    allRepos.set(repo.full_name, repo);
                    repoToPatMap.set(repo.full_name, null); // legacy PAT
                });
            } else {
                // 모든 PAT에 대해 레포지토리 가져오기
                console.log(`총 ${pats.length}개의 PAT에서 레포지토리 로드 시작`);
                for (const pat of pats) {
                    console.log(`PAT "${pat.label}" (ID: ${pat.id})에서 레포지토리 로드 중...`);
                    try {
                        const response = await fetch(`${API_BASE}/github/repositories?pat_id=${pat.id}`);
                        if (!response.ok) {
                            const errorData = await response.json();
                            const errorMsg = errorData.error || `${response.status} ${response.statusText}`;
                            console.error(`PAT "${pat.label}" 오류: ${errorMsg}`, errorData.detail || '');
                            if (response.status === 401) {
                                console.warn(`⚠️ PAT "${pat.label}"이 유효하지 않거나 만료되었습니다. 이 PAT를 삭제하고 새로 생성하세요.`);
                            }
                            continue;
                        }
                        const data = await response.json();
                        const repoCount = (data.repositories || []).length;
                        console.log(`PAT "${pat.label}"에서 ${repoCount}개 레포지토리 받음`);
                        (data.repositories || []).forEach(repo => {
                            if (!allRepos.has(repo.full_name)) {
                                allRepos.set(repo.full_name, repo);
                                repoToPatMap.set(repo.full_name, pat.id);
                            }
                            // 이미 있으면 첫 번째 PAT 우선 (중복 시 먼저 발견된 PAT 사용)
                        });
                    } catch (error) {
                        console.error(`PAT ${pat.label} 레포지토리 로드 실패:`, error);
                    }
                }
            }

            // Map을 배열로 변환하고 updated_at 기준으로 정렬
            repositories = Array.from(allRepos.values()).sort((a, b) =>
                new Date(b.updated_at) - new Date(a.updated_at)
            );

            console.log(`총 ${repositories.length}개의 레포지토리 로드됨 (${pats.length}개 PAT)`);
        } catch (error) {
            console.error('레포지토리 로드 실패:', error);
        }
    }

    async function onRepoChange() {
        if (!selectedRepo) return;

        // 이 레포지토리에 접근 가능한 PAT 자동 선택
        const repoPatId = repoToPatMap.get(selectedRepo);
        if (repoPatId && repoPatId !== selectedPatId) {
            selectedPatId = repoPatId;
            const pat = pats.find(p => p.id === repoPatId);
            if (pat) {
                githubUsername = pat.github_username || '';
                console.log(`레포지토리 ${selectedRepo}에 대해 PAT "${pat.label}" 자동 선택`);
            }
        }

        const [owner, repo] = selectedRepo.split('/');
        try {
            const patParam = selectedPatId ? `&pat_id=${selectedPatId}` : '';
            const response = await fetch(
                `${API_BASE}/github/branches?owner=${owner}&repo=${repo}${patParam}`
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

            if (selectedPatId) {
                params.append('pat_id', selectedPatId);
            }

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

        // Parse environment variables (KEY=VALUE format, newline separated)
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

        const projectData = {
            name: projectName,
            repo: `https://github.com/${selectedRepo}.git`,
            path_filter: pathFilter || '*',
            branch: selectedBranch,
            build_image: config.build_image || 'node:20',
            build_command: config.build_command || 'npm install && npm run build',
            cache_type: config.cache_type || 'none',
            working_directory: workingDirectory || config.working_directory || null,
            build_env_vars: Object.keys(parseEnvVars(buildEnvVars)).length > 0
              ? JSON.stringify(parseEnvVars(buildEnvVars))
              : null,
            runtime_image: config.runtime_image || 'nginx:alpine',
            runtime_command: config.runtime_command || '',
            health_check_url: config.health_check_url || '/',
            runtime_port: parseInt(runtimePort || config.runtime_port || runtimePortPlaceholder) || 8080,
            runtime_env_vars: Object.keys(parseEnvVars(runtimeEnvVars)).length > 0
              ? JSON.stringify(parseEnvVars(runtimeEnvVars))
              : null,
            github_pat_id: selectedPatId || null,
            discord_webhook_id: selectedDiscordWebhookId || null,
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
                        <span class="status-badge connected">✓ 연결됨 ({githubUsername})</span>
                    {/if}
                </div>

                {#if pats.length > 0}
                    <!-- PAT Selector -->
                    <div class="form-group">
                        <label for="patSelect">PAT 선택</label>
                        <div class="pat-selector-row">
                            <select id="patSelect" bind:value={selectedPatId} on:change={onPatChange} class="form-input">
                                {#each pats as pat}
                                    <option value={pat.id}>
                                        {pat.label} ({pat.github_username || pat.token_preview})
                                    </option>
                                {/each}
                            </select>
                            <button on:click={() => deletePat(selectedPatId)} class="btn btn-danger btn-sm">삭제</button>
                        </div>
                    </div>
                {/if}

                <!-- New PAT Form Toggle -->
                {#if !showNewPatForm}
                    <button on:click={() => showNewPatForm = true} class="btn btn-secondary btn-sm">
                        + 새 PAT 추가
                    </button>
                {:else}
                    <div class="new-pat-form">
                        <div class="form-group">
                            <label for="patLabel">PAT 이름</label>
                            <input
                                type="text"
                                id="patLabel"
                                bind:value={newPatLabel}
                                placeholder="예: Team A PAT"
                                class="form-input"
                            />
                        </div>
                        <div class="form-group">
                            <label for="patToken">Token</label>
                            <input
                                type="password"
                                id="patToken"
                                bind:value={newPatToken}
                                placeholder="ghp_xxxxxxxxxxxx"
                                class="form-input"
                            />
                        </div>
                        <div class="pat-form-actions">
                            <button on:click={() => { showNewPatForm = false; newPatLabel = ''; newPatToken = ''; }} class="btn btn-secondary btn-sm">취소</button>
                            <button on:click={createPat} class="btn btn-primary btn-sm" disabled={patSaving}>
                                {patSaving ? '저장 중...' : 'PAT 저장'}
                            </button>
                        </div>
                        <p class="form-help">
                            <a href="https://github.com/settings/tokens/new?scopes=repo,read:user" target="_blank">
                                GitHub PAT 생성하기 →
                            </a>
                        </p>
                    </div>
                {/if}

                {#if pats.length === 0 && !showNewPatForm}
                    <span class="status-badge disconnected" style="margin-top: 0.5rem;">× 연결 안됨</span>
                    <p class="form-help">프로젝트를 등록하려면 GitHub PAT를 추가하세요.</p>
                {/if}
            </div>

            <!-- Discord Webhook Section (Optional) -->
            <div class="section-box">
                <div class="section-header">
                    <h3>Discord 알림 (선택사항)</h3>
                </div>

                {#if discordWebhooks.length > 0}
                    <div class="form-group">
                        <label for="discordWebhook">Discord 웹훅</label>
                        <select id="discordWebhook" bind:value={selectedDiscordWebhookId} class="form-input">
                            <option value={null}>알림 사용 안 함</option>
                            {#each discordWebhooks as webhook}
                                <option value={webhook.id}>
                                    {webhook.label}
                                </option>
                            {/each}
                        </select>
                        <span class="form-help">빌드 및 배포 상태를 Discord로 알림받습니다.</span>
                    </div>
                {:else}
                    <p class="form-help">
                        <a href="#/settings" use:link>설정</a>에서 Discord 웹훅을 먼저 등록하세요.
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
                            on:change={(e) => projectName = e.target.value}
                            placeholder="my-awesome-app"
                            class="form-input"
                            autocomplete="off"
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

                            <!-- Environment Variables -->
                            <h4 style="margin-top: 1.5rem;">환경변수 (선택)</h4>
                            <div class="form-group">
                                <label for="buildEnvVars">빌드 환경변수</label>
                                <textarea
                                    id="buildEnvVars"
                                    bind:value={buildEnvVars}
                                    class="form-input"
                                    rows="3"
                                    style="font-family: monospace; font-size: 0.875rem;"
                                    placeholder="NODE_ENV=production&#10;REACT_APP_API_URL=https://api.example.com"
                                ></textarea>
                                <span class="form-help">빌드 시 사용할 환경변수. 줄바꿈으로 구분, KEY=VALUE 형식</span>
                            </div>
                            <div class="form-group">
                                <label for="runtimeEnvVars">런타임 환경변수</label>
                                <textarea
                                    id="runtimeEnvVars"
                                    bind:value={runtimeEnvVars}
                                    class="form-input"
                                    rows="3"
                                    style="font-family: monospace; font-size: 0.875rem;"
                                    placeholder="NODE_ENV=production&#10;DATABASE_URL=postgres://..."
                                ></textarea>
                                <span class="form-help">앱 실행 시 사용할 환경변수. 줄바꿈으로 구분, KEY=VALUE 형식</span>
                            </div>
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

    .pat-selector-row {
        display: flex;
        gap: 0.5rem;
        align-items: center;
    }

    .pat-selector-row .form-input {
        flex: 1;
    }

    .new-pat-form {
        margin-top: 1rem;
        padding: 1rem;
        background: #f9fafb;
        border-radius: 0.375rem;
        border: 1px solid #e5e7eb;
    }

    .pat-form-actions {
        display: flex;
        gap: 0.5rem;
        margin-top: 0.75rem;
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
