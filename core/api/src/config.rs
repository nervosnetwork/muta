use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct GraphQLConfig {
    pub listening_address: SocketAddr,

    pub graphql_uri:  String,
    pub graphiql_uri: String,
}

impl Default for GraphQLConfig {
    fn default() -> Self {
        Self {
            listening_address: "127.0.0.1:8080"
                .parse()
                .expect("Unable to parse socket address"),

            graphql_uri:  "/graphql".to_owned(),
            graphiql_uri: "/graphiql".to_owned(),
        }
    }
}
