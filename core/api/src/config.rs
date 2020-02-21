use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct GraphQLConfig {
    pub listening_address: SocketAddr,

    pub graphql_uri:  String,
    pub graphiql_uri: String,

    // Set number of workers to start.
    // By default http server uses number of available logical cpu as threads count.
    pub workers: usize,

    // Sets the maximum per-worker number of concurrent connections.
    // All socket listeners will stop accepting connections when this limit is reached for each
    // worker. By default max connections is set to a 25k.
    pub maxconn: usize,
}

impl Default for GraphQLConfig {
    fn default() -> Self {
        Self {
            listening_address: "127.0.0.1:8080"
                .parse()
                .expect("Unable to parse socket address"),

            graphql_uri:  "/graphql".to_owned(),
            graphiql_uri: "/graphiql".to_owned(),
            workers:      num_cpus::get(),
            maxconn:      25000,
        }
    }
}
