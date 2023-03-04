use async_graphql::InputObject;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, InputObject)]
#[graphql(name = "CollectionCreatorInput")]
pub struct CollectionCreator {
    pub address: String,
    pub verified: Option<bool>,
    pub share: u8,
}
