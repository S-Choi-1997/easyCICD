use oauth2::{
    basic::BasicClient, AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl,
};
use std::env;

/// OAuth2 configuration for Google authentication
#[derive(Clone)]
pub struct OAuthConfig {
    pub client: BasicClient,
    pub session_secret: String,
}

impl OAuthConfig {
    /// Load OAuth config from environment variables
    pub fn from_env() -> Result<Self, String> {
        let client_id = env::var("GOOGLE_CLIENT_ID")
            .map_err(|_| "GOOGLE_CLIENT_ID not set")?;
        let client_secret = env::var("GOOGLE_CLIENT_SECRET")
            .map_err(|_| "GOOGLE_CLIENT_SECRET not set")?;
        let redirect_uri = env::var("GOOGLE_REDIRECT_URI")
            .map_err(|_| "GOOGLE_REDIRECT_URI not set")?;
        let session_secret = env::var("SESSION_SECRET")
            .map_err(|_| "SESSION_SECRET not set")?;

        let client = BasicClient::new(
            ClientId::new(client_id),
            Some(ClientSecret::new(client_secret)),
            AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
                .map_err(|e| format!("Invalid auth URL: {}", e))?,
            Some(TokenUrl::new("https://oauth2.googleapis.com/token".to_string())
                .map_err(|e| format!("Invalid token URL: {}", e))?),
        )
        .set_redirect_uri(
            RedirectUrl::new(redirect_uri)
                .map_err(|e| format!("Invalid redirect URI: {}", e))?,
        );

        Ok(Self {
            client,
            session_secret,
        })
    }
}
