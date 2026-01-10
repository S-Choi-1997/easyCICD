import { writable, derived } from 'svelte/store';

// 프로젝트 목록 스토어
export const projects = writable([]);
export const projectsLoading = writable(false);
export const projectsError = writable(null);

// 선택된 프로젝트
export const selectedProject = writable(null);

const API_BASE = '/api';

/**
 * 프로젝트 목록 로드
 */
export async function loadProjects() {
    projectsLoading.set(true);
    projectsError.set(null);

    try {
        const response = await fetch(`${API_BASE}/projects`);
        if (!response.ok) throw new Error('프로젝트 목록을 가져올 수 없습니다');

        const data = await response.json();
        projects.set(data);
    } catch (error) {
        console.error('[Projects] Load error:', error);
        projectsError.set(error.message);
    } finally {
        projectsLoading.set(false);
    }
}

/**
 * 프로젝트 상세 정보 로드
 */
export async function loadProject(projectId) {
    try {
        const response = await fetch(`${API_BASE}/projects/${projectId}`);
        if (!response.ok) throw new Error('프로젝트 정보를 가져올 수 없습니다');

        const data = await response.json();
        selectedProject.set(data);
        return data;
    } catch (error) {
        console.error('[Projects] Load project error:', error);
        throw error;
    }
}

/**
 * 빌드 트리거
 */
export async function triggerBuild(projectId) {
    try {
        const response = await fetch(`${API_BASE}/projects/${projectId}/builds`, {
            method: 'POST'
        });

        if (!response.ok) throw new Error('빌드를 시작할 수 없습니다');

        // 약간의 딜레이 후 프로젝트 목록 갱신
        setTimeout(() => loadProjects(), 500);

        return await response.json();
    } catch (error) {
        console.error('[Projects] Trigger build error:', error);
        throw error;
    }
}

/**
 * 프로젝트 삭제
 */
export async function deleteProject(projectId) {
    try {
        const response = await fetch(`${API_BASE}/projects/${projectId}`, {
            method: 'DELETE'
        });

        if (!response.ok) throw new Error('프로젝트를 삭제할 수 없습니다');

        // 프로젝트 목록 갱신
        await loadProjects();
    } catch (error) {
        console.error('[Projects] Delete error:', error);
        throw error;
    }
}

/**
 * 프로젝트 상태 업데이트 (WebSocket 메시지 처리)
 */
export function updateProjectFromWebSocket(data) {
    if (data.type === 'BuildStatus') {
        // 프로젝트 목록 갱신 (빌드 상태 변경 시)
        loadProjects();

        // 선택된 프로젝트 업데이트
        selectedProject.update(proj => {
            if (proj && proj.id === data.project_id) {
                return { ...proj, last_build_status: data.status };
            }
            return proj;
        });
    }
}
