/// Main configration for this crate.
#[derive(Clone)]
pub struct Config {
    /// Bind on specific address. Default: "127.0.0.1:8080"
    pub listen: String,

    /// Max payload size.         Default: 65536
    pub payload_size: usize,

    /// Number of workers.        Default: 4
    pub workers: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen:       String::from("127.0.0.1:8080"),
            payload_size: 65536,
            workers:      4,
        }
    }
}
