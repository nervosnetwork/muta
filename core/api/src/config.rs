use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct GraphQLConfig {
    pub listening_address: SocketAddr,

    pub graphql_uri:  String,
    pub graphiql_uri: String,

    // Set number of workers to start.
    // By default http server uses number of available logical cpu as threads count.
    pub workers: usize,

    // Sets the maximum number of all concurrent connections.
    pub maxconn: usize,

    // Set the max payload size of graphql interface.
    // It is used to prevent DOS attacking through memory exhaustion.
    // The default value is 1024 * 1024, which is 1MB.
    pub max_payload_size: usize,

    pub tls: Option<GraphQLTLS>,
}

#[derive(Debug, Clone)]
pub struct GraphQLTLS {
    pub private_key_file_path:       PathBuf,
    pub certificate_chain_file_path: PathBuf,
}

impl Default for GraphQLConfig {
    fn default() -> Self {
        Self {
            listening_address: "127.0.0.1:8080"
                .parse()
                .expect("Unable to parse socket address"),

            graphql_uri:      "/graphql".to_owned(),
            graphiql_uri:     "/graphiql".to_owned(),
            workers:          num_cpus::get(),
            maxconn:          25000,
            max_payload_size: 1024 * 1024, // 1MB
            tls:              None,
        }
    }
}
