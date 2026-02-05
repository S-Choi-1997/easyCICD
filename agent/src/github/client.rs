use anyhow::{Result, anyhow};
use reqwest::Client;
use super::models::*;

#[derive(Debug, Clone)]
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

    /// List user repositories (with pagination to fetch all repos)
    pub async fn list_repositories(&self) -> Result<Vec<Repository>> {
        let mut all_repos = Vec::new();
        let mut page = 1;
        let per_page = 100;

        loop {
            let url = "https://api.github.com/user/repos";
            let response = self.client
                .get(url)
                .query(&[
                    ("affiliation", "owner,collaborator,organization_member"),
                    ("visibility", "all"),  // public, private, internal 모두 포함
                    ("sort", "updated"),
                    ("per_page", &per_page.to_string()),
                    ("page", &page.to_string()),
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

            let repos: Vec<Repository> = response.json().await?;
            let repos_count = repos.len();

            tracing::info!("Fetched page {} with {} repositories (total so far: {})", page, repos_count, all_repos.len() + repos_count);

            all_repos.extend(repos);

            // If we got less than per_page, we've reached the last page
            if repos_count < per_page {
                tracing::info!("Reached last page (got {} < {} per_page). Total repositories: {}", repos_count, per_page, all_repos.len());
                break;
            }

            page += 1;

            // Safety limit: stop after 10 pages (1000 repos)
            if page > 10 {
                tracing::warn!("Reached safety limit of 10 pages. Total repositories: {}", all_repos.len());
                break;
            }
        }

        Ok(all_repos)
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
        let url = format!("https://api.github.com/repos/{}/{}/git/trees/{}?recursive=1", owner, repo, sha);
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

    /// Get file content from repository
    pub async fn get_file_content(&self, owner: &str, repo: &str, path: &str, branch: &str) -> Result<String> {
        let url = format!("https://api.github.com/repos/{}/{}/contents/{}", owner, repo, path);
        let response = self.client
            .get(&url)
            .query(&[("ref", branch)])
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

        #[derive(serde::Deserialize)]
        struct FileContent {
            content: String,
            encoding: String,
        }

        let file_content: FileContent = response.json().await?;

        if file_content.encoding == "base64" {
            let decoded = base64::decode(&file_content.content.replace("\n", ""))?;
            Ok(String::from_utf8(decoded)?)
        } else {
            Ok(file_content.content)
        }
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

    /// List webhooks for a repository
    pub async fn list_webhooks(&self, owner: &str, repo: &str) -> Result<Vec<Webhook>> {
        let url = format!("https://api.github.com/repos/{}/{}/hooks", owner, repo);

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
}
