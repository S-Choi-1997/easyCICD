use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use oauth2::{
    reqwest::async_http_client, AuthorizationCode, CsrfToken, PkceCodeChallenge,
    PkceCodeVerifier, Scope, TokenResponse,
};
use serde::{Deserialize, Serialize};
use tower_cookies::{Cookie, Cookies};
use tracing::{info, warn, error};

use crate::db::models::CreateUser;
use crate::infrastructure::logging::{TraceContext, Timer};
use crate::state::AppContext;
use crate::application::ports::repositories::{SessionRepository, UserRepository};
use super::settings::is_email_allowed;

// Session cookie name
const SESSION_COOKIE: &str = "easycicd_session";
// PKCE verifier cookie (temporary during OAuth flow)
const PKCE_COOKIE: &str = "easycicd_pkce";
// CSRF state cookie
const CSRF_COOKIE: &str = "easycicd_csrf";

/// Auth routes
pub fn auth_routes() -> Router<AppContext> {
    Router::new()
        .route("/google", get(google_login))
        .route("/google/callback", get(google_callback))
        .route("/logout", post(logout))
        .route("/me", get(get_current_user))
}

/// GET /auth/google - Start OAuth flow
async fn google_login(
    State(ctx): State<AppContext>,
    cookies: Cookies,
    headers: HeaderMap,
) -> Response {
    let trace_id = TraceContext::extract_or_generate(&headers);
    ctx.logger.api_entry(&trace_id, "GET", "/auth/google", "");

    let oauth_config = match &ctx.oauth_config {
        Some(cfg) => cfg,
        None => {
            error!("[{}] OAuth not configured", trace_id);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "OAuth not configured",
            ).into_response();
        }
    };

    // Generate PKCE challenge
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    // Generate CSRF state
    let (auth_url, csrf_token) = oauth_config
        .client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    // Store PKCE verifier in HTTP-only cookie (expires in 10 minutes)
    let mut pkce_cookie = Cookie::new(PKCE_COOKIE, pkce_verifier.secret().clone());
    pkce_cookie.set_path("/");
    pkce_cookie.set_http_only(true);
    pkce_cookie.set_secure(true);
    pkce_cookie.set_same_site(tower_cookies::cookie::SameSite::Lax);
    pkce_cookie.set_max_age(tower_cookies::cookie::time::Duration::minutes(10));
    cookies.add(pkce_cookie);

    // Store CSRF state in HTTP-only cookie
    let mut csrf_cookie = Cookie::new(CSRF_COOKIE, csrf_token.secret().clone());
    csrf_cookie.set_path("/");
    csrf_cookie.set_http_only(true);
    csrf_cookie.set_secure(true);
    csrf_cookie.set_same_site(tower_cookies::cookie::SameSite::Lax);
    csrf_cookie.set_max_age(tower_cookies::cookie::time::Duration::minutes(10));
    cookies.add(csrf_cookie);

    info!("[{}] Redirecting to Google OAuth", trace_id);
    ctx.logger.api_exit(&trace_id, "GET", "/auth/google", 0.0, 302);

    Redirect::temporary(auth_url.as_str()).into_response()
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: String,
    state: String,
}

#[derive(Deserialize)]
struct GoogleUserInfo {
    sub: String,          // Google ID
    email: String,
    name: String,
    picture: Option<String>,
}

