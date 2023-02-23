use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql_poem::{GraphQLRequest, GraphQLResponse};
use hub_core::anyhow::Result;
use poem::{
    handler,
    web::{Data, Html},
    IntoResponse,
};

use crate::{AppContext, AppState, UserID};

#[handler]
pub fn health() {}

#[handler]
pub fn playground() -> impl IntoResponse {
    Html(playground_source(GraphQLPlaygroundConfig::new("/graphql")))
}

#[handler]
pub async fn graphql_handler(
    Data(state): Data<&AppState>,
    user_id: UserID,
    req: GraphQLRequest,
) -> Result<GraphQLResponse> {
    let context = AppContext::new(state.connection.clone(), user_id);

    Ok(state
        .schema
        .execute(
            req.0
                .data(context)
                .data(state.rpc.clone())
                .data(state.producer.clone())
                .data(state.keypair.clone()),
        )
        .await
        .into())
}
