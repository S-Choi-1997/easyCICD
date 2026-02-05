use anyhow::{Context, Result};
use serde::Serialize;
use tracing::{debug, warn};

/// Discord Embed 색상
pub struct EmbedColor;
impl EmbedColor {
    pub const SUCCESS: u32 = 0x00ff00;  // 초록색
    pub const FAILURE: u32 = 0xff0000;  // 빨간색
    pub const INFO: u32 = 0x3498db;     // 파란색
    pub const WARNING: u32 = 0xffaa00;  // 주황색
}

/// Discord Webhook 메시지
#[derive(Debug, Serialize)]
pub struct DiscordMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub embeds: Option<Vec<DiscordEmbed>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DiscordEmbed {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<EmbedField>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub footer: Option<EmbedFooter>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<EmbedAuthor>,
}

#[derive(Debug, Serialize)]
pub struct EmbedField {
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct EmbedFooter {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EmbedAuthor {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
}

/// Discord 클라이언트
#[derive(Clone)]
pub struct DiscordClient {
    client: reqwest::Client,
}

impl DiscordClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Discord webhook으로 메시지 전송
    pub async fn send_message(
        &self,
        webhook_url: &str,
        message: DiscordMessage,
    ) -> Result<()> {
        debug!("Sending Discord message to webhook");

        let response = self
            .client
            .post(webhook_url)
            .json(&message)
            .send()
            .await
            .context("Failed to send Discord webhook")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!("Discord webhook failed: {} - {}", status, body);
            anyhow::bail!("Discord webhook returned {}: {}", status, body);
        }

        debug!("Discord message sent successfully");
        Ok(())
    }

    /// 빌드 시작 알림
    pub fn build_started_message(
        &self,
        project_name: &str,
        build_number: i64,
        branch: &str,
        commit_hash: &str,
        author: Option<&str>,
    ) -> DiscordMessage {
        let embed = DiscordEmbed {
            title: Some(format!("🔨 빌드 #{} 시작", build_number)),
            description: Some(format!("프로젝트 **{}**의 빌드가 시작되었습니다.", project_name)),
            color: Some(EmbedColor::INFO),
            fields: Some(vec![
                EmbedField {
                    name: "브랜치".to_string(),
                    value: format!("`{}`", branch),
                    inline: Some(true),
                },
                EmbedField {
                    name: "커밋".to_string(),
                    value: format!("`{}`", &commit_hash[..7.min(commit_hash.len())]),
                    inline: Some(true),
                },
                EmbedField {
                    name: "작성자".to_string(),
                    value: author.unwrap_or("Unknown").to_string(),
                    inline: Some(true),
                },
            ]),
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
            footer: Some(EmbedFooter {
                text: "Easy CI/CD".to_string(),
                icon_url: None,
            }),
            author: None,
        };

        DiscordMessage {
            content: None,
            embeds: Some(vec![embed]),
            username: Some("Easy CI/CD".to_string()),
            avatar_url: None,
        }
    }

    /// 빌드 성공 알림
    pub fn build_success_message(
        &self,
        project_name: &str,
        build_number: i64,
        branch: &str,
        duration_seconds: u64,
        build_url: Option<&str>,
    ) -> DiscordMessage {
        let embed = DiscordEmbed {
            title: Some(format!("✅ 빌드 #{} 성공", build_number)),
            description: Some(format!("프로젝트 **{}**의 빌드가 성공했습니다!", project_name)),
            color: Some(EmbedColor::SUCCESS),
            fields: Some(vec![
                EmbedField {
                    name: "브랜치".to_string(),
                    value: format!("`{}`", branch),
                    inline: Some(true),
                },
                EmbedField {
                    name: "빌드 시간".to_string(),
                    value: format!("{}초", duration_seconds),
                    inline: Some(true),
                },
                EmbedField {
                    name: "링크".to_string(),
                    value: if let Some(url) = build_url {
                        format!("[빌드 로그 보기]({})", url)
                    } else {
                        "N/A".to_string()
                    },
                    inline: Some(false),
                },
            ]),
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
            footer: Some(EmbedFooter {
                text: "Easy CI/CD".to_string(),
                icon_url: None,
            }),
            author: None,
        };

        DiscordMessage {
            content: None,
            embeds: Some(vec![embed]),
            username: Some("Easy CI/CD".to_string()),
            avatar_url: None,
        }
    }

