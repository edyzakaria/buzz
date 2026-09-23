use crate::error::{IsmError, Result};
use crate::models::{Attachment, Comment, Issue, LoginResponse};
use chrono::{DateTime, Duration, Utc};
use moka::sync::Cache;
use tracing::debug;

const CACHE_SIZE: u64 = 1000;

/// Cached JWT token with expiry
#[derive(Clone, Debug)]
struct CachedToken {
    token: String,
    expires_at: DateTime<Utc>,
}

/// ISM HTTP client for fetching issues and related content.
///
/// Manages authentication via service-account credentials and caches the JWT token.
/// Endpoints are fetched live — no mirroring of ISM's schema.
pub struct IsmClient {
    base_url: String,
    service_email: String,
    service_password: String,
    http_client: reqwest::Client,
    token_cache: Cache<String, CachedToken>,
}

impl IsmClient {
    /// Create a new ISM client.
    ///
    /// # Arguments
    /// * `base_url` - ISM API base URL (e.g., `http://192.168.0.200:8080`)
    /// * `service_email` - Service account email for authentication
    /// * `service_password` - Service account password
    ///
    /// Returns an error if the base URL is invalid.
    pub fn new(base_url: String, service_email: String, service_password: String) -> Result<Self> {
        // Validate that base_url parses as a URL
        let _ = reqwest::Url::parse(&base_url)
            .map_err(|e| IsmError::Config(format!("invalid base_url: {e}")))?;

        Ok(Self {
            base_url,
            service_email,
            service_password,
            http_client: reqwest::Client::new(),
            token_cache: Cache::new(CACHE_SIZE),
        })
    }

    /// Get a valid JWT token, using cached token if available and not expired.
    async fn get_token(&self) -> Result<String> {
        let cache_key = "ism_token";

        // Check cache first
        if let Some(cached) = self.token_cache.get(cache_key) {
            if cached.expires_at > Utc::now() {
                debug!("Using cached ISM token");
                return Ok(cached.token);
            }
        }

        // Token missing or expired, fetch a new one
        debug!("Fetching new ISM token");
        let token = self.login().await?;

        // Cache for next time
        let expires_at = Utc::now() + Duration::seconds(3600);
        self.token_cache.insert(
            cache_key.to_string(),
            CachedToken {
                token: token.clone(),
                expires_at,
            },
        );

        Ok(token)
    }

    /// Authenticate with ISM and return a JWT token.
    async fn login(&self) -> Result<String> {
        let url = format!("{}/api/v1/auth/login", self.base_url);
        let body = serde_json::json!({
            "email": self.service_email,
            "password": self.service_password,
        });

        let response = self.http_client.post(&url).json(&body).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(IsmError::AuthFailed(format!("status {}: {}", status, text)));
        }

        let login_response: LoginResponse = response.json().await?;
        Ok(login_response.access_token)
    }

    /// Fetch an issue by ID.
    pub async fn get_issue(&self, id: &str) -> Result<Issue> {
        let token = self.get_token().await?;
        let url = format!("{}/api/v1/issues/{}", self.base_url, id);

        let response = self
            .http_client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await?;

        if response.status() == 404 {
            return Err(IsmError::NotFound { id: id.to_string() });
        }

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(IsmError::ApiError(format!("status {}: {}", status, text)));
        }

        let issue: Issue = response.json().await?;
        Ok(issue)
    }

    /// Fetch comments for an issue.
    pub async fn get_comments(&self, issue_id: &str) -> Result<Vec<Comment>> {
        let token = self.get_token().await?;
        let url = format!("{}/api/v1/issues/{}/comments", self.base_url, issue_id);

        let response = self
            .http_client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(IsmError::ApiError(format!("status {}: {}", status, text)));
        }

        let comments: Vec<Comment> = response.json().await?;
        Ok(comments)
    }

    /// Fetch attachments for an issue.
    pub async fn get_attachments(&self, issue_id: &str) -> Result<Vec<Attachment>> {
        let token = self.get_token().await?;
        let url = format!("{}/api/v1/issues/{}/attachments", self.base_url, issue_id);

        let response = self
            .http_client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(IsmError::ApiError(format!("status {}: {}", status, text)));
        }

        let attachments: Vec<Attachment> = response.json().await?;
        Ok(attachments)
    }

    /// Post a comment on an issue.
    pub async fn post_comment(&self, issue_id: &str, body: &str) -> Result<String> {
        let token = self.get_token().await?;
        let url = format!("{}/api/v1/issues/{}/comments", self.base_url, issue_id);
        let payload = serde_json::json!({ "body": body });

        let response = self
            .http_client
            .post(&url)
            .bearer_auth(&token)
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(IsmError::ApiError(format!("status {}: {}", status, text)));
        }

        let comment: Comment = response.json().await?;
        Ok(comment.id)
    }

    /// Create a new issue.
    pub async fn create_issue(&self, title: &str, description: Option<&str>) -> Result<String> {
        let token = self.get_token().await?;
        let url = format!("{}/api/v1/issues", self.base_url);

        let mut payload = serde_json::json!({ "title": title });
        if let Some(desc) = description {
            payload["description"] = serde_json::json!(desc);
        }

        let response = self
            .http_client
            .post(&url)
            .bearer_auth(&token)
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(IsmError::ApiError(format!("status {}: {}", status, text)));
        }

        let issue: Issue = response.json().await?;
        Ok(issue.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation_with_valid_url() {
        let result = IsmClient::new(
            "http://192.168.0.200:8080".to_string(),
            "user@example.com".to_string(),
            "password".to_string(),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_client_creation_with_invalid_url() {
        let result = IsmClient::new(
            "not a url".to_string(),
            "user@example.com".to_string(),
            "password".to_string(),
        );
        assert!(result.is_err());
    }
}
