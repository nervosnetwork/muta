use std::env;

use dotenv::dotenv;

use crate::service::error::ServiceError;

pub fn env_value<T>(var_name: &str) -> Result<T, ServiceError>
where
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::string::ToString,
{
    env_var(var_name)?
        .parse()
        .map_err(|e: <T as std::str::FromStr>::Err| ServiceError::Panic(e.to_string()))
}

pub fn env_var(var_name: &str) -> Result<String, ServiceError> {
    dotenv().ok();

    env::var(var_name)
        .map_err(|_| ServiceError::Panic(format!("{} environment variable not found", var_name)))
}
