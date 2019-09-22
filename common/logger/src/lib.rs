use std::env;

pub enum Flag {
    Main, // Used in main function
    Test, // Used in test function
}

/// Function init should used in main or test function.
pub fn init(flag: Flag) {
    match flag {
        Flag::Main => {
            env::var("RUST_LOG").unwrap_or_else(|_| {
                env::set_var("RUST_LOG", "info");
                "info".into()
            });
            env_logger::init();
        }
        Flag::Test => {
            let _ = env_logger::builder().is_test(true).try_init();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{init, Flag};
    use log::{debug, error, info, trace, warn};

    #[test]
    fn test_debug() {
        init(Flag::Test);
        debug!("Hello World!");
        debug!("Hello {}", "World!");
        debug!("Hell{} {}", "o", "World!");
        debug!("Hell{} World", 0);
    }

    #[test]
    fn test_error() {
        init(Flag::Test);
        error!("Hello World!");
        error!("Hello {}", "World!");
        error!("Hell{} {}", "o", "World!");
        error!("Hell{} World", 0);
    }

    #[test]
    fn test_info() {
        init(Flag::Test);
        info!("Hello World!");
        info!("Hello {}", "World!");
        info!("Hell{} {}", "o", "World!");
        info!("Hell{} World", 0);
    }

    #[test]
    fn test_warn() {
        init(Flag::Test);
        warn!("Hello World!");
        warn!("Hello {}", "World!");
        warn!("Hell{} {}", "o", "World!");
        warn!("Hell{} World", 0);
    }

    #[test]
    fn test_trace() {
        init(Flag::Test);
        trace!("Hello World!");
        trace!("Hello {}", "World!");
        trace!("Hell{} {}", "o", "World!");
        trace!("Hell{} World", 0);
    }
}
