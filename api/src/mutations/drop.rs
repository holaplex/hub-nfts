use std::sync::Arc;

use async_graphql::{self, Context, Error, InputObject, Object, Result};
use chrono::{DateTime, Local, Utc};
use hub_core::producer::Producer;
use mpl_token_metadata::state::Creator;
use sea_orm::{prelude::*, Set};
use serde::{Deserialize, Serialize};
use solana_client::rpc_client::RpcClient;
use solana_program::program_pack::Pack;
use solana_sdk::signer::{keypair::Keypair, Signer};
use spl_associated_token_account::get_associated_token_address;

use crate::{
    entities::{
        collections, drops,
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

        let uri = upload_metadata_json(nft_storage, input.metadata_json.clone()).await?;

        let payer = Keypair::from_bytes(keypair_bytes)?;
        let mint = Keypair::new();

        let owner = input.owner_address.parse()?;
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

        let creators = input.creators.as_ref().map(|creators| {
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
                input.seller_fee_basis_points.clone(),
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
            mint_pubkey: Set(mint.pubkey().to_string()),
            metadata_pubkey: Set(token_metadata_pubkey.to_string()),
            ..Default::default()
        };

        solana_collections_active_model.insert(db.get()).await?;

        let drop = drops::ActiveModel {
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

        let drop_model = drop.insert(db.get()).await?;

        let event = NftEvents {
            event: Some(nft_events::Event::CreateDrop(proto::DropTransaction {
                transaction: Some(proto::Transaction {
                    serialized_message,
                    signed_message_signatures: vec![
                        payer_signature.to_string(),
                        mint_signature.to_string(),
                    ],
                }),
                project_id: input.project_id.to_string(),
            })),
        };
        let key = NftEventKey {
            id: drop_model.id.to_string(),
            user_id: user_id.to_string(),
        };

        producer.send(Some(&event), Some(&key)).await?;

        Ok(drop_model)
    }
}

pub async fn upload_metadata_json(client: &NftStorageClient, data: MetadataJson) -> Result<String> {
    let response = client.upload(data).await?;
    let cid = response.value.cid;

    Ok(client.ipfs_endpoint.join(&cid)?.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize, InputObject)]
pub struct CreateDropInput {
    royalty_address: String,
    owner_address: String,
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
    pub collection: Option<Collection>,
    pub attributes: Vec<Attribute>,
    pub external_url: Option<String>,
    pub properties: Property,
}

#[derive(Clone, Debug, Serialize, Deserialize, InputObject)]
pub struct File {
    uri: Option<String>,
    r#type: Option<String>,
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
pub struct Collection {
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
