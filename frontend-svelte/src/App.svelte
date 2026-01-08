<script>
  import Router from 'svelte-spa-router';
  import Dashboard from './routes/Dashboard.svelte';
  import Setup from './routes/Setup.svelte';
  import BuildHistory from './routes/BuildHistory.svelte';
  import Settings from './routes/Settings.svelte';

  const routes = {
    '/': Dashboard,
    '/setup': Setup,
    '/build/:id': BuildHistory,
    '/settings': Settings,
  };
</script>

<Router {routes} />

<style global>
  @import './app.css';
</style>
