<script>
  import { onMount } from 'svelte';
  import Router, { push, location } from 'svelte-spa-router';
  import Dashboard from './routes/Dashboard.svelte';
  import Setup from './routes/Setup.svelte';
  import BuildHistory from './routes/BuildHistory.svelte';
  import ProjectDetail from './routes/ProjectDetail.svelte';
  import Settings from './routes/Settings.svelte';
  import ContainerNew from './routes/ContainerNew.svelte';
  import Login from './routes/Login.svelte';
  import { initWebSocket, subscribe } from './stores/websocket';
  import { updateProjectFromWebSocket } from './stores/projects';
  import { updateBuildFromWebSocket } from './stores/builds';
  import { initAuth, isAuthenticated, authLoading } from './stores/auth';
  import './app.css';

  const routes = {
    '/login': Login,
    '/': Dashboard,
    '/setup': Setup,
    '/build/:id': BuildHistory,
    '/project/:id': ProjectDetail,
    '/settings': Settings,
    '/containers/new': ContainerNew,
  };

  // Auth guard - redirect to login if not authenticated
  $: {
    if (!$authLoading && !$isAuthenticated && $location !== '/login') {
      push('/login');
    }
  }

  onMount(async () => {
    // Version output (debugging)
    console.log('EasyCI/CD Frontend v2.1.0 - OAuth2 Auth');
    console.log('Build timestamp:', new Date().toISOString());

    // Initialize auth first
    await initAuth();

    // Only initialize WebSocket if authenticated
    if ($isAuthenticated) {
      initWebSocket();

      const unsubscribe = subscribe('app-global', (data) => {
        updateProjectFromWebSocket(data);
        updateBuildFromWebSocket(data);
      });

      return unsubscribe;
    }
  });

  // Re-initialize WebSocket when auth status changes
  $: if ($isAuthenticated && typeof window !== 'undefined') {
    // Small delay to ensure auth is fully settled
    setTimeout(() => {
      initWebSocket();
      subscribe('app-global', (data) => {
        updateProjectFromWebSocket(data);
        updateBuildFromWebSocket(data);
      });
    }, 100);
  }
</script>

{#if $authLoading}
  <div class="loading-screen">
    <div class="spinner"></div>
    <p>인증 확인 중...</p>
  </div>
{:else}
  <Router {routes} />
{/if}

<style>
  .loading-screen {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    background: var(--gray-50, #f9fafb);
  }

  .spinner {
    width: 40px;
    height: 40px;
    border: 3px solid var(--gray-200, #e5e7eb);
    border-top-color: var(--primary, #667eea);
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .loading-screen p {
    margin-top: 1rem;
    color: var(--gray-600, #4b5563);
  }
</style>
