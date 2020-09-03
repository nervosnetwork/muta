use std::error::Error;
use std::fs;
use std::path::Path;

use chrono::prelude::Utc;
use log4rs::append::rolling_file::policy::compound::roll::Roll;
use log4rs::file::{Deserialize, Deserializers};

#[derive(serde_derive::Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct DateFixedWindowRollerConfig {
    pattern: String,
}

pub struct DateFixedWindowRollerBuilder;

impl DateFixedWindowRollerBuilder {
    pub fn build(
        self,
        pattern: &str,
    ) -> Result<DateFixedWindowRoller, Box<dyn Error + Sync + Send>> {
        if !pattern.contains("{date}") || !pattern.contains("{timestamp}") {
            return Err("pattern doesn't contain `{date}` or `{timestamp}`".into());
        }

        let roller = DateFixedWindowRoller {
            pattern: pattern.into(),
        };

        Ok(roller)
    }
}

/// The pattern takes two interpolation arguments, {date} and {timestamp}.
/// {date} and {timestamp} will be replaced with actual date and timestamp
/// value.
///
/// For example:
/// For pattern `log/{date}.muta.{timestamp}.log`, it will generate
/// `log/2020-08-27.muta.83748392743.log`.
#[derive(Debug)]
pub struct DateFixedWindowRoller {
    pattern: String,
}

impl DateFixedWindowRoller {
    pub fn builder() -> DateFixedWindowRollerBuilder {
        DateFixedWindowRollerBuilder
    }

    fn roll_file(
        &self,
        cur_log: &Path,
        date: &str,
        timestamp: &str,
    ) -> Result<(), Box<dyn Error + Sync + Send>> {
        let archived_log = {
            let pattern = self.pattern.clone();
            let partial_log = pattern.replace("{date}", date);
            partial_log.replace("{timestamp}", &timestamp)
        };

        if let Some(parent) = Path::new(&archived_log).parent() {
            fs::create_dir_all(parent)?;
        }

        match fs::rename(cur_log, &archived_log) {
            Ok(()) => return Ok(()),
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(_) => {}
        }

        // fall back to a copy
        fs::copy(cur_log, &archived_log).and_then(|_| fs::remove_file(cur_log))?;
        Ok(())
    }
}

impl Roll for DateFixedWindowRoller {
    fn roll(&self, cur_log: &Path) -> Result<(), Box<dyn Error + Sync + Send>> {
        let now = Utc::now();
        self.roll_file(
            cur_log,
            &now.format("%Y-%m-%d").to_string(),
            &now.timestamp().to_string(),
        )
    }
}

pub struct DateFixedWindowRollerDeserializer;

impl Deserialize for DateFixedWindowRollerDeserializer {
    type Config = DateFixedWindowRollerConfig;
    type Trait = dyn Roll;

    fn deserialize(
        &self,
        config: Self::Config,
        _: &Deserializers,
    ) -> Result<Box<Self::Trait>, Box<dyn Error + Sync + Send>> {
        let roll = DateFixedWindowRoller {
            pattern: config.pattern,
        };

        Ok(Box::new(roll))
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::{Read, Write};

    use chrono::prelude::Utc;

    use super::DateFixedWindowRoller;

    #[test]
    fn test_rotation() {
        let temp_dir = std::env::temp_dir();
        let pattern = format!(
            "{}/{{date}}.muta.{{timestamp}}.log",
            temp_dir.as_path().to_string_lossy()
        );
        let roller = DateFixedWindowRoller::builder().build(&pattern).unwrap();

        let test_log = {
            let mut temp_file = temp_dir.clone();
            temp_file.push("logger_test.log");
            temp_file
        };
        File::create(&test_log).unwrap().write_all(b"test").unwrap();

        let now = Utc::now();
        let date = &now.format("%Y-%m-%d").to_string();
        let timestamp = &now.timestamp().to_string();

        roller.roll_file(&test_log, &date, &timestamp).unwrap();
        assert!(!test_log.exists());

        let mut log_data = vec![];
        let archived_log = {
            let mut temp_file = temp_dir;
            temp_file.push(&format!("{}.muta.{}.log", &date, &timestamp));
            temp_file
        };

        File::open(archived_log)
            .unwrap()
            .read_to_end(&mut log_data)
            .unwrap();

        assert_eq!(log_data, b"test");
    }
}
