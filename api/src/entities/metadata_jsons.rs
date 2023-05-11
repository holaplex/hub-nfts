//! `SeaORM` Entity. Generated by sea-orm-codegen 0.11.0

use async_graphql::Result;
use sea_orm::entity::prelude::*;

/// The collection's associated metadata JSON.
/// ## References
/// [Metaplex v1.1.0 Standard](https://docs.metaplex.com/programs/token-metadata/token-standard)
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "metadata_jsons")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub identifier: String,
    /// The assigned name of the NFT.
    pub name: String,
    /// The URI for the complete metadata JSON.
    pub uri: String,
    /// The symbol of the NFT.
    pub symbol: String,
    /// The description of the NFT.
    pub description: String,
    /// The image URI for the NFT.
    pub image: String,
    /// An optional animated version of the NFT art.
    pub animation_url: Option<String>,
    /// An optional URL where viewers can find more information on the NFT, such as the collection's homepage or Twitter page.
    pub external_url: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::metadata_json_attributes::Entity")]
    MetadataJsonAttributes,
    #[sea_orm(has_many = "super::metadata_json_files::Entity")]
    MetadataJsonFiles,
}

impl Related<super::metadata_json_attributes::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MetadataJsonAttributes.def()
    }
}

impl Related<super::metadata_json_files::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MetadataJsonFiles.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
