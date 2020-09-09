use std::env;
use std::time::Duration;

use derive_more::{Display, From};
use lazy_static::lazy_static;
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;

lazy_static! {
    static ref PROFILER_MUTEX: Mutex<bool> = Mutex::new(false);
}

#[derive(Debug, Display, From)]
pub enum Error {
    #[display(fmt = "profile: {}", _0)]
    Profile(pprof::Error),

    #[display(fmt = "io: {}", _0)]
    Io(std::io::Error),
}

pub struct Profiler;

impl Profiler {
    pub async fn dump_pprof(duration: Duration, frequency: i32) -> Result<Vec<u8>, Error> {
        let guard = pprof::ProfilerGuard::new(frequency)?;
        tokio::time::delay_for(duration).await;

        let profile_path = {
            let mut path = env::temp_dir();
            path.push("muta_profile.svg");

            path
        };
        // Report::flamegraph only implement io::Write
        let profile_file = std::fs::File::create(&profile_path)?;

        let report = guard.report().build()?;
        report.flamegraph(profile_file)?;
        drop(guard);

        let mut profile_file = tokio::fs::File::open(&profile_path).await?;
        let mut buf = Vec::new();
        profile_file.read_to_end(&mut buf).await?;
        Ok(buf)
    }
}
