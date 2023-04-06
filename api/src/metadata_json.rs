use hub_core::{anyhow::Result, prelude::anyhow, uuid::Uuid};
use metadata_jsons::Column as MetadataJsonColumn;
use sea_orm::{prelude::*, sea_query::OnConflict, Set, TransactionTrait};

use crate::{
    db::Connection,
    entities::{
        metadata_json_attributes, metadata_json_files, metadata_jsons,
        prelude::{MetadataJsonAttributes, MetadataJsonFiles, MetadataJsons},
    },
    nft_storage::NftStorageClient,
    objects::MetadataJsonInput,
};
#[derive(Clone, Debug)]
pub struct MetadataJson {
    metadata_json: MetadataJsonInput,
    id: Uuid,
    uri: Option<String>,
    identifier: Option<String>,
}

impl MetadataJson {
    #[must_use]
    pub fn new(
        id: Uuid,
        metadata_json: MetadataJsonInput,
        uri: Option<String>,
        identifier: Option<String>,
    ) -> Self {
        Self {
            metadata_json,
            id,
            uri,
            identifier,
        }
    }

    /// Fetches metadata from the database and constructs a `Self` instance.
    ///
    /// # Arguments
    ///
    /// * `id` - The ID of the metadata to fetch.
    /// * `db` - The database connection to use.
    ///
    /// # Errors
    ///
    /// This function fails if there is no matching `metadata_json` entry in the database
    /// or if it is unable to fetch related data from the database
    pub async fn fetch(id: Uuid, db: &Connection) -> Result<Self> {
        let (metadata_json_model, attributes) = metadata_jsons::Entity::find_by_id(id)
            .find_with_related(MetadataJsonAttributes)
            .all(db.get())
            .await?
            .first()
            .map(ToOwned::to_owned)
            .ok_or_else(|| anyhow!("no metadata_json entry found in db"))?;

        let files = metadata_json_files::Entity::find()
            .filter(metadata_json_files::Column::MetadataJsonId.eq(id))
            .all(db.get())
            .await?;

        let metadata_json = (metadata_json_model.clone(), attributes, Some(files)).into();

        Ok(Self {
            metadata_json,
            id,
            uri: Some(metadata_json_model.uri.clone()),
            identifier: Some(metadata_json_model.identifier),
        })
    }

    /// Res
    ///
    /// # Errors
    /// This function fails if unable to upload `metadata_json` to nft.storage
    pub async fn upload(&mut self, nft_storage: &NftStorageClient) -> Result<&Self> {
        let response = nft_storage.upload(self.metadata_json.clone()).await?;
        let cid = response.value.cid;

        let uri = nft_storage.ipfs_endpoint.join(&cid)?.to_string();

        self.uri = Some(uri);
        self.identifier = Some(cid);

        Ok(self)
    }

    /// Res
    ///
    /// # Errors
    /// This function fails if unable to save `metadata_json` to the db
    pub async fn save(&self, db: &Connection) -> Result<metadata_jsons::Model> {
        let id = self.id;
        let payload = self.metadata_json.clone();
        let identifier = self
            .identifier
            .clone()
            .ok_or_else(|| anyhow!("no identifier. call #upload before #save"))?;
        let uri = self
            .uri
            .clone()
            .ok_or_else(|| anyhow!("no uri. call #upload before #save"))?;

        let metadata_json_active_model = metadata_jsons::ActiveModel {
            id: Set(id),
            identifier: Set(identifier),
            name: Set(payload.name),
            uri: Set(uri),
            symbol: Set(payload.symbol),
            description: Set(payload.description),
            image: Set(payload.image),
            animation_url: Set(payload.animation_url),
            external_url: Set(payload.external_url),
        };

        let metadata_json = MetadataJsons::insert(metadata_json_active_model)
            .on_conflict(
                OnConflict::column(MetadataJsonColumn::Id)
                    .update_columns([
                        MetadataJsonColumn::Identifier,
                        MetadataJsonColumn::Name,
                        MetadataJsonColumn::Uri,
                        MetadataJsonColumn::Symbol,
                        MetadataJsonColumn::Description,
                        MetadataJsonColumn::Image,
                        MetadataJsonColumn::AnimationUrl,
                        MetadataJsonColumn::ExternalUrl,
                    ])
                    .clone(),
            )
            .exec_with_returning(db.get())
            .await?;

        let tx = db.get().clone().begin().await?;

        MetadataJsonAttributes::delete_many()
            .filter(metadata_json_attributes::Column::MetadataJsonId.eq(metadata_json.id))
            .exec(&tx)
            .await?;

        for attribute in payload.attributes {
            let am = metadata_json_attributes::ActiveModel {
                metadata_json_id: Set(metadata_json.id),
                trait_type: Set(attribute.trait_type),
                value: Set(attribute.value),
                ..Default::default()
            };

            am.insert(&tx).await?;
        }

        tx.commit().await?;

        if let Some(files) = payload.properties.unwrap_or_default().files {
            let tx = db.get().clone().begin().await?;

            MetadataJsonFiles::delete_many()
                .filter(metadata_json_files::Column::MetadataJsonId.eq(metadata_json.id))
                .exec(&tx)
                .await?;

            for file in files {
                let metadata_json_file_am = metadata_json_files::ActiveModel {
                    metadata_json_id: Set(metadata_json.id),
                    uri: Set(file.uri),
                    file_type: Set(file.file_type),
                    ..Default::default()
                };

                metadata_json_file_am.insert(&tx).await?;
            }

            tx.commit().await?;
        }

        Ok(metadata_json)
    }
}
