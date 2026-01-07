<script>
  import Router from 'svelte-spa-router';
  import Dashboard from './routes/Dashboard.svelte';
  import Setup from './routes/Setup.svelte';
  import BuildHistory from './routes/BuildHistory.svelte';

  const routes = {
    '/': Dashboard,
    '/setup': Setup,
    '/build/:id': BuildHistory,
  };
</script>

<Router {routes} />

<style global>
  @import './app.css';
</style>
