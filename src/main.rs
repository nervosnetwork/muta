use log::info;
use logger;
use serde_derive::Deserialize;
use std::error::Error;

#[derive(Debug, Deserialize)]
struct Config {}

fn main() -> Result<(), Box<dyn Error>> {
    logger::init(logger::Flag::Main);
    let matches = clap::App::new("Muta")
        .version("0.1")
        .author("Cryptape Technologies <contact@cryptape.com>")
        .arg(clap::Arg::from_usage(
            "-c --config=[FILE] 'a required file for the configuration'",
        ))
        .get_matches();

    let args_config = matches.value_of("config").unwrap_or("config.toml");
    let cfg: Config = config::from(args_config)?;
    info!("Config: {:?}", cfg);
    Ok(())
}
