macro_rules! container {
    ($name: ident, [$($client:ident :: $client_type:ident,)+]) => {
        pub mod $name {
            use failure::Error;
            use std::result::Result;
            $(
                use crate::client::$client::$client_type;
            )+

            pub struct Client {
                $(
                    pub $client: $client_type,
                )+
            }

            impl Client {
                pub fn new() -> Result<Self, Error> {
                    $(
                        let $client = $client_type::new()?;
                    )+

                    Ok(Self {
                        $(
                            $client,
                        )+
                    })
                }
            }
        }
    };
}

/*
 * 1. Pool:
 *
 * Network
 * forword_unverified_transaction
 * send_unverified_transaction_hashes
 *
 * 2. Chain:
 *
 * Executor:
 * apply
 *
 * 3. Consensus:
 *
 * Chain:
 * get_proposal_config
 * add_block
 *
 * Network:
 * broadcast_consensus_message
 *
 * Sync:
 * update_status
 *
 * Pool:
 * proposal_unverified_transaction_hashes
 * check_unverified_transaction
 * confirm_unverified_transaction
 *
 * 4. Network:
 *
 * Pool:
 * add_unverified_transaction
 * get_unverified_transaction
 * add_batch_unverified_transactions
 *
 * Sync:
 * update_status
 * proc_sync_request
 * proc_sync_response
 *
 * 5. Sync:
 *
 * Network:
 * broadcast_new_status
 * send_sync_request
 * send_sync_response
 *
 * Chain:
 * get_block
 * add_glock
 *
 * Pool:
 * confirm_unverified_transaction
 *
 * Consensus:
 * set_status
 *
 * 6. RPC:
 *
 * Pool:
 * add_unverified_transaction
 *
 * Chain:
 * get_receipt
 */

container! {
    rpc, [
        chain::ChainClient,
        pool::PoolClient,
    ]
}

container! {
    pool, [
        network::NetworkClient,
    ]
}

container! {
    chain, [
        executor::ExecutorClient,
    ]
}

container! {
    consensus, [
        chain::ChainClient,
        network::NetworkClient,
        sync::SyncClient,
        pool::PoolClient,
    ]
}

container! {
    network, [
        pool::PoolClient,
        sync::SyncClient,
    ]
}

container! {
    sync, [
        chain::ChainClient,
        network::NetworkClient,
        pool::PoolClient,
        consensus::ConsensusClient,
    ]
}
