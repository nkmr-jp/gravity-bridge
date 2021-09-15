//! Ethereum Event watcher watches for events such as a deposit to the Peggy Ethereum contract or a validator set update
//! or a transaction batch update. It then responds to these events by performing actions on the Cosmos chain if required

use clarity::{utils::bytes_to_hex_str, Address as EthAddress, Uint256};
use contact::client::Contact;
use cosmos_peggy::{query::get_last_event_nonce, send::send_ethereum_claims};
use deep_space::{coin::Coin, private_key::PrivateKey as CosmosPrivateKey};
use peggy_proto::peggy::query_client::QueryClient as PeggyQueryClient;
use peggy_utils::{
    error::PeggyError,
    types::{
        ERC20DeployedEvent, LogicCallExecutedEvent, SendToCosmosEvent,
        TransactionBatchExecutedEvent, ValsetUpdatedEvent,
    },
};
use tonic::transport::Channel;
use web30::client::Web3;
use web30::jsonrpc::error::Web3Error;
use json_logger::LOGGING;
use slog::{info as sinfo};

use crate::get_with_retry::get_block_number_with_retry;
use crate::get_with_retry::get_net_version_with_retry;

pub async fn check_for_events(
    web3: &Web3,
    contact: &Contact,
    grpc_client: &mut PeggyQueryClient<Channel>,
    peggy_contract_address: EthAddress,
    our_private_key: CosmosPrivateKey,
    fee: Coin,
    starting_block: Uint256,
) -> Result<Uint256, PeggyError> {
    let our_cosmos_address = our_private_key.to_public_key().unwrap().to_address();
    let latest_block = get_block_number_with_retry(web3).await;
    let latest_block = latest_block - get_block_delay(web3).await;

    let deposits = web3
        .check_for_events(
            starting_block.clone(),
            Some(latest_block.clone()),
            vec![peggy_contract_address],
            vec!["SendToCosmosEvent(address,address,bytes32,uint256,uint256)"],
        )
        .await;
    trace!("Deposits {:?}", deposits);

    let batches = web3
        .check_for_events(
            starting_block.clone(),
            Some(latest_block.clone()),
            vec![peggy_contract_address],
            vec!["TransactionBatchExecutedEvent(uint256,address,uint256)"],
        )
        .await;
    trace!("Batches {:?}", batches);

    let valsets = web3
        .check_for_events(
            starting_block.clone(),
            Some(latest_block.clone()),
            vec![peggy_contract_address],
            vec!["ValsetUpdatedEvent(uint256,address[],uint256[])"],
        )
        .await;
    trace!("Valsets {:?}", valsets);

    let erc20_deployed = web3
        .check_for_events(
            starting_block.clone(),
            Some(latest_block.clone()),
            vec![peggy_contract_address],
            vec!["ERC20DeployedEvent(string,address,string,string,uint8,uint256)"],
        )
        .await;
    trace!("ERC20 Deployments {:?}", erc20_deployed);

    let logic_call_executed = web3
        .check_for_events(
            starting_block.clone(),
            Some(latest_block.clone()),
            vec![peggy_contract_address],
            vec!["LogicCallEvent(bytes32,uint256,bytes,uint256)"],
        )
        .await;
    trace!("Logic call executions {:?}", logic_call_executed);

    if let (Ok(valsets), Ok(batches), Ok(deposits), Ok(deploys), Ok(logic_calls)) = (
        valsets,
        batches,
        deposits,
        erc20_deployed,
        logic_call_executed,
    ) {
        let valsets = ValsetUpdatedEvent::from_logs(&valsets)?;
        trace!("parsed valsets {:?}", valsets);
        let withdraws = TransactionBatchExecutedEvent::from_logs(&batches)?;
        trace!("parsed batches {:?}", batches);
        let deposits = SendToCosmosEvent::from_logs(&deposits)?;
        trace!("parsed deposits {:?}", deposits);
        let erc20_deploys = ERC20DeployedEvent::from_logs(&deploys)?;
        trace!("parsed erc20 deploys {:?}", erc20_deploys);
        let logic_calls = LogicCallExecutedEvent::from_logs(&logic_calls)?;
        trace!("logic call executions {:?}", logic_calls);

        // note that starting block overlaps with our last checked block, because we have to deal with
        // the possibility that the relayer was killed after relaying only one of multiple events in a single
        // block, so we also need this routine so make sure we don't send in the first event in this hypothetical
        // multi event block again. In theory we only send all events for every block and that will pass of fail
        // atomicly but lets not take that risk.
        let last_event_nonce = get_last_event_nonce(grpc_client, our_cosmos_address).await?;
        let deposits = SendToCosmosEvent::filter_by_event_nonce(last_event_nonce, &deposits);
        let withdraws =
            TransactionBatchExecutedEvent::filter_by_event_nonce(last_event_nonce, &withdraws);
        let erc20_deploys =
            ERC20DeployedEvent::filter_by_event_nonce(last_event_nonce, &erc20_deploys);
        let logic_calls =
            LogicCallExecutedEvent::filter_by_event_nonce(last_event_nonce, &logic_calls);

        if !deposits.is_empty() {
            info!(
                "Oracle observed deposit with sender {}, destination {}, amount {}, and event nonce {}",
                deposits[0].sender, deposits[0].destination, deposits[0].amount, deposits[0].event_nonce
            );
            sinfo!(&LOGGING.logger, "ORACLE_OBSERVED_DEPOSIT";
                "function" => "check_for_events()",
                "last_nonce" => format!("{}",deposits[0].sender),
                "destination" => format!("{}",deposits[0].destination),
                "amount" => format!("{}",deposits[0].amount),
                "event_nonce" => format!("{}",deposits[0].event_nonce),
            );
        }
        if !withdraws.is_empty() {
            info!(
                "Oracle observed batch with nonce {}, contract {}, and event nonce {}",
                withdraws[0].batch_nonce, withdraws[0].erc20, withdraws[0].event_nonce
            );
            sinfo!(&LOGGING.logger, "ORACLE_OBSERVED_BATCH";
                "function" => "check_for_events()",
                "batch_nonce" => format!("{}",withdraws[0].batch_nonce),
                "erc20" => format!("{}",withdraws[0].erc20),
                "event_nonce" => format!("{}",withdraws[0].event_nonce),
            );
        }
        if !erc20_deploys.is_empty() {
            info!(
                "Oracle observed ERC20 deployment with denom {} erc20 name {} and symbol {} and event nonce {}",
                erc20_deploys[0].cosmos_denom, erc20_deploys[0].name, erc20_deploys[0].symbol, erc20_deploys[0].event_nonce,
            );
            sinfo!(&LOGGING.logger, "ORACLE_OBSERVED_ERC20_DEPLOYMENT";
                "function" => "check_for_events()",
                "cosmos_denom" => format!("{}",erc20_deploys[0].cosmos_denom),
                "name" => format!("{}",erc20_deploys[0].name),
                "symbol" => format!("{}",erc20_deploys[0].symbol),
                "event_nonce" => format!("{}",erc20_deploys[0].event_nonce),
            );
        }
        if !logic_calls.is_empty() {
            info!(
                "Oracle observed logic call execution with ID {} Nonce {} and event nonce {}",
                bytes_to_hex_str(&logic_calls[0].invalidation_id),
                logic_calls[0].invalidation_nonce,
                logic_calls[0].event_nonce
            );
            sinfo!(&LOGGING.logger, "ORACLE_OBSERVED_LOGIC_CALL_EXECUTION";
                "function" => "check_for_events()",
                "invalidation_id" => format!("{}",bytes_to_hex_str(&logic_calls[0].invalidation_id)),
                "invalidation_nonce" => format!("{}",logic_calls[0].invalidation_nonce),
                "event_nonce" => format!("{}",logic_calls[0].event_nonce),
            );
        }

        if !deposits.is_empty()
            || !withdraws.is_empty()
            || !erc20_deploys.is_empty()
            || !logic_calls.is_empty()
        {
            let res = send_ethereum_claims(
                contact,
                our_private_key,
                deposits,
                withdraws,
                erc20_deploys,
                logic_calls,
                fee,
            )
            .await?;
            trace!("Claims response {:?}", res);
            let new_event_nonce = get_last_event_nonce(grpc_client, our_cosmos_address).await?;
            // since we can't actually trust that the above txresponse is correct we have to check here
            // we may be able to trust the tx response post grpc
            if new_event_nonce == last_event_nonce {
                return Err(PeggyError::InvalidBridgeStateError(
                    format!("Claims did not process, trying to update but still on {}, trying again in a moment, check txhash {} for errors", last_event_nonce, res.txhash),
                ));
            } else {
                info!("Claims processed, new nonce {}", new_event_nonce);
                sinfo!(&LOGGING.logger, "CLAIMS_PROCESSED";
                    "function" => "check_for_events()",
                    "new_event_nonce" => format!("{}",new_event_nonce),
                );
            }
        }
        Ok(latest_block)
    } else {
        error!("Failed to get events");
        Err(PeggyError::EthereumRestError(Web3Error::BadResponse(
            "Failed to get logs!".to_string(),
        )))
    }
}

