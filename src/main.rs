use podsort::{PodcastJob, runner::Runner};

#[tokio::main]
async fn main() -> color_eyre::eyre::Result<()> {
    let job = PodcastJob {
        id: "local-sample".into(),
        source: "http://localhost:1234".parse()?,
        outputs: vec![],
    };
    Runner::new("/tmp").run_job(&job).await?;
    Ok(())
}
