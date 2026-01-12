<script>
  import { onMount } from 'svelte';
  import Router from 'svelte-spa-router';
  import Dashboard from './routes/Dashboard.svelte';
  import Setup from './routes/Setup.svelte';
  import BuildHistory from './routes/BuildHistory.svelte';
  import ProjectDetail from './routes/ProjectDetail.svelte';
  import Settings from './routes/Settings.svelte';
  import ContainerNew from './routes/ContainerNew.svelte';
  import { initWebSocket, subscribe } from './stores/websocket';
  import { updateProjectFromWebSocket } from './stores/projects';
  import { updateBuildFromWebSocket } from './stores/builds';
  import './app.css';

  const routes = {
    '/': Dashboard,
    '/setup': Setup,
    '/build/:id': BuildHistory,
    '/project/:id': ProjectDetail,
    '/settings': Settings,
    '/containers/new': ContainerNew,
  };

  onMount(() => {
    // 버전 출력 (디버깅용)
    console.log('🚀 EasyCI/CD Frontend v2.0.8 - Container state sync debugging complete');
    console.log('Build timestamp:', new Date().toISOString());

    // 전역 WebSocket 초기화
    initWebSocket();

    // 전역 WebSocket 메시지 처리
    const unsubscribe = subscribe('app-global', (data) => {
      // 프로젝트 관련 메시지
      updateProjectFromWebSocket(data);

      // 빌드 관련 메시지
      updateBuildFromWebSocket(data);
    });

    return unsubscribe;
  });
</script>

<Router {routes} />
