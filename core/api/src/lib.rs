pub mod adapter;

use std::sync::{atomic, Arc};

use http::status::StatusCode;
use juniper::graphiql::graphiql_source;
use juniper::graphql_object;
use tide::{error::ResultExt, response, App, Context, EndpointResult};

// First, we define `State` that holds accumulator state. This is accessible as
// state in Tide, and as executor context in Juniper.
#[derive(Clone, Default)]
struct State(Arc<atomic::AtomicIsize>);

impl juniper::Context for State {}

// We define `Query` unit struct here. GraphQL queries will refer to this
// struct. The struct itself doesn't have any associated state (and there's no
// need to do so), but instead it exposes the accumulator state from the
// context.
struct Query;

graphql_object!(Query: State |&self| {
    // GraphQL integers are signed and 32 bits long.
    field accumulator(&executor) -> i32 as "Current value of the accumulator" {
        executor.context().0.load(atomic::Ordering::Relaxed) as i32
    }
});

// Here is `Mutation` unit struct. GraphQL mutations will refer to this struct.
// This is similar to `Query`, but it provides the way to "mutate" the
// accumulator state.
struct Mutation;

graphql_object!(Mutation: State |&self| {
    field add(&executor, by: i32) -> i32 as "Add given value to the accumulator." {
        executor.context().0.fetch_add(by as isize, atomic::Ordering::Relaxed) as i32 + by
    }
    field add2(&executor, by: i32) -> i32 as "Add given value to the accumulator." {
        executor.context().0.fetch_add(by as isize, atomic::Ordering::Relaxed) as i32 + by
    }
});

// Adding `Query` and `Mutation` together we get `Schema`, which describes,
// well, the whole GraphQL schema.
type Schema = juniper::RootNode<'static, Query, Mutation>;

// Finally, we'll bridge between Tide and Juniper. `GraphQLRequest` from Juniper
// implements `Deserialize`, so we use `Json` extractor to deserialize the
// request body.
async fn handle_graphql(mut cx: Context<State>) -> EndpointResult {
    let query: juniper::http::GraphQLRequest = cx.body_json().await.client_err()?;
    let schema = Schema::new(Query, Mutation);
    let response = query.execute(&schema, cx.state());
    let status = if response.is_ok() {
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    };
    let mut resp = response::json(response);
    *resp.status_mut() = status;
    Ok(resp)
}

async fn handle_graphiql(_ctx: Context<State>) -> tide::http::Response<String> {
    let html = graphiql_source("/graphql");

    tide::http::Response::builder()
        .status(http::status::StatusCode::OK)
        .header("Content-Type", "text/html")
        .body(html)
        .unwrap()
}

pub async fn start_rpc() {
    let mut app = App::with_state(State::default());

    app.at("/graphiql").get(handle_graphiql);
    app.at("/graphql").post(handle_graphql);
    app.serve("127.0.0.1:8000").await.unwrap();
}
