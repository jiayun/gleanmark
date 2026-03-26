use std::process::{Child, Command};
use std::time::Duration;

use qdrant_client::Qdrant;
use tracing::{info, warn};

use crate::error::{Error, Result};
use crate::models::Config;

pub struct QdrantManager {
    child: Option<Child>,
    url: String,
}

impl QdrantManager {
    pub async fn start(config: &Config) -> Result<Self> {
        let url = &config.qdrant_url;

        // Check if Qdrant is already running
        if Self::health_check(url).await.is_ok() {
            info!("Qdrant already running at {url}");
            return Ok(Self {
                child: None,
                url: url.clone(),
            });
        }

        // Find qdrant binary
        let binary = Self::find_binary(config)?;
        info!("Starting Qdrant from {}", binary.display());

        let storage_path = config.data_dir.join("qdrant_storage");
        std::fs::create_dir_all(&storage_path)?;

        let child = Command::new(&binary)
            .arg("--disable-telemetry")
            .env("QDRANT__STORAGE__STORAGE_PATH", &storage_path)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| Error::Other(format!("Failed to start Qdrant: {e}")))?;

        info!("Qdrant process started (pid: {})", child.id());

        let mut manager = Self {
            child: Some(child),
            url: url.clone(),
        };

        // Wait for Qdrant to be ready
        manager.wait_ready().await?;
        info!("Qdrant is ready at {url}");

        Ok(manager)
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    async fn health_check(url: &str) -> Result<()> {
        let client = Qdrant::from_url(url).build()?;
        client.health_check().await?;
        Ok(())
    }

    async fn wait_ready(&mut self) -> Result<()> {
        let max_retries = 30;
        let delay = Duration::from_millis(500);

        for i in 0..max_retries {
            // Check if process has exited
            if let Some(ref mut child) = self.child {
                if let Some(status) = child.try_wait()? {
                    return Err(Error::Other(format!(
                        "Qdrant process exited with status: {status}"
                    )));
                }
            }

            if Self::health_check(&self.url).await.is_ok() {
                return Ok(());
            }

            if i > 0 && i % 10 == 0 {
                warn!("Still waiting for Qdrant to start (attempt {i}/{max_retries})...");
            }
            tokio::time::sleep(delay).await;
        }

        Err(Error::Other(
            "Qdrant failed to start within 15 seconds".to_string(),
        ))
    }

    fn find_binary(config: &Config) -> Result<std::path::PathBuf> {
        // 1. Check config data_dir (macOS: ~/Library/Application Support/gleanmark/bin/)
        let data_dir_bin = config.data_dir.join("bin").join("qdrant");
        if data_dir_bin.exists() {
            return Ok(data_dir_bin);
        }

        // 2. Check XDG-style path (~/.local/share/gleanmark/bin/)
        if let Some(home) = dirs::home_dir() {
            let xdg_bin = home
                .join(".local")
                .join("share")
                .join("gleanmark")
                .join("bin")
                .join("qdrant");
            if xdg_bin.exists() {
                return Ok(xdg_bin);
            }
        }

        // 3. Check PATH
        if let Ok(path) = which("qdrant") {
            return Ok(path);
        }

        Err(Error::Other(format!(
            "Qdrant binary not found. Place it in one of:\n\
             - {}\n\
             - ~/.local/share/gleanmark/bin/qdrant\n\
             - Anywhere on $PATH\n\
             Download from: https://github.com/qdrant/qdrant/releases",
            data_dir_bin.display()
        )))
    }
}

fn which(binary: &str) -> std::result::Result<std::path::PathBuf, ()> {
    let path_var = std::env::var("PATH").map_err(|_| ())?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(binary);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(())
}

impl Drop for QdrantManager {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.child {
            info!("Stopping Qdrant process (pid: {})", child.id());
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