    /// 빌드 실패 알림 (멘션 포함)
    pub fn build_failure_message(
        &self,
        project_name: &str,
        build_number: i64,
        branch: &str,
        error_message: Option<&str>,
        build_url: Option<&str>,
        mentions: Vec<String>,
    ) -> DiscordMessage {
        let embed = DiscordEmbed {
            title: Some(format!("❌ 빌드 #{} 실패", build_number)),
            description: Some(format!("프로젝트 **{}**의 빌드가 실패했습니다.", project_name)),
            color: Some(EmbedColor::FAILURE),
            fields: Some(vec![
                EmbedField {
                    name: "브랜치".to_string(),
                    value: format!("`{}`", branch),
                    inline: Some(true),
                },
                EmbedField {
                    name: "에러".to_string(),
                    value: error_message
                        .map(|e| format!("`{}`", e))
                        .unwrap_or_else(|| "로그를 확인해주세요".to_string()),
                    inline: Some(false),
                },
                EmbedField {
                    name: "링크".to_string(),
                    value: if let Some(url) = build_url {
                        format!("[빌드 로그 보기]({})", url)
                    } else {
                        "N/A".to_string()
                    },
                    inline: Some(false),
                },
            ]),
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
            footer: Some(EmbedFooter {
                text: "Easy CI/CD".to_string(),
                icon_url: None,
            }),
            author: None,
        };

        DiscordMessage {
            content: if mentions.is_empty() {
                None
            } else {
                Some(mentions.join(" "))
            },
            embeds: Some(vec![embed]),
            username: Some("Easy CI/CD".to_string()),
            avatar_url: None,
        }
    }

    /// 배포 성공 알림
    pub fn deployment_success_message(
        &self,
        project_name: &str,
        build_number: i64,
        slot: &str,
        app_url: Option<&str>,
    ) -> DiscordMessage {
        let embed = DiscordEmbed {
            title: Some(format!("🚀 배포 완료 (빌드 #{})", build_number)),
            description: Some(format!("프로젝트 **{}**가 성공적으로 배포되었습니다!", project_name)),
            color: Some(EmbedColor::SUCCESS),
            fields: Some(vec![
                EmbedField {
                    name: "슬롯".to_string(),
                    value: format!("`{}`", slot),
                    inline: Some(true),
                },
                EmbedField {
                    name: "애플리케이션".to_string(),
                    value: if let Some(url) = app_url {
                        format!("[앱 열기]({})", url)
                    } else {
                        "N/A".to_string()
                    },
                    inline: Some(false),
                },
            ]),
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
            footer: Some(EmbedFooter {
                text: "Easy CI/CD".to_string(),
                icon_url: None,
            }),
            author: None,
        };

        DiscordMessage {
            content: None,
            embeds: Some(vec![embed]),
            username: Some("Easy CI/CD".to_string()),
            avatar_url: None,
        }
    }

    /// 배포 실패 알림
    pub fn deployment_failure_message(
        &self,
        project_name: &str,
        build_number: i64,
        error_message: Option<&str>,
        mentions: Vec<String>,
    ) -> DiscordMessage {
        let embed = DiscordEmbed {
            title: Some(format!("🔥 배포 실패 (빌드 #{})", build_number)),
            description: Some(format!("프로젝트 **{}**의 배포가 실패했습니다.", project_name)),
            color: Some(EmbedColor::FAILURE),
            fields: Some(vec![EmbedField {
                name: "에러".to_string(),
                value: error_message
                    .map(|e| format!("`{}`", e))
                    .unwrap_or_else(|| "로그를 확인해주세요".to_string()),
                inline: Some(false),
            }]),
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
            footer: Some(EmbedFooter {
                text: "Easy CI/CD".to_string(),
                icon_url: None,
            }),
            author: None,
        };

        DiscordMessage {
            content: if mentions.is_empty() {
                None
            } else {
                Some(mentions.join(" "))
            },
            embeds: Some(vec![embed]),
            username: Some("Easy CI/CD".to_string()),
            avatar_url: None,
        }
    }
}
