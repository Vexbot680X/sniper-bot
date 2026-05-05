use anyhow::Result;
use serde_json::json;
use tracing::warn;

#[derive(Clone)]
pub struct Telegram {
    enabled: bool,
    token: Option<String>,
    chat_id: Option<String>,
    client: reqwest::Client,
}

impl Telegram {
    pub fn from_env(enabled: bool) -> Self {
        let token = std::env::var("TELEGRAM_BOT_TOKEN").ok();
        let chat_id = std::env::var("TELEGRAM_CHAT_ID").ok();
        if enabled && (token.is_none() || chat_id.is_none()) {
            warn!("TELEGRAM_BOT_TOKEN or TELEGRAM_CHAT_ID not set — telegram alerts disabled");
        }
        Self {
            enabled: enabled && token.is_some() && chat_id.is_some(),
            token, chat_id,
            client: reqwest::Client::new(),
        }
    }

    pub async fn send(&self, msg: &str) -> Result<()> {
        if !self.enabled { return Ok(()); }
        let token = self.token.as_ref().unwrap();
        let chat_id = self.chat_id.as_ref().unwrap();
        let url = format!("https://api.telegram.org/bot{token}/sendMessage");
        let _ = self.client.post(&url)
            .json(&json!({
                "chat_id": chat_id,
                "text": msg,
                "parse_mode": "Markdown",
                "disable_web_page_preview": true
            }))
            .send().await?;
        Ok(())
    }
}
