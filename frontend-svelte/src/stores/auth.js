import { writable, derived } from 'svelte/store';

// Auth state stores
export const user = writable(null);
export const authLoading = writable(true);
export const authError = writable(null);

// Derived store for easy auth check
export const isAuthenticated = derived(user, ($user) => $user !== null);

/**
 * Check current authentication status
 */
export async function checkAuth() {
    authLoading.set(true);
    authError.set(null);

    try {
        const response = await fetch('/auth/me', {
            credentials: 'include'  // Important for cookies
        });

        if (!response.ok) {
            throw new Error('Failed to check auth status');
        }

        const data = await response.json();

        if (data.authenticated && data.user) {
            user.set(data.user);
        } else {
            user.set(null);
        }
    } catch (error) {
        console.error('[Auth] Check auth error:', error);
        authError.set(error.message);
        user.set(null);
    } finally {
        authLoading.set(false);
    }
}

/**
 * Redirect to Google login
 */
export function loginWithGoogle() {
    window.location.href = '/auth/google';
}

/**
 * Logout
 */
export async function logout() {
    try {
        const response = await fetch('/auth/logout', {
            method: 'POST',
            credentials: 'include'
        });

        if (response.ok) {
            user.set(null);
            window.location.href = '/#/login';
        } else {
            throw new Error('Logout failed');
        }
    } catch (error) {
        console.error('[Auth] Logout error:', error);
        authError.set(error.message);
    }
}

/**
 * Initialize auth check on app load
 */
export function initAuth() {
    return checkAuth();
}
