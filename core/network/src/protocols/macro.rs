/// `Fn` protocol name generator
#[macro_export]
macro_rules! name {
    ($proto_name:expr) => {
        |id| format!("{}/{}", $proto_name, id)
    };
}

/// Create `Vec<String>` support versions from constant `[&str, N]`
#[macro_export]
macro_rules! support_versions {
    ($versions:expr) => {
        $versions.to_vec().into_iter().map(String::from).collect()
    };
}
