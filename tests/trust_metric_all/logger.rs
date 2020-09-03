use std::{collections::HashMap, path::PathBuf};

const LOGGER_FILTER: &str = "warn";
const LOGGER_LOG_TO_CONSOLE: bool = true;
const LOGGER_CONSOLE_SHOW_FILE_AND_LINE: bool = false;
const LOGGER_LOG_TO_FILE: bool = false;
const LOGGER_METRICS: bool = false;
const LOGGER_FILE_SIZE_LIMIT: u64 = 1024 * 1024 * 1024;

#[allow(dead_code)]
pub fn init() {
    let log_path = PathBuf::new();

    let mut modules_level = HashMap::new();
    modules_level.insert("core_network".to_owned(), "debug".to_owned());

    common_logger::init(
        LOGGER_FILTER.to_owned(),
        LOGGER_LOG_TO_CONSOLE,
        LOGGER_CONSOLE_SHOW_FILE_AND_LINE,
        LOGGER_LOG_TO_FILE,
        LOGGER_METRICS,
        log_path,
        LOGGER_FILE_SIZE_LIMIT,
        modules_level,
    )
}
