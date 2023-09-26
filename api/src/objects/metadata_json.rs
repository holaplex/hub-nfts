use async_graphql::{ComplexObject, Context, InputObject, Result, SimpleObject};
use hub_core::{assets::AssetProxy, uuid::Uuid};
use reqwest::Url;
use sea_orm::{prelude::*, sea_query::OnConflict, DatabaseTransaction, Set};
use serde::{Deserialize, Serialize};

use crate::{
    entities::{metadata_json_attributes, metadata_json_files, metadata_jsons},
    AppContext,
};

/// The collection's associated metadata JSON.
/// ## References
/// [Metaplex v1.1.0 Standard](https://docs.metaplex.com/programs/token-metadata/token-standard)
#[derive(Clone, Debug, PartialEq, Eq, SimpleObject)]
#[graphql(complex, concrete(name = "MetadataJson", params()))]
pub struct MetadataJson {
    // The id of the metadata json.
    pub id: Uuid,
    // The assigned identifier of the metadata json uri.
    pub identifier: Option<String>,
    /// The assigned name of the NFT.
    pub name: String,
    /// The URI for the complete metadata JSON.
    pub uri: Option<String>,
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

impl MetadataJsonInput {
    /// Saves the metadata json to the database. If the metadata json already exists, it will update the existing record.
    /// # Arguments
    /// * `id` - The id of the metadata json
    /// * `tx` - The database transaction to use
    /// # Returns
    /// Returns Ok with ()
    /// # Errors
    /// Returns Err if unable to save records to the database
    pub async fn save(&self, id: Uuid, tx: &DatabaseTransaction) -> Result<()> {
        let metadata_json = self.clone();
        let metadata_json_active_model = metadata_jsons::ActiveModel {
            id: Set(id),
            identifier: Set(None),
            name: Set(metadata_json.name),
            uri: Set(None),
            symbol: Set(metadata_json.symbol),
            description: Set(metadata_json.description),
            image: Set(metadata_json.image),
            animation_url: Set(metadata_json.animation_url),
            external_url: Set(metadata_json.external_url),
        };

        let metadata_json_model = metadata_jsons::Entity::insert(metadata_json_active_model)
            .on_conflict(
                OnConflict::column(metadata_jsons::Column::Id)
                    .update_columns([
                        metadata_jsons::Column::Identifier,
                        metadata_jsons::Column::Name,
                        metadata_jsons::Column::Uri,
                        metadata_jsons::Column::Symbol,
                        metadata_jsons::Column::Description,
                        metadata_jsons::Column::Image,
                        metadata_jsons::Column::AnimationUrl,
                        metadata_jsons::Column::ExternalUrl,
                    ])
                    .clone(),
            )
            .exec_with_returning(tx)
            .await?;

        metadata_json_attributes::Entity::delete_many()
            .filter(metadata_json_attributes::Column::MetadataJsonId.eq(metadata_json_model.id))
            .exec(tx)
            .await?;

        for attribute in metadata_json.attributes {
            let am = metadata_json_attributes::ActiveModel {
                metadata_json_id: Set(metadata_json_model.id),
                trait_type: Set(attribute.trait_type),
                value: Set(attribute.value),
                ..Default::default()
            };

            am.insert(tx).await?;
        }

        if let Some(files) = metadata_json.properties.unwrap_or_default().files {
            metadata_json_files::Entity::delete_many()
                .filter(metadata_json_files::Column::MetadataJsonId.eq(metadata_json_model.id))
                .exec(tx)
                .await?;

            for file in files {
                let metadata_json_file_am = metadata_json_files::ActiveModel {
                    metadata_json_id: Set(metadata_json_model.id),
                    uri: Set(file.uri),
                    file_type: Set(file.file_type),
                    ..Default::default()
                };

                metadata_json_file_am.insert(tx).await?;
            }
        }

        Ok(())
    }
}

impl From<metadata_jsons::Model> for MetadataJson {
    fn from(
        metadata_jsons::Model {
            id,
            identifier,
            name,
            uri,
            symbol,
            description,
            image,
            animation_url,
            external_url,
        }: metadata_jsons::Model,
    ) -> Self {
        Self {
            id,
            identifier,
            name,
            uri,
            symbol,
            description,
            image_original: image,
            animation_url,
            external_url,
        }
    }
}
