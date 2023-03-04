use async_graphql::InputObject;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, InputObject)]
pub struct MetadataJson {
    pub name: String,
    pub symbol: String,
    pub description: String,
    pub image: String,
    pub animation_url: Option<String>,
    pub collection: Option<Collection>,
    pub attributes: Vec<Attribute>,
    pub external_url: Option<String>,
    pub properties: Property,
}

#[derive(Clone, Debug, Serialize, Deserialize, InputObject)]
#[graphql(name = "MetadataJsonFile")]
pub struct File {
    pub uri: Option<String>,
    #[serde(rename = "type")]
    pub file_type: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, InputObject)]
#[graphql(name = "MetadataJsonProperty")]
pub struct Property {
    pub files: Option<Vec<File>>,
    pub category: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, InputObject)]
#[graphql(name = "MetadataJsonAttribute")]
pub struct Attribute {
    pub trait_type: String,
    pub value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, InputObject)]
#[graphql(name = "MetadataJsonCollection")]
pub struct Collection {
    pub name: Option<String>,
    pub family: Option<String>,
}