/// The number of blocks behind the 'latest block' on Ethereum our event checking should be.
/// Ethereum does not have finality and as such is subject to chain reorgs and temporary forks
/// if we check for events up to the very latest block we may process an event which did not
/// 'actually occur' in the longest POW chain.
///
/// Obviously we must chose some delay in order to prevent incorrect events from being claimed
///
/// For EVM chains with finality the correct value for this is zero. As there's no need
/// to concern ourselves with re-orgs or forking. This function checks the netID of the
/// provided Ethereum RPC and adjusts the block delay accordingly
///
/// The value used here for Ethereum is a balance between being reasonably fast and reasonably secure
/// As you can see on https://etherscan.io/blocks_forked uncles (one block deep reorgs)
/// occur once every few minutes. Two deep once or twice a day.
/// https://etherscan.io/chart/uncles
/// Let's make a conservative assumption of 1% chance of an uncle being a two block deep reorg
/// (actual is closer to 0.3%) and assume that continues as we increase the depth.
/// Given an uncle every 2.8 minutes, a 6 deep reorg would be 2.8 minutes * (100^4) or one
/// 6 deep reorg every 53,272 years.
///
pub async fn get_block_delay(web3: &Web3) -> Uint256 {
    let net_version = get_net_version_with_retry(web3).await;

    match net_version {
        // Mainline Ethereum, Ethereum classic, or the Ropsten, Mordor testnets
        // all POW Chains
        1 | 3 | 7 => 6u8.into(),
        // Rinkeby, Goerli, Dev, our own Peggy Ethereum testnet, and Kotti respectively
        // all non-pow chains
        4 | 5 | 2018 | 15 | 6 => 0u8.into(),
        // assume the safe option (POW) where we don't know
        _ => 6u8.into(),
    }
}
