use async_graphql::InputObject;
use serde::{Deserialize, Serialize};

use crate::entities::{
    metadata_json_attributes::Model as MetadataJsonAttributeModel,
    metadata_json_files::Model as MetadataJsonFileModel,
    metadata_jsons::Model as MetadataJsonModel,
};

#[derive(Clone, Debug, Serialize, Deserialize, InputObject)]
pub struct MetadataJsonInput {
    pub name: String,
    pub symbol: String,
    pub description: String,
    pub image: String,
    pub animation_url: Option<String>,
    pub collection: Option<Collection>,
    pub attributes: Vec<Attribute>,
    pub external_url: Option<String>,
    pub properties: Option<Property>,
}

#[derive(Clone, Debug, Serialize, Deserialize, InputObject)]
#[graphql(name = "MetadataJsonFileInput")]
pub struct File {
    pub uri: Option<String>,
    #[serde(rename = "type")]
    pub file_type: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, InputObject, Default)]
#[graphql(name = "MetadataJsonPropertyInput")]
pub struct Property {
    pub files: Option<Vec<File>>,
    pub category: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, InputObject)]
#[graphql(name = "MetadataJsonAttributeInput")]
pub struct Attribute {
    pub trait_type: String,
    pub value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, InputObject)]
#[graphql(name = "MetadataJsonCollectionInput")]
pub struct Collection {
    pub name: Option<String>,
    pub family: Option<String>,
}

impl
    From<(
        MetadataJsonModel,
        Vec<MetadataJsonAttributeModel>,
        Option<Vec<MetadataJsonFileModel>>,
    )> for MetadataJsonInput
{
    fn from(
        (metadata_json, attributes, files): (
            MetadataJsonModel,
            Vec<MetadataJsonAttributeModel>,
            Option<Vec<MetadataJsonFileModel>>,
        ),
    ) -> Self {
        let input = MetadataJsonInput {
            name: metadata_json.name,
            symbol: metadata_json.symbol,
            description: metadata_json.description,
            image: metadata_json.image,
            animation_url: metadata_json.animation_url,
            collection: None,
            attributes: attributes.iter().map(|a| a.clone().into()).collect(),
            external_url: metadata_json.external_url,
            properties: Some(Property {
                files: files.map(|files| files.iter().map(|f| f.clone().into()).collect()),
                category: None,
            }),
        };
        input
    }
}

impl From<MetadataJsonAttributeModel> for Attribute {
    fn from(attribute: MetadataJsonAttributeModel) -> Self {
        Attribute {
            trait_type: attribute.trait_type,
            value: attribute.value,
        }
    }
}

impl From<MetadataJsonFileModel> for File {
    fn from(file: MetadataJsonFileModel) -> Self {
        File {
            uri: file.uri,
            file_type: file.file_type,
        }
    }
}
