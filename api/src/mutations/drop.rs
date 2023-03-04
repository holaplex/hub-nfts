use std::sync::Arc;

use async_graphql::{self, Context, Error, InputObject, Object, Result};
use chrono::{DateTime, Local, Utc};
use hub_core::producer::Producer;
use mpl_token_metadata::state::Creator;
use sea_orm::{prelude::*, Set};
use serde::{Deserialize, Serialize};
use solana_client::rpc_client::RpcClient;
use solana_program::{program_pack::Pack, pubkey::Pubkey};
use solana_sdk::signer::{keypair::Keypair, Signer};
use spl_associated_token_account::get_associated_token_address;

use crate::{
    db::Connection,
    entities::{
        collection_attributes, collection_creators, collections, drops, metadata_json_files,
        metadata_jsons,
        prelude::ProjectWallets,
        project_wallets,
        sea_orm_active_enums::{Blockchain, CreationStatus},
        solana_collections,
    },
    nft_storage::NftStorageClient,
    proto::{self, nft_events, NftEventKey, NftEvents},
    AppContext, UserID,
};

#[derive(Default)]
pub struct Mutation;

#[Object(name = "DropMutation")]
impl Mutation {
    /// Res
    ///
    /// # Errors
    /// This function fails if ...
    pub async fn create_drop(
        &self,
        ctx: &Context<'_>,
        input: CreateDropInput,
    ) -> Result<drops::Model> {
        let AppContext { db, user_id, .. } = ctx.data::<AppContext>()?;
        let UserID(id) = user_id;
        let producer = ctx.data::<Producer<NftEvents>>()?;
        let rpc = &**ctx.data::<Arc<RpcClient>>()?;
        let nft_storage = ctx.data::<NftStorageClient>()?;
        let keypair_bytes = ctx.data::<Vec<u8>>()?;

        let user_id = id.ok_or_else(|| Error::new("X-USER-ID header not found"))?;

        let payer = Keypair::from_bytes(keypair_bytes)?;
        let mint = Keypair::new();

        let (uri, cid) = upload_metadata_json(nft_storage, input.metadata_json.clone()).await?;

        let wallets = ProjectWallets::find()
            .filter(
                project_wallets::Column::ProjectId
                    .eq(input.project_id)
                    .and(project_wallets::Column::Blockchain.eq(input.blockchain)),
            )
            .all(db.get())
            .await?;

        if wallets.len() > 1 {
            return Err(Error::new("More than one wallet found"));
        }

        let owner_address = &wallets
            .get(0)
            .ok_or_else(|| Error::new("no wallet found"))?
            .wallet_address;
        let owner = owner_address.parse()?;

        let ata = get_associated_token_address(&owner, &mint.pubkey());

        let (token_metadata_pubkey, _) = solana_program::pubkey::Pubkey::find_program_address(
            &[
                b"metadata",
                mpl_token_metadata::ID.as_ref(),
                mint.pubkey().as_ref(),
            ],
            &mpl_token_metadata::ID,
        );

        let (master_edition_pubkey, _) = solana_program::pubkey::Pubkey::find_program_address(
            &[
                b"metadata",
                mpl_token_metadata::ID.as_ref(),
                mint.pubkey().as_ref(),
                b"edition",
            ],
            &mpl_token_metadata::ID,
        );

        let len = spl_token::state::Mint::LEN;

        let rent = rpc.get_minimum_balance_for_rent_exemption(len)?;

        let create_account_ins = solana_program::system_instruction::create_account(
            &payer.pubkey(),
            &mint.pubkey(),
            rent,
            len.try_into()?,
            &spl_token::ID,
        );

        let initialize_mint_ins = spl_token::instruction::initialize_mint(
            &spl_token::ID,
            &mint.pubkey(),
            &owner,
            Some(&owner),
            0,
        )?;

        let ata_ins = spl_associated_token_account::instruction::create_associated_token_account(
            &payer.pubkey(),
            &owner,
            &mint.pubkey(),
            &spl_token::ID,
        );

        let min_to_ins =
            spl_token::instruction::mint_to(&spl_token::ID, &mint.pubkey(), &ata, &owner, &[], 1)?;

        let creators = input.creators.clone().as_ref().map(|creators| {
            creators
                .iter()
                .map(|creator| creator.clone().try_into().unwrap())
                .collect()
        });
        let create_metadata_account_ins =
            mpl_token_metadata::instruction::create_metadata_accounts_v3(
                mpl_token_metadata::ID,
                token_metadata_pubkey,
                mint.pubkey(),
                owner,
                payer.pubkey(),
                owner,
                input.metadata_json.name.clone(),
                input.metadata_json.symbol.clone(),
                uri.clone(),
                creators,
                input.seller_fee_basis_points,
                input.update_authority_is_signer,
                input.is_mutable,
                None,
                None,
                None,
            );

        let create_master_edition_ins = mpl_token_metadata::instruction::create_master_edition_v3(
            mpl_token_metadata::ID,
            master_edition_pubkey,
            mint.pubkey(),
            owner,
            owner,
            token_metadata_pubkey,
            payer.pubkey(),
            input.supply,
        );

        let blockhash = rpc.get_latest_blockhash()?;

        let instructions = &[
            create_account_ins,
            initialize_mint_ins,
            ata_ins,
            min_to_ins,
            create_metadata_account_ins,
            create_master_edition_ins,
        ];

        let message = solana_program::message::Message::new_with_blockhash(
            instructions,
            Some(&payer.pubkey()),
            &blockhash,
        );

        let serialized_message = message.serialize();
        let mint_signature = mint.try_sign_message(&message.serialize())?;
        let payer_signature = payer.try_sign_message(&message.serialize())?;

        let (collection, _, drop) = index_drop_and_collection(
            db,
            input.clone(),
            uri,
            user_id,
            master_edition_pubkey,
            ata,
            owner,
            mint.pubkey(),
            token_metadata_pubkey,
        )
        .await?;

        index_metadata_json(db, input.metadata_json.clone(), collection.id, cid).await?;

        if let Some(creators) = input.creators {
            index_creators(db, creators, collection.id).await?;
        }

        emit_drop_transaction_event(
            producer,
            drop.id,
            user_id,
            serialized_message,
            vec![payer_signature.to_string(), mint_signature.to_string()],
            input.project_id,
        )
        .await?;

        Ok(drop)
    }
}

