use std::{collections::HashMap, convert::TryInto, sync::Arc, vec};

use clap::{arg, command};
use dashmap::DashMap;
use futures::{sink::SinkExt, stream::StreamExt};
use hub_core::{chrono::Utc, clap, prelude::*, tokio::task};
use hub_nfts_api::{
    db::{self, Connection},
    entities::{collection_mints, nft_transfers, prelude::CollectionMints},
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use solana_client::rpc_client::RpcClient;
use solana_program::program_pack::Pack;
use solana_sdk::pubkey::Pubkey;
use spl_token::{instruction::TokenInstruction, state::Account};
use yellowstone_grpc_client::{GeyserGrpcClient, GeyserGrpcClientError};
use yellowstone_grpc_proto::{
    prelude::{
        subscribe_request_filter_accounts_filter::Filter,
        subscribe_request_filter_accounts_filter_memcmp::Data, subscribe_update::UpdateOneof, *,
    },
    tonic::Status,
};

#[derive(Debug, clap::Args)]
pub struct Args {
    #[arg(short, long, env)]
    pub endpoint: String,

    #[arg(short, long, env)]
    pub x_token: Option<String>,

    #[arg(short, long, env)]
    pub solana_endpoint: String,

    #[command(flatten)]
    pub db: db::DbArgs,
}

pub fn main() {
    let opts = hub_core::StartConfig {
        service_name: "hub-nfts-indexer",
    };

    hub_core::run(opts, |common, args| {
        let Args {
            endpoint,
            x_token,
            solana_endpoint,
            db,
        } = args;

        common
            .rt
            .block_on(run(endpoint, x_token, solana_endpoint, db))
    });
}

async fn run(
    endpoint: String,
    x_token: Option<String>,
    solana_endpoint: String,
    db: db::DbArgs,
) -> Result<()> {
    let connection = Connection::new(db)
        .await
        .context("failed to get database connection")?;
    let dashmap: DashMap<u64, Vec<SubscribeUpdateTransaction>> = DashMap::new();
    let mut client = GeyserGrpcClient::connect(endpoint, x_token, None)?;
    let rpc = Arc::new(RpcClient::new(solana_endpoint));

    let (mut subscribe_tx, mut stream) = client.subscribe().await?;
    let request = create_request();

    loop {
        let connection = connection.clone();
        let request = request.clone();
        let rpc = rpc.clone();

        subscribe_tx
            .send(request)
            .await
            .map_err(GeyserGrpcClientError::SubscribeSendError)?;

        while let Some(message) = stream.next().await {
            handle_message(message, dashmap.clone(), connection.clone(), rpc.clone()).await?;
        }
    }
}

async fn handle_message(
    message: Result<SubscribeUpdate, Status>,
    dashmap: DashMap<u64, Vec<SubscribeUpdateTransaction>>,
    connection: Connection,
    rpc: Arc<RpcClient>,
) -> Result<()> {
    match message {
        Ok(msg) => match msg.update_oneof {
            Some(UpdateOneof::Transaction(tx)) => {
                dashmap.entry(tx.slot).or_insert(Vec::new()).push(tx);
            },
            Some(UpdateOneof::Slot(slot)) => {
                if let Some((_, transactions)) = dashmap.remove(&slot.slot) {
                    for tx in transactions {
                        task::spawn({
                            let tx = tx.clone();
                            let connection = connection.clone();
                            let rpc = rpc.clone();
                            process_transaction(connection, rpc, tx)
                        });
                    }
                }
            },
            _ => {},
        },
        Err(error) => return Err(anyhow!("stream error: {:?}", error)),
    };

    Ok(())
}

fn create_request() -> SubscribeRequest {
    let mut slots = HashMap::new();
    slots.insert("client".to_owned(), SubscribeRequestFilterSlots {});

    let mut accounts = HashMap::new();
    accounts.insert("client".to_string(), SubscribeRequestFilterAccounts {
        account: Vec::new(),
        owner: vec![spl_token::ID.to_string()],
        filters: vec![SubscribeRequestFilterAccountsFilter {
            filter: Some(Filter::Memcmp(SubscribeRequestFilterAccountsFilterMemcmp {
                offset: 65,
                data: Some(Data::Bytes(vec![0, 0, 0, 0, 0, 0, 0, 1])),
            })),
        }],
    });

    let mut transactions = HashMap::new();
    transactions.insert("client".to_string(), SubscribeRequestFilterTransactions {
        vote: Some(false),
        failed: Some(false),
        signature: None,
        account_include: vec![spl_token::ID.to_string()],
        account_exclude: Vec::new(),
        account_required: Vec::new(),
    });

    SubscribeRequest {
        accounts,
        slots,
        transactions,
        blocks: HashMap::new(),
        blocks_meta: HashMap::new(),
        commitment: Some(CommitmentLevel::Finalized as i32),
    }
}

async fn process_transaction(
    connection: Connection,
    rpc: Arc<RpcClient>,
    tx: SubscribeUpdateTransaction,
) -> Result<()> {
    let message = tx
        .clone()
        .transaction
        .context("SubsribeTransactionInfo not found")?
        .transaction
        .context("Transaction not found")?
        .message
        .context("Message not found")?;

    let mut i = 0;
    let keys = message.clone().account_keys;

    for (idx, key) in message.clone().account_keys.iter().enumerate() {
        let k = Pubkey::try_from(key.clone()).map_err(|_| anyhow!("failed to parse pubkey"))?;
        if k == spl_token::ID {
            i = idx;
            break;
        }
    }

    for ins in message.instructions.iter() {
        let account_indices = ins.accounts.clone();
        let program_idx: usize = ins.program_id_index.try_into()?;

        if program_idx == i {
            let data = ins.data.clone();
            let data = data.as_slice();
            let tkn_instruction = spl_token::instruction::TokenInstruction::unpack(data)?;
            if let TokenInstruction::Transfer { amount } = tkn_instruction {
                if amount == 1 {
                    // let sig = tx
                    //     .transaction
                    //     .as_ref()
                    //     .ok_or_else(|| anyhow!("failed to get transaction"))?
                    //     .signature
                    //     .clone();
                    // let _signature: Signature = Signature::new(sig.as_slice());
                    let source_account_index = account_indices[0];
                    let source_bytes = &keys[source_account_index as usize];
                    let source = Pubkey::try_from(source_bytes.clone())
                        .map_err(|_| anyhow!("failed to parse pubkey"))?;
                    let destination_account_index = account_indices[1];
                    let destination_bytes = &keys[destination_account_index as usize];
                    let destination = Pubkey::try_from(destination_bytes.clone())
                        .map_err(|_| anyhow!("failed to parse pubkey"))?;

                    let collection_mint = CollectionMints::find()
                        .filter(collection_mints::Column::OwnerAta.eq(source.to_string()))
                        .one(connection.get())
                        .await?;

                    if let Some(collection_mint) = collection_mint {
                        let acct = rpc.get_account(&destination)?;
                        let destination_tkn_act = Account::unpack(&acct.data)?;

                        let nft_transfer_am = nft_transfers::ActiveModel {
                            tx_signature: Set(None),
                            mint_address: Set(destination_tkn_act.mint.to_string()),
                            sender: Set(collection_mint.owner.to_string()),
                            recipient: Set(destination_tkn_act.owner.to_string()),
                            created_at: Set(Utc::now().into()),
                            credits_deduction_id: Set(None),
                            ..Default::default()
                        };

                        nft_transfer_am.insert(connection.get()).await?;
                    }
                }
            }
        }
    }

    Ok(())
}
