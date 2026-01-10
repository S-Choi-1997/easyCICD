import { writable, derived } from 'svelte/store';

// 빌드 목록 (프로젝트 ID별로 저장)
export const builds = writable({});
export const buildsLoading = writable(false);
export const buildsError = writable(null);

// 선택된 빌드
export const selectedBuild = writable(null);

// 빌드 로그
export const buildLogs = writable([]);
export const isStreaming = writable(false);

const API_BASE = '/api';

/**
 * 빌드 목록 로드
 */
export async function loadBuilds(projectId, limit = 50) {
    buildsLoading.set(true);
    buildsError.set(null);

    try {
        const response = await fetch(`${API_BASE}/builds?project_id=${projectId}&limit=${limit}`);
        if (!response.ok) throw new Error('빌드 목록을 가져올 수 없습니다');

        const data = await response.json();

        // 프로젝트 ID별로 저장
        builds.update(allBuilds => ({
            ...allBuilds,
            [projectId]: data
        }));

        return data;
    } catch (error) {
        console.error('[Builds] Load error:', error);
        buildsError.set(error.message);
    } finally {
        buildsLoading.set(false);
    }
}

/**
 * 빌드 상세 정보 로드
 */
export async function loadBuild(buildId) {
    try {
        const response = await fetch(`${API_BASE}/builds/${buildId}`);
        if (!response.ok) throw new Error('빌드 정보를 가져올 수 없습니다');

        const data = await response.json();
        selectedBuild.set(data);

        // 로그 초기화
        buildLogs.set([]);

        // 진행 중이면 스트리밍 활성화
        const streaming = data.status === 'Building' || data.status === 'Deploying' || data.status === 'Queued';
        isStreaming.set(streaming);

        // 기존 로그 로드
        await loadBuildLogs(buildId);

        return data;
    } catch (error) {
        console.error('[Builds] Load build error:', error);
        throw error;
    }
}

/**
 * 빌드 로그 로드
 */
export async function loadBuildLogs(buildId) {
    try {
        const response = await fetch(`${API_BASE}/builds/${buildId}/logs`);
        if (response.ok) {
            const text = await response.text();
            if (text) {
                const lines = text.split('\n').filter(line => line.trim());
                buildLogs.set(lines);
            }
        }
    } catch (error) {
        console.error('[Builds] Load logs error:', error);
    }
}

/**
 * 로그 라인 추가 (WebSocket 메시지)
 */
export function appendLogLine(line) {
    buildLogs.update(logs => [...logs, line]);
}

/**
 * 빌드 상태 업데이트 (WebSocket 메시지 처리)
 */
export function updateBuildFromWebSocket(data) {
    if (data.type === 'Log') {
        // 로그 라인 추가
        appendLogLine(data.line);

        // 선택된 빌드의 로그면 스트리밍 활성화
        selectedBuild.subscribe(build => {
            if (build && build.id === data.build_id) {
                isStreaming.set(true);
            }
        })();
    } else if (data.type === 'BuildStatus') {
        // 빌드 상태 변경
        const { project_id, build_id, status } = data;

        // 빌드 목록에서 해당 빌드 업데이트
        builds.update(allBuilds => {
            if (allBuilds[project_id]) {
                return {
                    ...allBuilds,
                    [project_id]: allBuilds[project_id].map(build =>
                        build.id === build_id ? { ...build, status } : build
                    )
                };
            }
            return allBuilds;
        });

        // 선택된 빌드 업데이트
        selectedBuild.update(build => {
            if (build && build.id === build_id) {
                // 완료 상태면 스트리밍 종료
                if (status === 'Success' || status === 'Failed') {
                    isStreaming.set(false);
                }
                return { ...build, status };
            }
            return build;
        });

        // 프로젝트의 빌드 목록 갱신
        if (project_id) {
            loadBuilds(project_id);
        }
    }
}

/**
 * 빌드 프로그레스 계산 (Derived Store)
 */
export const buildProgress = derived(selectedBuild, $selectedBuild => {
    if (!$selectedBuild) return { stage: null, progress: 0 };

    const { status } = $selectedBuild;

    switch (status) {
        case 'Queued':
            return { stage: '대기 중', progress: 0 };
        case 'Building':
            return { stage: '빌드 중', progress: 40 };
        case 'Deploying':
            return { stage: '배포 중', progress: 80 };
        case 'Success':
            return { stage: '완료', progress: 100 };
        case 'Failed':
            return { stage: '실패', progress: 100 };
        default:
            return { stage: null, progress: 0 };
    }
});
