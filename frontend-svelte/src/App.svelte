<script>
  import { onMount } from 'svelte';
  import Router from 'svelte-spa-router';
  import Dashboard from './routes/Dashboard.svelte';
  import Setup from './routes/Setup.svelte';
  import BuildHistory from './routes/BuildHistory.svelte';
  import Settings from './routes/Settings.svelte';
  import { initWebSocket, subscribe } from './stores/websocket';
  import { updateProjectFromWebSocket } from './stores/projects';
  import { updateBuildFromWebSocket } from './stores/builds';

  const routes = {
    '/': Dashboard,
    '/setup': Setup,
    '/build/:id': BuildHistory,
    '/settings': Settings,
  };

  onMount(() => {
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

<style global>
  @import './app.css';
</style>