/// GET /auth/google/callback - OAuth callback handler
async fn google_callback(
    State(ctx): State<AppContext>,
    cookies: Cookies,
    headers: HeaderMap,
    Query(query): Query<CallbackQuery>,
) -> Response {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();
    ctx.logger.api_entry(&trace_id, "GET", "/auth/google/callback", "");

    let oauth_config = match &ctx.oauth_config {
        Some(cfg) => cfg,
        None => {
            error!("[{}] OAuth not configured", trace_id);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "OAuth not configured",
            ).into_response();
        }
    };

    // Verify CSRF state
    let stored_csrf = match cookies.get(CSRF_COOKIE) {
        Some(c) => c.value().to_string(),
        None => {
            warn!("[{}] Missing CSRF state cookie", trace_id);
            ctx.logger.api_exit(&trace_id, "GET", "/auth/google/callback", timer.elapsed_ms(), 400);
            return Redirect::temporary("/login?error=missing_csrf").into_response();
        }
    };

    if stored_csrf != query.state {
        warn!("[{}] Invalid CSRF state", trace_id);
        ctx.logger.api_exit(&trace_id, "GET", "/auth/google/callback", timer.elapsed_ms(), 400);
        return Redirect::temporary("/login?error=invalid_csrf").into_response();
    }

    // Get PKCE verifier
    let pkce_verifier = match cookies.get(PKCE_COOKIE) {
        Some(c) => PkceCodeVerifier::new(c.value().to_string()),
        None => {
            warn!("[{}] Missing PKCE verifier cookie", trace_id);
            ctx.logger.api_exit(&trace_id, "GET", "/auth/google/callback", timer.elapsed_ms(), 400);
            return Redirect::temporary("/login?error=missing_pkce").into_response();
        }
    };

    // Exchange code for token
    let token_result = oauth_config
        .client
        .exchange_code(AuthorizationCode::new(query.code))
        .set_pkce_verifier(pkce_verifier)
        .request_async(async_http_client)
        .await;

    let token = match token_result {
        Ok(t) => t,
        Err(e) => {
            error!("[{}] Token exchange failed: {:?}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "GET", "/auth/google/callback", timer.elapsed_ms(), 500);
            return Redirect::temporary("/login?error=token_exchange_failed").into_response();
        }
    };

    // Fetch user info from Google
    let user_info = match fetch_google_user_info(token.access_token().secret()).await {
        Ok(info) => info,
        Err(e) => {
            error!("[{}] Failed to fetch user info: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "GET", "/auth/google/callback", timer.elapsed_ms(), 500);
            return Redirect::temporary("/login?error=user_info_failed").into_response();
        }
    };

    // Check email whitelist
    if !is_email_allowed(&ctx, &user_info.email).await {
        warn!("[{}] Email not in whitelist: {}", trace_id, user_info.email);
        ctx.logger.api_exit(&trace_id, "GET", "/auth/google/callback", timer.elapsed_ms(), 403);
        return Redirect::temporary("/login?error=not_allowed").into_response();
    }

    // Upsert user in database
    let create_user = CreateUser {
        google_id: user_info.sub,
        email: user_info.email.clone(),
        name: user_info.name,
        picture: user_info.picture,
    };

    let user = match ctx.user_repo.upsert(create_user).await {
        Ok(u) => u,
        Err(e) => {
            error!("[{}] Failed to upsert user: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "GET", "/auth/google/callback", timer.elapsed_ms(), 500);
            return Redirect::temporary("/login?error=database_error").into_response();
        }
    };

    // Create session
    let session_id = uuid::Uuid::new_v4().to_string();
    let expires_at = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::days(7))
        .unwrap()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    let create_session = crate::db::models::CreateSession {
        id: session_id.clone(),
        user_id: user.id,
        expires_at,
    };

    if let Err(e) = ctx.session_repo.create(create_session).await {
        error!("[{}] Failed to create session: {}", trace_id, e);
        ctx.logger.api_exit(&trace_id, "GET", "/auth/google/callback", timer.elapsed_ms(), 500);
        return Redirect::temporary("/login?error=session_error").into_response();
    }

    // Clear temporary cookies
    let mut remove_pkce = Cookie::new(PKCE_COOKIE, "");
    remove_pkce.set_path("/");
    remove_pkce.set_max_age(tower_cookies::cookie::time::Duration::seconds(0));
    cookies.add(remove_pkce);

    let mut remove_csrf = Cookie::new(CSRF_COOKIE, "");
    remove_csrf.set_path("/");
    remove_csrf.set_max_age(tower_cookies::cookie::time::Duration::seconds(0));
    cookies.add(remove_csrf);

    // Set session cookie (7 days, HTTP-only, Secure)
    let mut session_cookie = Cookie::new(SESSION_COOKIE, session_id);
    session_cookie.set_path("/");
    session_cookie.set_http_only(true);
    session_cookie.set_secure(true);
    session_cookie.set_same_site(tower_cookies::cookie::SameSite::Lax);
    session_cookie.set_max_age(tower_cookies::cookie::time::Duration::days(7));
    cookies.add(session_cookie);

    info!("[{}] User {} logged in successfully", trace_id, user_info.email);
    ctx.logger.api_exit(&trace_id, "GET", "/auth/google/callback", timer.elapsed_ms(), 302);

    // Redirect to home page
    Redirect::temporary("/").into_response()
}

async fn fetch_google_user_info(access_token: &str) -> Result<GoogleUserInfo, String> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://www.googleapis.com/oauth2/v3/userinfo")
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Google API error: {}", response.status()));
    }

    response
        .json::<GoogleUserInfo>()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

