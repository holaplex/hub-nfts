use async_graphql::{ComplexObject, Context, InputObject, Result, SimpleObject};
use hub_core::{assets::AssetProxy, uuid::Uuid};
use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::{
    entities::{
        metadata_json_attributes::{self, Model as MetadataJsonAttributeModel},
        metadata_json_files::Model as MetadataJsonFileModel,
        metadata_jsons::{self, Model as MetadataJsonModel},
    },
    AppContext,
};

/// The collection's associated metadata JSON.
/// ## References
/// [Metaplex v1.1.0 Standard](https://docs.metaplex.com/programs/token-metadata/token-standard)
#[derive(Clone, Debug, PartialEq, Eq, SimpleObject)]
#[graphql(complex, concrete(name = "MetadataJson", params()))]
pub struct MetadataJson {
    pub id: Uuid,
    /// The assigned name of the NFT.
    pub name: String,
    /// The symbol of the NFT.
    pub symbol: String,
    /// The description of the NFT.
    pub description: String,
    /// The image URI for the NFT.
    pub image_original: String,
    /// An optional animated version of the NFT art.
    pub animation_url: Option<String>,
    /// An optional URL where viewers can find more information on the NFT, such as the collection's homepage or Twitter page.
    pub external_url: Option<String>,
}

#[ComplexObject]
impl MetadataJson {
    // The NFT's associated attributes.
    async fn attributes(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<Vec<metadata_json_attributes::Model>>> {
        let AppContext {
            metadata_json_attributes_loader,
            ..
        } = ctx.data::<AppContext>()?;

        metadata_json_attributes_loader.load_one(self.id).await
    }

    async fn image(&self, ctx: &Context<'_>) -> Result<String> {
        let asset_proxy = ctx.data::<AssetProxy>()?;
        let url = Url::parse(&self.image_original)?;

        asset_proxy
            .proxy_ipfs_image(&url, None)
            .map_err(Into::into)
            .map(|u| u.map_or(self.image_original.clone(), Into::into))
    }
}

impl From<metadata_jsons::Model> for MetadataJson {
    fn from(
        metadata_jsons::Model {
            id,
            name,
            symbol,
            description,
            image,
            animation_url,
            external_url,
        }: metadata_jsons::Model,
    ) -> Self {
        Self {
            id,
            name,
            symbol,
            description,
            image_original: image,
            animation_url,
            external_url,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, InputObject)]
pub struct MetadataJsonInput {
    pub name: String,
    pub symbol: String,
    pub description: String,
    pub image: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animation_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection: Option<Collection>,
    pub attributes: Vec<Attribute>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
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