/// This functions emits the drop transaction event
/// # Errors
/// This function fails if producer is unable to sent the event
pub async fn emit_drop_transaction_event(
    producer: &Producer<NftEvents>,
    id: Uuid,
    user_id: Uuid,
    serialized_message: Vec<u8>,
    signatures: Vec<String>,
    project_id: Uuid,
) -> Result<()> {
    let event = NftEvents {
        event: Some(nft_events::Event::CreateDrop(proto::DropTransaction {
            transaction: Some(proto::Transaction {
                serialized_message,
                signed_message_signatures: signatures,
            }),
            project_id: project_id.to_string(),
        })),
    };
    let key = NftEventKey {
        id: id.to_string(),
        user_id: user_id.to_string(),
    };

    producer.send(Some(&event), Some(&key)).await?;

    Ok(())
}

/// This functions indexes the collection, `solana_collection` and drop
/// # Errors
/// This function fails if insert fails
#[allow(clippy::too_many_arguments)]
pub async fn index_drop_and_collection(
    db: &Connection,
    input: CreateDropInput,
    uri: String,
    user_id: Uuid,
    master_edition_pubkey: Pubkey,
    ata: Pubkey,
    owner: Pubkey,
    mint: Pubkey,
    token_metadata_pubkey: Pubkey,
) -> Result<(collections::Model, solana_collections::Model, drops::Model)> {
    let collection_active_model = collections::ActiveModel {
        blockchain: Set(input.blockchain),
        name: Set(input.metadata_json.name.clone()),
        description: Set(input.metadata_json.description.clone()),
        metadata_uri: Set(uri),
        royalty_wallet: Set(input.royalty_address.to_string()),
        supply: Set(input.supply.map(|s| s.try_into().unwrap_or_default())),
        creation_status: Set(CreationStatus::Pending),
        ..Default::default()
    };

    let collection = collection_active_model.insert(db.get()).await?;

    let solana_collections_active_model = solana_collections::ActiveModel {
        collection_id: Set(collection.id),
        master_edition_address: Set(master_edition_pubkey.to_string()),
        seller_fee_basis_points: Set(input.seller_fee_basis_points.try_into()?),
        created_by: Set(user_id),
        created_at: Set(Local::now().naive_utc()),
        ata_pubkey: Set(ata.to_string()),
        owner_pubkey: Set(owner.to_string()),
        update_authority: Set(owner.to_string()),
        mint_pubkey: Set(mint.to_string()),
        metadata_pubkey: Set(token_metadata_pubkey.to_string()),
        ..Default::default()
    };

    let solana_collection = solana_collections_active_model.insert(db.get()).await?;

    let drop_am = drops::ActiveModel {
        project_id: Set(input.project_id),
        collection_id: Set(collection.id),
        creation_status: Set(CreationStatus::Pending),
        start_time: Set(input.start_time.naive_utc()),
        end_time: Set(input.end_time.naive_utc()),
        price: Set(input.price.try_into()?),
        created_by: Set(user_id),
        created_at: Set(Local::now().naive_utc()),
        ..Default::default()
    };

    let drop = drop_am.insert(db.get()).await?;

    Ok((collection, solana_collection, drop))
}

