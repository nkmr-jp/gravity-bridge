use clarity::address::Address as EthAddress;
use clarity::PrivateKey as EthPrivateKey;
use cosmos_peggy::query::get_latest_transaction_batches;
use cosmos_peggy::query::get_transaction_batch_signatures;
use ethereum_peggy::utils::{downcast_to_u128, get_tx_batch_nonce};
use ethereum_peggy::{one_eth, submit_batch::send_eth_transaction_batch};
use peggy_proto::peggy::query_client::QueryClient as PeggyQueryClient;
use peggy_utils::message_signatures::encode_tx_batch_confirm_hashed;
use peggy_utils::types::Valset;
use peggy_utils::types::{BatchConfirmResponse, TransactionBatch};
use std::time::Duration;
use tonic::transport::Channel;
use web30::client::Web3;
use json_logger::LOGGING;
use slog::{info as sinfo};
use slog::{warn as swarn};
use slog::{error as serror};

pub async fn relay_batches(
    // the validator set currently in the contract on Ethereum
    current_valset: Valset,
    ethereum_key: EthPrivateKey,
    web3: &Web3,
    grpc_client: &mut PeggyQueryClient<Channel>,
    peggy_contract_address: EthAddress,
    peggy_id: String,
    timeout: Duration,
) {
    let our_ethereum_address = ethereum_key.to_public_key().unwrap();

    let latest_batches = get_latest_transaction_batches(grpc_client).await;
    trace!("Latest batches {:?}", latest_batches);
    if latest_batches.is_err() {
        return;
    }
    let latest_batches = latest_batches.unwrap();
    let mut oldest_signed_batch: Option<TransactionBatch> = None;
    let mut oldest_signatures: Option<Vec<BatchConfirmResponse>> = None;
    for batch in latest_batches {
        let sigs =
            get_transaction_batch_signatures(grpc_client, batch.nonce, batch.token_contract).await;
        trace!("Got sigs {:?}", sigs);
        if let Ok(sigs) = sigs {
            // this checks that the signatures for the batch are actually possible to submit to the chain
            let hash = encode_tx_batch_confirm_hashed(peggy_id.clone(), batch.clone());
            if current_valset.order_sigs(&hash, &sigs).is_ok() {
                oldest_signed_batch = Some(batch);
                oldest_signatures = Some(sigs);
            } else {
                warn!(
                    "Batch {}/{} can not be submitted yet, waiting for more signatures",
                    batch.token_contract, batch.nonce
                );
                swarn!(&LOGGING.logger, "BATCH_CAN_NOT_BE_SUBMITTED_YET";
                    "function" => "relay_batches()",
                    "token_contract" => format!("{}",batch.token_contract),
                    "nonce" => format!("{}",batch.nonce),
                );
            }
        } else {
            error!(
                "could not get signatures for {}:{} with {:?}",
                batch.token_contract, batch.nonce, sigs
            );
        }
    }
    if oldest_signed_batch.is_none() {
        trace!("Could not find batch with signatures! exiting");
        return;
    }
    let oldest_signed_batch = oldest_signed_batch.unwrap();
    let oldest_signatures = oldest_signatures.unwrap();
    let erc20_contract = oldest_signed_batch.token_contract;

    let latest_ethereum_batch = get_tx_batch_nonce(
        peggy_contract_address,
        erc20_contract,
        our_ethereum_address,
        web3,
    )
    .await;
    if latest_ethereum_batch.is_err() {
        error!(
            "Failed to get latest Ethereum batch with {:?}",
            latest_ethereum_batch
        );
        return;
    }
    let latest_ethereum_batch = latest_ethereum_batch.unwrap();
    let latest_cosmos_batch_nonce = oldest_signed_batch.clone().nonce;
    if latest_cosmos_batch_nonce > latest_ethereum_batch {
        let cost = ethereum_peggy::submit_batch::estimate_tx_batch_cost(
            current_valset.clone(),
            oldest_signed_batch.clone(),
            &oldest_signatures,
            web3,
            peggy_contract_address,
            peggy_id.clone(),
            ethereum_key,
        )
        .await;
        if cost.is_err() {
            error!("Batch cost estimate failed with {:?}", cost);
            return;
        }
        let cost = cost.unwrap();
        info!(
                "We have detected latest batch {} but latest on Ethereum is {} This batch is estimated to cost {} Gas / {:.4} ETH to submit",
                latest_cosmos_batch_nonce,
                latest_ethereum_batch,
                cost.gas_price.clone(),
                downcast_to_u128(cost.get_total()).unwrap() as f32
                    / downcast_to_u128(one_eth()).unwrap() as f32
            );
        sinfo!(&LOGGING.logger, "WE_HAVE_DETECTED_LATEST_BATCH";
            "function" => "relay_batches()",
            "latest_cosmos_batch_nonce" => format!("{}",latest_cosmos_batch_nonce),
            "latest_ethereum_batch" => format!("{}",latest_ethereum_batch),
            "cost_gas_price" => format!("{}",cost.gas_price.clone()),
            "per_eth" => format!("{:.4}",downcast_to_u128(cost.get_total()).unwrap() as f32
                / downcast_to_u128(one_eth()).unwrap() as f32),
        );

        let res = send_eth_transaction_batch(
            current_valset,
            oldest_signed_batch,
            &oldest_signatures,
            web3,
            timeout,
            peggy_contract_address,
            peggy_id,
            ethereum_key,
        )
        .await;
        if res.is_err() {
            info!("Batch submission failed with {:?}", res);
            sinfo!(&LOGGING.logger, "BATCH_SUBMISSION_FAILED";
                "function" => "relay_batches()",
                "res" => format!("{:?}",res),
            );
        }
    }
}
