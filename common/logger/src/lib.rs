//! Crate rlog in a simple wrapper for `env_logger`, You can use it with
//! confidence.

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
