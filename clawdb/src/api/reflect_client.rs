//! HTTP client connecting the ClawDB runtime to the claw-reflect Python microservice.

use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;
use serde::{Deserialize, Serialize};

use crate::{
    config::ClawDBConfig,
    error::{ClawDBError, ClawDBResult},
};

/// Result of triggering a reflect job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectJobResult {
    pub job_id: String,
    pub status: String,
}

/// Status of a running reflect job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectJobStatus {
    pub job_id: String,
    pub status: String,
    pub progress: f32,
    pub processed: u32,
}

/// HTTP client for the claw-reflect Python microservice.
pub struct ReflectClient {
    base_url: String,
    client: reqwest::Client,
    #[allow(dead_code)]
    config: Arc<ClawDBConfig>,
}

impl ReflectClient {
    /// Creates a new `ReflectClient` connected to `base_url`.
    pub fn new(base_url: String, config: Arc<ClawDBConfig>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build reqwest client");
        Self { base_url, client, config }
    }

    /// Returns `true` if the reflect service responds with HTTP 200 to `GET /health`.
    pub async fn health_check(&self) -> ClawDBResult<bool> {
        let url = format!("{}/health", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ClawDBError::Reflect(e.to_string()))?;
        Ok(resp.status().is_success())
    }

    /// Returns `true` if the reflect service is reachable and ready.
    pub async fn is_ready(&self) -> bool {
        let url = format!("{}/ready", self.base_url);
        self.client
            .get(&url)
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Triggers a reflection job; retries up to 3 times on transient errors.
    pub async fn trigger_reflection(
        &self,
        agent_id: Uuid,
        job_type: &str,
        dry_run: bool,
    ) -> ClawDBResult<ReflectJobResult> {
        let url = format!("{}/api/v1/reflect/trigger", self.base_url);
        let body = serde_json::json!({
            "agent_id": agent_id,
            "job_type": job_type,
            "options": { "dry_run": dry_run },
        });

        let op = || async {
            self.client
                .post(&url)
                .json(&body)
                .send()
                .await
                .map_err(|e| backoff::Error::transient(e.to_string()))
        };

        let resp = backoff::future::retry(backoff::ExponentialBackoff::default(), op)
            .await
            .map_err(ClawDBError::Reflect)?;

        if !resp.status().is_success() {
            return Err(ClawDBError::Reflect(format!(
                "reflect trigger returned {}",
                resp.status()
            )));
        }

        resp.json::<ReflectJobResult>()
            .await
            .map_err(|e| ClawDBError::Reflect(e.to_string()))
    }

    /// Pushes a batch of memory records to the reflect service; returns the accepted count.
    pub async fn push_memories(
        &self,
        agent_id: Uuid,
        memories: Vec<serde_json::Value>,
    ) -> ClawDBResult<u32> {
        let url = format!("{}/api/v1/reflect/memories", self.base_url);
        let body = serde_json::json!({
            "agent_id": agent_id,
            "memories": memories,
        });

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| ClawDBError::Reflect(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(ClawDBError::Reflect(format!(
                "push_memories returned {}",
                resp.status()
            )));
        }

        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ClawDBError::Reflect(e.to_string()))?;
        Ok(v["accepted"].as_u64().unwrap_or(0) as u32)
    }

    /// Fetches the status of a running reflect job.
    pub async fn get_job_status(&self, job_id: &str) -> ClawDBResult<ReflectJobStatus> {
        let url = format!("{}/api/v1/jobs/{job_id}", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ClawDBError::Reflect(e.to_string()))?;
        resp.json::<ReflectJobStatus>()
            .await
            .map_err(|e| ClawDBError::Reflect(e.to_string()))
    }

    /// Retrieves the agent profile from the reflect service.
    pub async fn get_agent_profile(&self, agent_id: Uuid) -> ClawDBResult<serde_json::Value> {
        let url = format!("{}/api/v1/profiles/{agent_id}", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ClawDBError::Reflect(e.to_string()))?;
        resp.json().await.map_err(|e| ClawDBError::Reflect(e.to_string()))
    }

    /// Retrieves the agent's learned preferences from the reflect service.
    pub async fn get_preferences(
        &self,
        agent_id: Uuid,
    ) -> ClawDBResult<Vec<serde_json::Value>> {
        let url = format!("{}/api/v1/profiles/{agent_id}/preferences", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ClawDBError::Reflect(e.to_string()))?;
        resp.json().await.map_err(|e| ClawDBError::Reflect(e.to_string()))
    }

    /// Background loop: pushes recently added memories to the reflect service every 60 seconds.
    pub async fn start_memory_push_loop(
        &self,
        engine: Arc<claw_core::ClawEngine>,
        shutdown: tokio_util::sync::CancellationToken,
    ) {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => break,
                _ = interval.tick() => {
                    if !self.is_ready().await {
                        continue;
                    }
                    match engine.get_recent_memories("", 60).await {
                        Ok(memories) if !memories.is_empty() => {
                            let _ = self.push_memories(Uuid::nil(), memories).await;
                        }
                        Ok(_) => {}
                        Err(e) => tracing::warn!("memory push loop fetch error: {e}"),
                    }
                }
            }
        }
    }
}
