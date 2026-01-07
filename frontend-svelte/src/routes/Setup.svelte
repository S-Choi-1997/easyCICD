<script>
  import { push } from 'svelte-spa-router';
  import { link } from 'svelte-spa-router';

  const API_BASE = '/api';

  let formData = {
    name: '',
    repo: '',
    branch: 'main',
    path_filter: '**',
    build_image: '',
    cache_type: 'gradle',
    build_command: '',
    runtime_image: '',
    runtime_command: '',
    health_check_url: '/',
  };

  let submitting = false;

  async function handleSubmit() {
    submitting = true;

    try {
      const response = await fetch(`${API_BASE}/projects`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(formData),
      });

      if (!response.ok) {
        const error = await response.json();
        throw new Error(error.message || 'Failed to create project');
      }

      const project = await response.json();
      alert(`Project "${project.name}" created successfully!`);
      push('/');
    } catch (error) {
      alert('Failed to create project: ' + error.message);
    } finally {
      submitting = false;
    }
  }
</script>

<header>
  <div class="header-content">
    <h1>Lightweight CI/CD</h1>
    <div class="header-actions">
      <a href="/" use:link class="btn btn-secondary">← Back to Dashboard</a>
    </div>
  </div>
</header>

<div class="container">
  <div class="card">
    <div class="card-header">
      <h2 class="card-title">Create New Project</h2>
    </div>

    <form on:submit|preventDefault={handleSubmit}>
      <!-- Project Info -->
      <div class="form-group">
        <label class="form-label" for="name">Project Name</label>
        <input
          type="text"
          id="name"
          class="form-input"
          bind:value={formData.name}
          required
          pattern="[a-z0-9-]+"
          placeholder="my-backend"
        />
        <div class="form-help">Lowercase letters, numbers, and hyphens only</div>
      </div>

      <div class="form-group">
        <label class="form-label" for="repo">GitHub Repository</label>
        <input
          type="text"
          id="repo"
          class="form-input"
          bind:value={formData.repo}
          required
          placeholder="username/repository"
        />
        <div class="form-help">Format: username/repository</div>
      </div>

      <div class="form-group">
        <label class="form-label" for="branch">Branch</label>
        <input
          type="text"
          id="branch"
          class="form-input"
          bind:value={formData.branch}
          required
        />
      </div>

      <div class="form-group">
        <label class="form-label" for="path_filter">Path Filter</label>
        <input
          type="text"
          id="path_filter"
          class="form-input"
          bind:value={formData.path_filter}
        />
        <div class="form-help">Use ** for all files, or specify patterns like src/**,tests/**</div>
      </div>

      <!-- Build Settings -->
      <h3 class="mt-2 mb-2">Build Settings</h3>

      <div class="form-group">
        <label class="form-label" for="build_image">Build Image</label>
        <input
          type="text"
          id="build_image"
          class="form-input"
          bind:value={formData.build_image}
          required
          placeholder="gradle:jdk17"
        />
        <div class="form-help">Docker image for building (e.g., gradle:jdk17, node:20)</div>
      </div>

      <div class="form-group">
        <label class="form-label" for="cache_type">Cache Type</label>
        <select id="cache_type" class="form-select" bind:value={formData.cache_type} required>
          <option value="gradle">Gradle</option>
          <option value="maven">Maven</option>
          <option value="npm">NPM</option>
          <option value="pip">Pip</option>
          <option value="cargo">Cargo</option>
          <option value="none">None</option>
        </select>
      </div>

      <div class="form-group">
        <label class="form-label" for="build_command">Build Command</label>
        <textarea
          id="build_command"
          class="form-textarea"
          bind:value={formData.build_command}
          required
          placeholder="./gradlew clean bootJar && cp build/libs/*.jar /output/app.jar"
        ></textarea>
        <div class="form-help">Commands to build your project. Output files should be copied to /output/</div>
      </div>

      <!-- Runtime Settings -->
      <h3 class="mt-2 mb-2">Runtime Settings</h3>

      <div class="form-group">
        <label class="form-label" for="runtime_image">Runtime Image</label>
        <input
          type="text"
          id="runtime_image"
          class="form-input"
          bind:value={formData.runtime_image}
          required
          placeholder="eclipse-temurin:17-jre"
        />
        <div class="form-help">Docker image for running your app</div>
      </div>

      <div class="form-group">
        <label class="form-label" for="runtime_command">Runtime Command</label>
        <input
          type="text"
          id="runtime_command"
          class="form-input"
          bind:value={formData.runtime_command}
          required
          placeholder="java -jar /app/app.jar"
        />
        <div class="form-help">Command to start your application</div>
      </div>

      <div class="form-group">
        <label class="form-label" for="health_check_url">Health Check URL</label>
        <input
          type="text"
          id="health_check_url"
          class="form-input"
          bind:value={formData.health_check_url}
          required
          placeholder="/actuator/health"
        />
        <div class="form-help">URL path for health check (e.g., /health, /actuator/health)</div>
      </div>

      <div class="form-group">
        <button type="submit" class="btn btn-primary" disabled={submitting}>
          {submitting ? 'Creating...' : 'Create Project'}
        </button>
        <a href="/" use:link class="btn btn-secondary">Cancel</a>
      </div>
    </form>
  </div>
</div>