/// This functions indexes the creators
/// # Errors
/// This function fails if insert fails
pub async fn index_creators(
    db: &Connection,
    creators: Vec<MetadataCreator>,
    collection: Uuid,
) -> Result<()> {
    for creator in creators {
        let am = collection_creators::ActiveModel {
            collection_id: Set(collection),
            address: Set(creator.address),
            verified: Set(creator.verified),
            share: Set(creator.share.try_into()?),
        };

        am.insert(db.get()).await?;
    }

    Ok(())
}

/// This functions indexes the metadata json uri data to `metadata_json`, attributes and files tables
/// # Errors
/// This function fails if insert fails
pub async fn index_metadata_json(
    db: &Connection,
    data: MetadataJson,
    collection: Uuid,
    cid: String,
) -> Result<()> {
    let metadata_json_active_model = metadata_jsons::ActiveModel {
        collection_id: Set(collection),
        identifier: Set(cid.clone()),
        name: Set(data.name.clone()),
        symbol: Set(data.symbol.clone()),
        description: Set(data.description.clone()),
        image: Set(data.image.clone()),
        animation_url: Set(data.animation_url.clone()),
        external_url: Set(data.external_url.clone()),
    };

    metadata_json_active_model.insert(db.get()).await?;

    for attribute in data.attributes {
        let am = collection_attributes::ActiveModel {
            collection_id: Set(collection),
            trait_type: Set(attribute.trait_type),
            value: Set(attribute.value),
            ..Default::default()
        };

        am.insert(db.get()).await?;
    }

    if let Some(files) = data.properties.files {
        for file in files {
            let metadata_json_file_am = metadata_json_files::ActiveModel {
                collection_id: Set(collection),
                uri: Set(file.uri),
                file_type: Set(file.file_type),
                ..Default::default()
            };

            metadata_json_file_am.insert(db.get()).await?;
        }
    }

    Ok(())
}

/// uploads the metadata json to nft.storage
/// # Errors
/// if the upload fails
pub async fn upload_metadata_json(
    client: &NftStorageClient,
    data: MetadataJson,
) -> Result<(String, String)> {
    let response = client.upload(data.clone()).await?;
    let cid = response.value.cid;

    let uri = client.ipfs_endpoint.join(&cid)?.to_string();

    Ok((uri, cid))
}

#[derive(Debug, Clone, Serialize, Deserialize, InputObject)]
pub struct CreateDropInput {
    royalty_address: String,
    project_id: Uuid,
    price: u64,
    update_authority_is_signer: bool,
    is_mutable: bool,
    metadata_json: MetadataJson,
    creators: Option<Vec<MetadataCreator>>,
    pub seller_fee_basis_points: u16,
    supply: Option<u64>,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    blockchain: Blockchain,
}

#[derive(Clone, Debug, Serialize, Deserialize, InputObject)]
pub struct MetadataJson {
    pub name: String,
    pub symbol: String,
    pub description: String,
    pub image: String,
    pub animation_url: Option<String>,
    pub collection: Option<MetadataJsonCollection>,
    pub attributes: Vec<Attribute>,
    pub external_url: Option<String>,
    pub properties: Property,
}

#[derive(Clone, Debug, Serialize, Deserialize, InputObject)]
pub struct File {
    uri: Option<String>,
    #[serde(rename = "type")]
    file_type: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, InputObject)]
pub struct Property {
    files: Option<Vec<File>>,
    category: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, InputObject)]
pub struct Attribute {
    trait_type: String,
    value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, InputObject)]
pub struct MetadataJsonCollection {
    name: Option<String>,
    family: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, InputObject)]
pub struct MetadataCreator {
    pub address: String,
    pub verified: bool,
    pub share: u8,
}

impl TryFrom<MetadataCreator> for Creator {
    type Error = Error;

    fn try_from(
        MetadataCreator {
            address,
            verified,
            share,
        }: MetadataCreator,
    ) -> Result<Self> {
        Ok(Self {
            address: address.parse()?,
            verified,
            share,
        })
    }
}
