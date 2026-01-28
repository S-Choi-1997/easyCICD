<script>
  import { onMount } from 'svelte';
  import { push, querystring } from 'svelte-spa-router';
  import { loginWithGoogle, authLoading, authError, isAuthenticated } from '../stores/auth';

  // Parse error from URL query string
  let errorMessage = '';

  $: {
    if ($querystring) {
      const params = new URLSearchParams($querystring);
      const error = params.get('error');
      if (error) {
        switch (error) {
          case 'missing_csrf':
          case 'invalid_csrf':
            errorMessage = '인증 세션이 만료되었습니다. 다시 시도해주세요.';
            break;
          case 'missing_pkce':
            errorMessage = '인증 검증에 실패했습니다. 다시 시도해주세요.';
            break;
          case 'token_exchange_failed':
            errorMessage = 'Google 인증에 실패했습니다. 다시 시도해주세요.';
            break;
          case 'user_info_failed':
            errorMessage = '사용자 정보를 가져올 수 없습니다.';
            break;
          case 'not_allowed':
            errorMessage = '접근 권한이 없습니다. 관리자에게 문의하세요.';
            break;
          case 'database_error':
          case 'session_error':
            errorMessage = '서버 오류가 발생했습니다. 잠시 후 다시 시도해주세요.';
            break;
          default:
            errorMessage = '로그인 중 오류가 발생했습니다.';
        }
      }
    }
  }

  onMount(() => {
    // If already authenticated, redirect to home
    const unsubscribe = isAuthenticated.subscribe(value => {
      if (value) {
        push('/');
      }
    });
    return unsubscribe;
  });

  function handleGoogleLogin() {
    errorMessage = '';
    loginWithGoogle();
  }
</script>

<div class="login-container">
  <div class="login-card">
    <div class="login-header">
      <div class="logo">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M12 2L2 7l10 5 10-5-10-5z"/>
          <path d="M2 17l10 5 10-5"/>
          <path d="M2 12l10 5 10-5"/>
        </svg>
      </div>
      <h1>Easy CI/CD</h1>
      <p>지속적 통합 및 배포 시스템</p>
    </div>

    {#if errorMessage || $authError}
      <div class="error-message">
        {errorMessage || $authError}
      </div>
    {/if}

    <div class="login-body">
      <button
        class="btn-google"
        on:click={handleGoogleLogin}
        disabled={$authLoading}
      >
        <svg class="google-icon" viewBox="0 0 24 24" width="24" height="24">
          <path fill="#4285F4" d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"/>
          <path fill="#34A853" d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"/>
          <path fill="#FBBC05" d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"/>
          <path fill="#EA4335" d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"/>
        </svg>
        {#if $authLoading}
          로그인 중...
        {:else}
          Google로 로그인
        {/if}
      </button>
    </div>

    <div class="login-footer">
      <p>Google 계정으로 로그인하여 시작하세요</p>
    </div>
  </div>
</div>

<style>
  .login-container {
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    padding: 1rem;
  }

  .login-card {
    background: white;
    border-radius: 1rem;
    box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.25);
    padding: 3rem;
    width: 100%;
    max-width: 400px;
    text-align: center;
  }

  .login-header {
    margin-bottom: 2rem;
  }

  .logo {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 80px;
    height: 80px;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    border-radius: 1rem;
    color: white;
    margin-bottom: 1rem;
  }

  .login-header h1 {
    margin: 0 0 0.5rem 0;
    font-size: 2rem;
    font-weight: 700;
    color: #1a202c;
  }

  .login-header p {
    margin: 0;
    color: #718096;
    font-size: 0.95rem;
  }

  .error-message {
    background: #fed7d7;
    color: #c53030;
    padding: 0.75rem 1rem;
    border-radius: 0.5rem;
    margin-bottom: 1.5rem;
    font-size: 0.875rem;
  }

  .login-body {
    margin: 2rem 0;
  }

  .btn-google {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.75rem;
    width: 100%;
    padding: 0.875rem 1.5rem;
    background: white;
    border: 2px solid #e2e8f0;
    border-radius: 0.5rem;
    font-size: 1rem;
    font-weight: 500;
    color: #4a5568;
    cursor: pointer;
    transition: all 0.2s;
  }

  .btn-google:hover:not(:disabled) {
    border-color: #cbd5e0;
    background: #f7fafc;
    box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);
  }

  .btn-google:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .google-icon {
    flex-shrink: 0;
  }

  .login-footer {
    margin-top: 1rem;
    padding-top: 1rem;
    border-top: 1px solid #e2e8f0;
  }

  .login-footer p {
    margin: 0;
    color: #a0aec0;
    font-size: 0.813rem;
  }
</style>
