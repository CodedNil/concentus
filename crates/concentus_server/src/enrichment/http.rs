use reqwest::{Client, StatusCode, Url, header::RETRY_AFTER};
use serde::de::DeserializeOwned;
use std::{env, sync::Arc, time::Duration};
use tokio::{
    sync::Mutex,
    time::{Instant, sleep_until},
};
use topcoat::{Error, Result};

#[derive(Clone)]
pub struct Source {
    client: Client,
    url: Url,
    next: Arc<Mutex<Instant>>,
}

impl Source {
    pub fn new(client: &Client, variable: &str, default: &str) -> Result<Self> {
        let url = match env::var(variable) {
            Ok(value) => Url::parse(&value)?,
            Err(env::VarError::NotPresent) => Url::parse(default)?,
            Err(error) => return Err(error.into()),
        };
        Ok(Self {
            client: client.clone(),
            url,
            next: Arc::new(Mutex::new(Instant::now())),
        })
    }

    pub async fn get<T: DeserializeOwned>(&self, params: &[(&str, &str)]) -> Result<Option<T>> {
        let mut next = self.next.lock().await;
        if *next > Instant::now() + Duration::from_secs(2) {
            return Err(Error::msg("Provider asked us to retry later"));
        }
        sleep_until(*next).await;
        // Pace request attempts even when the caller cancels the response.
        *next = Instant::now() + Duration::from_millis(750);
        let response = self
            .client
            .get(self.url.clone())
            .query(params)
            .send()
            .await
            .map_err(reqwest::Error::without_url)?;
        let status = response.status();
        tracing::debug!(
            host = self.url.host_str(),
            status = status.as_u16(),
            "Provider responded"
        );
        if matches!(
            status,
            StatusCode::TOO_MANY_REQUESTS | StatusCode::SERVICE_UNAVAILABLE
        ) {
            let delay = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|header| header.to_str().ok())
                .and_then(|header| header.parse::<u64>().ok())
                .unwrap_or(30);
            *next = Instant::now() + Duration::from_secs(delay.saturating_add(1).min(86_400));
        }
        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let value = response
            .error_for_status()
            .map_err(reqwest::Error::without_url)?
            .json()
            .await
            .map_err(reqwest::Error::without_url)?;
        *next = Instant::now() + Duration::from_millis(750);
        drop(next);
        Ok(Some(value))
    }
}