/// POST /auth/logout - Logout
async fn logout(
    State(ctx): State<AppContext>,
    cookies: Cookies,
    headers: HeaderMap,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();
    ctx.logger.api_entry(&trace_id, "POST", "/auth/logout", "");

    // Get session from cookie
    if let Some(session_cookie) = cookies.get(SESSION_COOKIE) {
        let session_id = session_cookie.value();

        // Delete session from database
        if let Err(e) = ctx.session_repo.delete(session_id).await {
            warn!("[{}] Failed to delete session: {}", trace_id, e);
        }
    }

    // Clear session cookie
    let mut removal_cookie = Cookie::new(SESSION_COOKIE, "");
    removal_cookie.set_path("/");
    removal_cookie.set_max_age(tower_cookies::cookie::time::Duration::seconds(0));
    cookies.add(removal_cookie);

    ctx.logger.api_exit(&trace_id, "POST", "/auth/logout", timer.elapsed_ms(), 200);

    (StatusCode::OK, Json(serde_json::json!({"success": true})))
}

#[derive(Serialize)]
struct CurrentUserResponse {
    authenticated: bool,
    user: Option<UserInfo>,
}

#[derive(Serialize)]
struct UserInfo {
    id: i64,
    email: String,
    name: String,
    picture: Option<String>,
}

/// GET /auth/me - Get current user
async fn get_current_user(
    State(ctx): State<AppContext>,
    cookies: Cookies,
    headers: HeaderMap,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();
    ctx.logger.api_entry(&trace_id, "GET", "/auth/me", "");

    let session_cookie = match cookies.get(SESSION_COOKIE) {
        Some(c) => c,
        None => {
            ctx.logger.api_exit(&trace_id, "GET", "/auth/me", timer.elapsed_ms(), 200);
            return (
                StatusCode::OK,
                Json(CurrentUserResponse {
                    authenticated: false,
                    user: None,
                }),
            );
        }
    };

    let session_id = session_cookie.value();

    match ctx.session_repo.get_with_user(session_id).await {
        Ok(Some((_session, user))) => {
            ctx.logger.api_exit(&trace_id, "GET", "/auth/me", timer.elapsed_ms(), 200);
            (
                StatusCode::OK,
                Json(CurrentUserResponse {
                    authenticated: true,
                    user: Some(UserInfo {
                        id: user.id,
                        email: user.email,
                        name: user.name,
                        picture: user.picture,
                    }),
                }),
            )
        }
        _ => {
            ctx.logger.api_exit(&trace_id, "GET", "/auth/me", timer.elapsed_ms(), 200);
            (
                StatusCode::OK,
                Json(CurrentUserResponse {
                    authenticated: false,
                    user: None,
                }),
            )
        }
    }
}
