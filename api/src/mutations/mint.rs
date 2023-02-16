use std::{str::FromStr, sync::Arc};

use async_graphql::{self, Context, Error, InputObject, Object, Result};
use chrono::{DateTime, Local, Utc};
use hub_core::producer::{self, Producer};
use mpl_token_metadata::{
    instruction::mint_new_edition_from_master_edition_via_token,
    state::{Creator, EDITION, PREFIX},
};
use sea_orm::{prelude::*, JoinType, QuerySelect};
use solana_client::rpc_client::RpcClient;
use solana_program::{
    message, program_pack::Pack, pubkey::Pubkey, system_instruction::create_account,
};
use solana_sdk::{
    signer::{keypair::Keypair, Signer},
    transaction::Transaction,
};
use spl_associated_token_account::{
    get_associated_token_address, instruction::create_associated_token_account,
};
use spl_token::{
    instruction::{initialize_account, initialize_mint, mint_to},
    state::{Account, Mint},
};

use crate::{
    entities::{
        collections, drops,
        prelude::{Collections, Drops, SolanaCollections},
        sea_orm_active_enums::{Blockchain, CreationStatus},
        solana_collections,
    },
    proto::{
        self,
        drop_events::{self},
        DropEventKey,
    },
    AppContext, DropEvents, UserID,
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
    pub async fn mint_edition(&self, ctx: &Context<'_>, input: MintDropInput) -> Result<String> {
        let AppContext { db, user_id, .. } = ctx.data::<AppContext>()?;
        let rpc = &**ctx.data::<Arc<RpcClient>>()?;
        let producer = ctx.data::<Producer<DropEvents>>()?;

        let UserID(id) = user_id;
        let user_id = id.unwrap();

        let drop_model = Drops::find()
            .select_also(Collections)
            .join(JoinType::InnerJoin, drops::Relation::Collections.def())
            .filter(drops::Column::Id.eq(input.drop))
            .one(db.get())
            .await?;

        let (drop, collection_model) =
            drop_model.ok_or_else(|| Error::new("Drop not found in db"))?;
        let collection =
            collection_model.ok_or_else(|| Error::new("Collection not found in db"))?;

        let solana_collection_model = SolanaCollections::find()
            .filter(solana_collections::Column::Id.eq(collection.collection))
            .one(db.get())
            .await?;

        let sc =
            solana_collection_model.ok_or_else(|| Error::new("Solana Collection not found"))?;

        let wallet = input.owner_address.parse()?;



        let program_pubkey = mpl_token_metadata::id();
        let master_edition_pubkey: Pubkey = sc.address.parse()?;
        let master_edition_mint: Pubkey = sc.mint_pubkey.parse()?;
        let existing_token_account: Pubkey = sc.ata_pubkey.parse()?;

       
        let token_key = Pubkey::from_str(TOKEN_PROGRAM_PUBKEY).unwrap();

        let new_mint_key = Keypair::new();
        let added_token_account = get_associated_token_address(&wallet, &new_mint_key.pubkey());
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
                &wallet,
                &new_mint_key.pubkey(),
                rpc.get_minimum_balance_for_rent_exemption(Mint::LEN)
                    .unwrap(),
                Mint::LEN as u64,
                &token_key,
            ),
            initialize_mint(
                &token_key,
                &new_mint_key.pubkey(),
                &wallet,
                Some(&wallet),
                0,
            )
            .unwrap(),
            create_associated_token_account(
                &wallet,
                &wallet,
                &new_mint_key.pubkey(),
                &spl_token::ID,
            ),
            mint_to(
                &token_key,
                &new_mint_key.pubkey(),
                &added_token_account,
                &wallet,
                &[&wallet],
                1,
            )
            .unwrap(),
        ];

        instructions.push(mint_new_edition_from_master_edition_via_token(
            program_pubkey,
            metadata_key,
            edition_key,
            master_edition_pubkey,
            new_mint_key.pubkey(),
            wallet,
            wallet,
            wallet,
            existing_token_account,
            wallet,
            sc.metadata_pubkey.parse()?,
            master_edition_mint,
            1,
        ));

        let recent_blockhash = rpc.get_latest_blockhash().unwrap();

        let message = solana_program::message::Message::new_with_blockhash(
            &instructions,
            Some(&wallet),
            &recent_blockhash,
        );

        let signature = new_mint_key.try_sign_message(&message.serialize()).unwrap();

        let drop = Drops::find_by_id(input.drop)
            .one(db.get())
            .await?
            .ok_or_else(|| Error::new("failed to load drop from db"))?;

        let event = DropEvents {
            event: Some(drop_events::Event::MintEdition(
                proto::Transaction {
                    serialized_message: message.serialize(),
                    signed_message_signature: signature.to_string(),
                    project_id: drop.project_id.to_string(),
                },
            )),
        };

        let key = DropEventKey {
            id: signature.to_string(),
            user_id: user_id.to_string(),
        };

        producer.send(Some(&event), Some(&key)).await?;

        Ok("sent to treasury service for signing".to_string())
    }
}
#[derive(Debug, Clone, InputObject)]
pub struct MintDropInput {
    drop: Uuid,
    owner_address: String,
    destination: String,
}
