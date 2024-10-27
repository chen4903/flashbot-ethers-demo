use ethers::prelude::*;
use ethers_flashbots::*;
use std::convert::TryFrom;
use url::Url;
use std::str::FromStr;
use ethers::core::types::transaction::eip2718::TypedTransaction;
use anyhow::Result;
use std::env;
use dotenv::dotenv;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let private_key = env::var("PRIVATE_KEY").expect("error private key");
    let rpc_url = env::var("RPC_URL").expect("error private key");

    let provider = Provider::<Http>::try_from(rpc_url)?;

    let bundle_signer = LocalWallet::from_str(&private_key)?;
    let wallet = LocalWallet::from_str(&private_key)?;

    // Set chainId for Sepolia testnet
    // `cast chain-id --rpc-url {your rpc}`
    let wallet = wallet.with_chain_id(11155111u64);
    let bundle_signer = bundle_signer.with_chain_id(11155111u64);

    // Add signer and Flashbots middleware
    let client = SignerMiddleware::new(
        FlashbotsMiddleware::new(
            provider,
            Url::parse("https://relay-sepolia.flashbots.net")?,
            bundle_signer,
        ),
        wallet,
    );

    let bundle = get_bundle_for_test(&client).await?;
    let current_block_number = client.inner().inner().get_block_number().await?;
    let bundle: BundleRequest = bundle
        .set_simulation_block(current_block_number)
        .set_simulation_timestamp(1731851886) //  2024-11-17 21:58:06
        .set_block(current_block_number + 1);
    let raw_txs: Vec<Bytes> = bundle.transactions()
        .iter()
        .map(|tx| match tx {
            BundleTransaction::Signed(inner) => inner.rlp(),
            BundleTransaction::Raw(inner) => inner.clone(),
        })
        .collect();

    println!("Simulated bundle: {:?}", raw_txs);

    // Submitting multiple bundles to increase the probability on inclusion, plz be patient :)
    for x in 0..100 {
        let bundle = get_bundle_for_test(&client).await?;
        let bundle = bundle.set_block(current_block_number + x);
        println!("Bundle Initialized");
        println!("Current block height: {}", current_block_number + x);
        let pending_bundle = client.inner().send_bundle(&bundle).await?;

        let transactions = pending_bundle.transactions.clone();
        
        match pending_bundle.await {
            Ok(bundle_hash) => {
                println!("🤖 Bundle with hash {:?} was included in target block",bundle_hash);
                println!("🎉 Transaction hashes: {:?}", transactions);
                break;
            },
            Err(PendingBundleError::BundleNotIncluded) => {
                println!("Bundle was not included in target block.")
            }
            Err(e) => println!("An error occurred: {}", e),
        }
    }

    Ok(())
}

async fn get_bundle_for_test<M: 'static + Middleware, S: 'static + Signer>(client: &SignerMiddleware<M, S>) -> Result<BundleRequest> {
    let mut nonce = client.get_transaction_count(client.address(), None).await?;

    // Send me a coffee in test network :)
    let mut tx: TypedTransaction = TransactionRequest::pay("0x510CB00000074c9f5063e81bd4647d00905Abd11", 100).into();
    let mut bundle = BundleRequest::new();

    // Create a bundle with multiple transactions
    for _ in 0..2 {
        tx.set_nonce(nonce);
        client.fill_transaction(&mut tx, None).await?;
        nonce += U256::from(1);
        let signature = client.signer().sign_transaction(&tx).await?;
        // Use rlp_signed with only the signature
        bundle = bundle.push_transaction(tx.rlp_signed(&signature));
    }

    Ok(bundle)
}