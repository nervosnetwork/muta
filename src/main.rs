use log::info;
use logger;

fn main() {
    logger::init(logger::Flag::Main);

    info!("hello world");
}
