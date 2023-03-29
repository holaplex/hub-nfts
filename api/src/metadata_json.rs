use hub_core::{anyhow::Result, prelude::anyhow, uuid::Uuid};
use metadata_jsons::Column as MetadataJsonColumn;
use sea_orm::{prelude::*, sea_query::OnConflict, Set};

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
    collection: Uuid,
    uri: Option<String>,
    identifier: Option<String>,
}

impl MetadataJson {
    #[must_use]
    pub fn new(collection: Uuid, metadata_json: MetadataJsonInput) -> Self {
        Self {
            metadata_json,
            collection,
            uri: None,
            identifier: None,
        }
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
        let conn = db.get();
        let collection = self.collection;
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
            collection_id: Set(collection),
            identifier: Set(identifier),
            name: Set(payload.name),
            uri: Set(uri),
            symbol: Set(payload.symbol),
            description: Set(payload.description),
            image: Set(payload.image),
            animation_url: Set(payload.animation_url),
            external_url: Set(payload.external_url),
        };

        // let metadata_json = metadata_json_active_model.insert(conn).await?;

        let metadata_json = MetadataJsons::insert(metadata_json_active_model)
            .on_conflict(
                OnConflict::column(MetadataJsonColumn::CollectionId)
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
                    .to_owned(),
            )
            .exec_with_returning(db.get())
            .await?;

        MetadataJsonAttributes::delete_many()
            .filter(metadata_json_attributes::Column::CollectionId.eq(collection))
            .exec(db.get())
            .await?;

        for attribute in payload.attributes {
            let am = metadata_json_attributes::ActiveModel {
                collection_id: Set(collection),
                trait_type: Set(attribute.trait_type),
                value: Set(attribute.value),
                ..Default::default()
            };

            am.insert(conn).await?;
        }

        if let Some(files) = payload.properties.unwrap_or_default().files {
            MetadataJsonFiles::delete_many()
                .filter(metadata_json_files::Column::CollectionId.eq(collection))
                .exec(db.get())
                .await?;

            for file in files {
                let metadata_json_file_am = metadata_json_files::ActiveModel {
                    collection_id: Set(collection),
                    uri: Set(file.uri),
                    file_type: Set(file.file_type),
                    ..Default::default()
                };

                metadata_json_file_am.insert(conn).await?;
            }
        }

        Ok(metadata_json)
    }
}
