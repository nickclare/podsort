use std::{io::BufReader, path::Path, time::SystemTime};

use bytes::Buf;
use color_eyre::eyre::{self, Context};

use super::*;

#[derive(Clone, Debug)]
pub struct Runner {
    cache_path: PathBuf,
    client: reqwest::Client,
}

impl Runner {
    pub fn new(cache_path: impl AsRef<Path>) -> Self {
        Self {
            cache_path: cache_path.as_ref().to_owned(),
            client: reqwest::Client::new(),
        }
    }

    pub async fn run_job(&self, job: &PodcastJob) -> eyre::Result<Vec<String>> {
        let _source = self.fetch_source(job).await?;

        todo!()
    }

    /// Return the fetched channel source, unless it is unchanged from the currently
    /// cached channel data
    async fn fetch_source(&self, job: &PodcastJob) -> eyre::Result<Option<rss::Channel>> {
        let id = &job.id;
        let cached_source = self.cache_path.join(format!("{id}.rss"));
        let cached_timestamp = if cached_source.exists() {
            cached_source
                .metadata()
                .ok()
                .and_then(|e| e.modified().ok())
                .and_then(|st| st.duration_since(SystemTime::UNIX_EPOCH).ok())
                .and_then(|d| {
                    chrono::DateTime::from_timestamp(d.as_secs() as i64, d.subsec_nanos())
                })
        } else {
            None
        };

        let req = self
            .client
            .get(job.source.clone())
            .header(http::header::USER_AGENT, "podsort/v1");
        let req = if let Some(cached_timestamp) = cached_timestamp {
            req.header(
                http::header::IF_MODIFIED_SINCE,
                format!("{}", cached_timestamp.format("%a, %d %b %Y %H:%M:%S GMT")),
            )
        } else {
            req
        };

        let resp = req.send().await?;
        if resp.status() == http::StatusCode::NOT_MODIFIED {
            Ok(None)
        } else {
            let body = BufReader::new(resp.bytes().await?.reader());
            Ok(Some(
                rss::Channel::read_from(body).wrap_err("invalid rss feed")?,
            ))
        }
    }
}
