use anyhow::{Result, anyhow};
use reqwest::Client;
use super::models::*;

pub struct GitHubClient {
    client: Client,
    token: String,
}

impl GitHubClient {
    pub fn new(token: String) -> Self {
        Self {
            client: Client::new(),
            token,
        }
    }

    /// Get authenticated user info
    pub async fn get_user(&self) -> Result<User> {
        let url = "https://api.github.com/user";
        let response = self.client
            .get(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "EasyCI CD")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            return Err(anyhow!("GitHub API error ({}): {}", status, body));
        }

        Ok(response.json().await?)
    }

    /// List user repositories
    pub async fn list_repositories(&self) -> Result<Vec<Repository>> {
        let url = "https://api.github.com/user/repos";
        let response = self.client
            .get(url)
            .query(&[
                ("affiliation", "owner,collaborator"),
                ("sort", "updated"),
                ("per_page", "100"),
            ])
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "EasyCI CD")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            return Err(anyhow!("GitHub API error ({}): {}", status, body));
        }

        Ok(response.json().await?)
    }

    /// List repository branches
    pub async fn list_branches(&self, owner: &str, repo: &str) -> Result<Vec<Branch>> {
        let url = format!("https://api.github.com/repos/{}/{}/branches", owner, repo);
        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "EasyCI CD")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            return Err(anyhow!("GitHub API error ({}): {}", status, body));
        }

        Ok(response.json().await?)
    }

    /// Get repository tree (for folder structure)
    pub async fn get_tree(&self, owner: &str, repo: &str, sha: &str) -> Result<Tree> {
        let url = format!("https://api.github.com/repos/{}/{}/git/trees/{}", owner, repo, sha);
        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "EasyCI CD")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            return Err(anyhow!("GitHub API error ({}): {}", status, body));
        }

        Ok(response.json().await?)
    }

    /// Create webhook
    pub async fn create_webhook(
        &self,
        owner: &str,
        repo: &str,
        webhook_url: &str,
        secret: &str,
    ) -> Result<Webhook> {
        let url = format!("https://api.github.com/repos/{}/{}/hooks", owner, repo);

        let request = CreateWebhookRequest {
            name: "web".to_string(),
            active: true,
            events: vec!["push".to_string()],
            config: WebhookConfig {
                url: webhook_url.to_string(),
                content_type: "json".to_string(),
                secret: secret.to_string(),
                insecure_ssl: "0".to_string(),
            },
        };

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "EasyCI CD")
            .header("Accept", "application/vnd.github.v3+json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            return Err(anyhow!("GitHub API error ({}): {}", status, body));
        }

        Ok(response.json().await?)
    }

    /// Delete webhook
    pub async fn delete_webhook(&self, owner: &str, repo: &str, hook_id: u64) -> Result<()> {
        let url = format!("https://api.github.com/repos/{}/{}/hooks/{}", owner, repo, hook_id);

        let response = self.client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "EasyCI CD")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            return Err(anyhow!("GitHub API error ({}): {}", status, body));
        }

        Ok(())
    }
}
