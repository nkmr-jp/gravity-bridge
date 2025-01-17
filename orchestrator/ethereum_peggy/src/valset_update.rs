use crate::utils::{get_valset_nonce, GasCost};
use clarity::PrivateKey as EthPrivateKey;
use clarity::{Address as EthAddress, Uint256};
use peggy_utils::types::*;
use peggy_utils::{error::PeggyError, message_signatures::encode_valset_confirm_hashed};
use std::{cmp::min, time::Duration};
use web30::{client::Web3, types::TransactionRequest};
use json_logger::LOGGING;
use slog::{info as sinfo};
use slog::{error as serror};

/// this function generates an appropriate Ethereum transaction
/// to submit the provided validator set and signatures.
#[allow(clippy::too_many_arguments)]
pub async fn send_eth_valset_update(
    new_valset: Valset,
    old_valset: Valset,
    confirms: &[ValsetConfirmResponse],
    web3: &Web3,
    timeout: Duration,
    peggy_contract_address: EthAddress,
    peggy_id: String,
    our_eth_key: EthPrivateKey,
) -> Result<(), PeggyError> {
    let old_nonce = old_valset.nonce;
    let new_nonce = new_valset.nonce;
    assert!(new_nonce > old_nonce);
    let eth_address = our_eth_key.to_public_key().unwrap();
    info!(
        "Ordering signatures and submitting validator set {} -> {} update to Ethereum",
        old_nonce.clone(), new_nonce.clone()
    );
    sinfo!(&LOGGING.logger, "ORDERING_SIGNATURES_AND_SUBMITTING_VALIDATOR";
        "function" => "send_eth_valset_update()",
        "old_nonce" => format!("{}",old_nonce),
        "new_nonce" => format!("{}",new_nonce),
    );

    let before_nonce = get_valset_nonce(peggy_contract_address, eth_address, web3).await?;
    if before_nonce != old_nonce {
        info!(
            "Someone else updated the valset to {}, exiting early",
            before_nonce.clone()
        );
        sinfo!(&LOGGING.logger, "SOMEONE_ELSE_UPDATED_THE_VALSET";
            "function" => "send_eth_valset_update()",
            "before_nonce" => format!("{}",before_nonce),
        );
        return Ok(());
    }

    let payload = encode_valset_payload(new_valset, old_valset, confirms, peggy_id)?;

    let tx = web3
        .send_transaction(
            peggy_contract_address,
            payload,
            0u32.into(),
            eth_address,
            our_eth_key,
            vec![],
        )
        .await?;
    info!("Sent valset update with txid {:#066x}", tx);
    sinfo!(&LOGGING.logger, "SENT_VALSET_UPDATE_WITH_TXI";
        "function" => "send_eth_valset_update()",
        "tx" => format!("{:#066x}",tx),
    );

    web3.wait_for_transaction(tx, timeout, None).await?;

    let last_nonce = get_valset_nonce(peggy_contract_address, eth_address, web3).await?;
    if last_nonce != new_nonce {
        error!(
            "Current nonce is {} expected to update to nonce {}",
            last_nonce.clone(), new_nonce.clone()
        );
        serror!(&LOGGING.logger, "CURRENT_NONCE_IS_FAILED";
            "function" => "send_eth_valset_update()",
            "last_nonce" => format!("{}",last_nonce),
            "new_nonce" => format!("{}",new_nonce),
        );
    } else {
        info!(
            "Successfully updated Valset with new Nonce {:?}",
            last_nonce.clone()
        );
        sinfo!(&LOGGING.logger, "SUCCESSFULLY_UPDATED_VALSET_WITH_NEW_NONCE";
            "function" => "send_eth_valset_update()",
            "last_nonce" => format!("{:?}",last_nonce),
        );
    }
    Ok(())
}

/// Returns the cost in Eth of sending this valset update
pub async fn estimate_valset_cost(
    new_valset: &Valset,
    old_valset: &Valset,
    confirms: &[ValsetConfirmResponse],
    web3: &Web3,
    peggy_contract_address: EthAddress,
    peggy_id: String,
    our_eth_key: EthPrivateKey,
) -> Result<GasCost, PeggyError> {
    let our_eth_address = our_eth_key.to_public_key().unwrap();
    let our_balance = web3.eth_get_balance(our_eth_address).await?;
    let our_nonce = web3.eth_get_transaction_count(our_eth_address).await?;
    let gas_limit = min((u64::MAX - 1).into(), our_balance.clone());
    let gas_price = web3.eth_gas_price().await?;
    let zero: Uint256 = 0u8.into();
    let val = web3
        .eth_estimate_gas(TransactionRequest {
            from: Some(our_eth_address),
            to: peggy_contract_address,
            nonce: Some(our_nonce.clone().into()),
            gas_price: Some(gas_price.clone().into()),
            gas: Some(gas_limit.into()),
            value: Some(zero.into()),
            data: Some(
                encode_valset_payload(new_valset.clone(), old_valset.clone(), confirms, peggy_id)?
                    .into(),
            ),
        })
        .await?;

    Ok(GasCost {
        gas: val,
        gas_price,
    })
}

/// Encodes the payload bytes for the validator set update call, useful for
/// estimating the cost of submitting a validator set
fn encode_valset_payload(
    new_valset: Valset,
    old_valset: Valset,
    confirms: &[ValsetConfirmResponse],
    peggy_id: String,
) -> Result<Vec<u8>, PeggyError> {
    let (old_addresses, old_powers) = old_valset.filter_empty_addresses();
    let (new_addresses, new_powers) = new_valset.filter_empty_addresses();
    let old_nonce = old_valset.nonce;
    let new_nonce = new_valset.nonce;

    // remember the signatures are over the new valset and therefore this is the value we must encode
    // the old valset exists only as a hash in the ethereum store
    let hash = encode_valset_confirm_hashed(peggy_id, new_valset);
    // we need to use the old valset here because our signatures need to match the current
    // members of the validator set in the contract.
    let sig_data = old_valset.order_sigs(&hash, confirms)?;
    let sig_arrays = to_arrays(sig_data);

    // Solidity function signature
    // function updateValset(
    // // The new version of the validator set
    // address[] memory _newValidators,
    // uint256[] memory _newPowers,
    // uint256 _newValsetNonce,
    // // The current validators that approve the change
    // address[] memory _currentValidators,
    // uint256[] memory _currentPowers,
    // uint256 _currentValsetNonce,
    // // These are arrays of the parts of the current validator's signatures
    // uint8[] memory _v,
    // bytes32[] memory _r,
    // bytes32[] memory _s
    let tokens = &[
        new_addresses.into(),
        new_powers.into(),
        new_nonce.into(),
        old_addresses.into(),
        old_powers.into(),
        old_nonce.into(),
        sig_arrays.v,
        sig_arrays.r,
        sig_arrays.s,
    ];
    let payload = clarity::abi::encode_call("updateValset(address[],uint256[],uint256,address[],uint256[],uint256,uint8[],bytes32[],bytes32[])",
    tokens).unwrap();

    Ok(payload)
}
