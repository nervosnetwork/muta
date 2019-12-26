use std::path::PathBuf;

use json::JsonValue;
use log::LevelFilter;
use log4rs::append::console::ConsoleAppender;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Logger, Root};
use log4rs::encode::json::JsonEncoder;
use log4rs::encode::pattern::PatternEncoder;

pub use json::array;
pub use json::object;

pub fn init(
    filter: String,
    log_to_console: bool,
    console_show_file_and_line: bool,
    log_to_file: bool,
    metrics: bool,
    log_path: PathBuf,
) {
    let console = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            if console_show_file_and_line {
                "[{d} {h({l})} {t} {f}:{L}] {m}{n}"
            } else {
                "[{d} {h({l})} {t}] {m}{n}"
            },
        )))
        .build();

    let file = FileAppender::builder()
        .encoder(Box::new(JsonEncoder::new()))
        .build(log_path.join("muta.log"))
        .unwrap();

    let metrics_appender = FileAppender::builder()
        .encoder(Box::new(JsonEncoder::new()))
        .build(log_path.join("metrics.log"))
        .unwrap();

    let mut root_builder = Root::builder();
    if log_to_console {
        root_builder = root_builder.appender("console");
    }
    if log_to_file {
        root_builder = root_builder.appender("file");
    }
    let level_filter = match filter.as_ref() {
        "off" => LevelFilter::Off,
        "error" => LevelFilter::Error,
        "info" => LevelFilter::Info,
        "warn" => LevelFilter::Warn,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        f => {
            println!("invalid logger.filter {}, use info", f);
            LevelFilter::Info
        }
    };
    let root = root_builder.build(level_filter);

    let metrics_logger = Logger::builder().additive(false).appender("metrics").build(
        "metrics",
        if metrics {
            LevelFilter::Trace
        } else {
            LevelFilter::Off
        },
    );

    let config = Config::builder()
        .appender(Appender::builder().build("console", Box::new(console)))
        .appender(Appender::builder().build("file", Box::new(file)))
        .appender(Appender::builder().build("metrics", Box::new(metrics_appender)))
        .logger(metrics_logger)
        .build(root)
        .unwrap();

    log4rs::init_config(config).unwrap();
}

pub fn metrics(name: &str, mut content: JsonValue) {
    log::trace!(target: "metrics", "{}", {
        content["name"] = name.into();
        content
    });
}
