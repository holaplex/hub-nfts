use std::{str::FromStr, sync::Arc};

use async_graphql::{self, Context, Error, InputObject, Object, Result, SimpleObject};
use hub_core::producer::Producer;
use mpl_token_metadata::{
    instruction::mint_new_edition_from_master_edition_via_token,
    state::{EDITION, PREFIX},
};
use sea_orm::{prelude::*, JoinType, QuerySelect, Set};
use solana_client::rpc_client::RpcClient;
use solana_program::{program_pack::Pack, pubkey::Pubkey, system_instruction::create_account};
use solana_sdk::signer::{keypair::Keypair, Signer};
use spl_associated_token_account::{
    get_associated_token_address, instruction::create_associated_token_account,
};
use spl_token::{
    instruction::{initialize_mint, mint_to},
    state::Mint,
};

use crate::{
    entities::{
        collection_mints, drops,
        prelude::{Collections, Drops, SolanaCollections},
        sea_orm_active_enums::CreationStatus,
        solana_collections,
    },
    proto::{drop_events, DropEventKey, DropEvents, Transaction},
    AppContext, UserID,
};

const TOKEN_PROGRAM_PUBKEY: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

#[derive(Default)]
pub struct Mutation;

#[Object(name = "MintMutation")]
impl Mutation {
    /// Res
    ///
    /// # Errors
    /// This function fails if ...
    pub async fn mint_edition(
        &self,
        ctx: &Context<'_>,
        input: MintDropInput,
    ) -> Result<MintEditionPayload> {
        let AppContext { db, user_id, .. } = ctx.data::<AppContext>()?;
        let rpc = &**ctx.data::<Arc<RpcClient>>()?;
        let producer = ctx.data::<Producer<DropEvents>>()?;
        let keypair_bytes = ctx.data::<Vec<u8>>()?;

        let UserID(id) = user_id;
        let user_id = id.ok_or_else(|| Error::new("X-USER-ID header not found"))?;

        let payer = Keypair::from_bytes(keypair_bytes)?;
        let owner = input.owner_address.parse()?;

        let drop_model = Drops::find()
            .select_also(Collections)
            .join(JoinType::InnerJoin, drops::Relation::Collections.def())
            .filter(drops::Column::Id.eq(input.drop))
            .one(db.get())
            .await?;

        let (drop_model, collection_model) =
            drop_model.ok_or_else(|| Error::new("Drop not found in db"))?;

        let collection =
            collection_model.ok_or_else(|| Error::new("Collection not found in db"))?;

        let solana_collection_model = SolanaCollections::find()
            .filter(solana_collections::Column::CollectionId.eq(collection.id))
            .one(db.get())
            .await?;

        let sc =
            solana_collection_model.ok_or_else(|| Error::new("Solana Collection not found"))?;

        let program_pubkey = mpl_token_metadata::id();
        let master_edition_pubkey: Pubkey = sc.master_edition_address.parse()?;
        let master_edition_mint: Pubkey = sc.mint_pubkey.parse()?;
        let existing_token_account: Pubkey = sc.ata_pubkey.parse()?;

        let token_key = Pubkey::from_str(TOKEN_PROGRAM_PUBKEY)?;

        let new_mint_key = Keypair::new();
        let added_token_account = get_associated_token_address(&owner, &new_mint_key.pubkey());
        let new_mint_pub = new_mint_key.pubkey();
        let edition_seeds = &[
            PREFIX.as_bytes(),
            program_pubkey.as_ref(),
            new_mint_pub.as_ref(),
            EDITION.as_bytes(),
        ];
        let (edition_key, _) = Pubkey::find_program_address(edition_seeds, &program_pubkey);

        let metadata_seeds = &[
            PREFIX.as_bytes(),
            program_pubkey.as_ref(),
            new_mint_pub.as_ref(),
        ];
        let (metadata_key, _) = Pubkey::find_program_address(metadata_seeds, &program_pubkey);

        let mut instructions = vec![
            create_account(
                &owner,
                &new_mint_key.pubkey(),
                rpc.get_minimum_balance_for_rent_exemption(Mint::LEN)?,
                Mint::LEN as u64,
                &token_key,
            ),
            initialize_mint(&token_key, &new_mint_key.pubkey(), &owner, Some(&owner), 0)?,
            create_associated_token_account(
                &payer.pubkey(),
                &owner,
                &new_mint_key.pubkey(),
                &spl_token::ID,
            ),
            mint_to(
                &token_key,
                &new_mint_key.pubkey(),
                &added_token_account,
                &owner,
                &[&owner],
                1,
            )?,
        ];

        instructions.push(mint_new_edition_from_master_edition_via_token(
            program_pubkey,
            metadata_key,
            edition_key,
            master_edition_pubkey,
            new_mint_key.pubkey(),
            owner,
            payer.pubkey(),
            owner,
            existing_token_account,
            sc.update_authority.parse()?,
            sc.metadata_pubkey.parse()?,
            master_edition_mint,
            input.edition,
        ));

        let destination_token_account =
            get_associated_token_address(&input.destination.parse()?, &new_mint_key.pubkey());

        instructions.push(create_associated_token_account(
            &payer.pubkey(),
            &input.destination.parse()?,
            &new_mint_key.pubkey(),
            &spl_token::ID,
        ));

        instructions.push(spl_token::instruction::transfer(
            &spl_token::id(),
            &added_token_account,
            &destination_token_account,
            &owner,
            &[&owner],
            1,
        )?);

        let blockhash = rpc.get_latest_blockhash()?;

        let message = solana_program::message::Message::new_with_blockhash(
            &instructions,
            Some(&payer.pubkey()),
            &blockhash,
        );

        let serialized_message = message.serialize();
        let mint_signature = new_mint_key.try_sign_message(&message.serialize())?;
        let payer_signature = payer.try_sign_message(&message.serialize())?;

        let collection_mint_active_model = collection_mints::ActiveModel {
            collection_id: Set(collection.id),
            address: Set(edition_key.to_string()),
            owner: Set(owner.to_string()),
            creation_status: Set(CreationStatus::Pending),
            created_by: Set(user_id),
            ..Default::default()
        };

        let collection_mint_model = collection_mint_active_model.insert(db.get()).await?;

        let event = DropEvents {
            event: Some(drop_events::Event::MintEdition(Transaction {
                serialized_message,
                signed_message_signatures: vec![
                    payer_signature.to_string(),
                    mint_signature.to_string(),
                ],
                project_id: drop_model.project_id.to_string(),
            })),
        };
        let key = DropEventKey {
            id: collection_mint_model.id.to_string(),
            user_id: user_id.to_string(),
        };

        producer.send(Some(&event), Some(&key)).await?;

        Ok(MintEditionPayload {
            collection_mint: collection_mint_model,
        })
    }
}
#[derive(Debug, Clone, InputObject)]
pub struct MintDropInput {
    drop: Uuid,
    owner_address: String,
    destination: String,
    edition: u64,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct MintEditionPayload {
    collection_mint: collection_mints::Model,
}
