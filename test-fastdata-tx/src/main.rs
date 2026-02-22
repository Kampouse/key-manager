use borsh::BorshSerialize;
use near_crypto::{InMemorySigner, SecretKey};
use near_primitives::transaction::{Action, FunctionCallAction, Transaction};
use near_primitives::types::AccountId;
use near_primitives::hash::CryptoHash;
use reqwest::Client;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::str::FromStr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    
    // Account info
    let account_id = "kampouse.near";
    let public_key_str = "ed25519:AHwMutaKKS5UbEgdyyCXtMswTnGL5mZqGrf7PHGYiXTz";
    let private_key = "ed25519:5W5fCgLmensuYR9eis2EVicZixyrAZWZnXNfHAjXVRkV79DEVcmw3vaEuyueT1wbv6SGpBxNhnZqVDLetCt7CtfE";
    
    let rpc_url = "https://rpc.mainnet.near.org";
    
    // Get access key info (nonce and block hash)
    println!("Fetching access key info...");
    let access_key_query = json!({
        "jsonrpc": "2.0",
        "id": "dontcare",
        "method": "query",
        "params": {
            "request_type": "view_access_key",
            "finality": "final",
            "account_id": account_id,
            "public_key": public_key_str
        }
    });
    
    let resp = client.post(rpc_url)
        .json(&access_key_query)
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;
    
    if let Some(error) = resp.get("error") {
        println!("Error: {:?}", error);
        return Err(error.to_string().into());
    }
    
    let nonce = resp["result"]["nonce"].as_u64().unwrap() + 1;
    println!("Nonce: {}", nonce);
    
    // Get recent block hash
    let block_query = json!({
        "jsonrpc": "2.0",
        "id": "dontcare",
        "method": "block",
        "params": {"finality": "final"}
    });
    
    let block_resp = client.post(rpc_url)
        .json(&block_query)
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;
    
    let recent_block_hash = block_resp["result"]["header"]["hash"].as_str().unwrap();
    println!("Recent block: {}", recent_block_hash);
    
    // Create test data - JSON key-value pairs
    let test_data = json!({
        "test/name": "Gork Test",
        "test/timestamp": "2026-02-22",
        "test/message": "Hello from indexer test!"
    });
    let args = serde_json::to_vec(&test_data)?;
    
    println!("\nTest data: {}", String::from_utf8_lossy(&args));
    
    // Create signer
    let signer = InMemorySigner::from_secret_key(
        AccountId::from_str(account_id)?,
        SecretKey::from_str(private_key)?
    );
    
    // Create transaction
    let tx = Transaction {
        signer_id: AccountId::from_str(account_id)?,
        public_key: signer.public_key.clone(),
        nonce,
        receiver_id: AccountId::from_str(account_id)?, // Call self
        block_hash: CryptoHash::from_str(recent_block_hash)?,
        actions: vec![
            Action::FunctionCall(FunctionCallAction {
                method_name: "__fastdata_kv".to_string(),
                args,
                gas: 100_000_000_000_000,
                deposit: 0,
            })
        ],
    };
    
    // Sign transaction
    let serialized_tx = borsh::to_vec(&tx)?;
    let hash = Sha256::digest(&serialized_tx);
    let signature = signer.sign(&hash);
    
    // Create signed transaction
    let signed_tx = near_primitives::transaction::SignedTransaction::new(
        signature,
        tx
    );
    
    // Serialize and encode
    let signed_tx_borsh = borsh::to_vec(&signed_tx)?;
    let tx_base64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &signed_tx_borsh);
    
    // Broadcast transaction
    println!("\n📡 Broadcasting transaction...");
    let broadcast = json!({
        "jsonrpc": "2.0",
        "id": "dontcare",
        "method": "broadcast_tx_commit",
        "params": [tx_base64]
    });
    
    let result = client.post(rpc_url)
        .json(&broadcast)
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;
    
    if let Some(error) = result.get("error") {
        println!("❌ Error: {:?}", error);
        return Err(error.to_string().into());
    }
    
    if let Some(tx_hash) = result["result"]["transaction"]["hash"].as_str() {
        println!("\n✅ Transaction sent!");
        println!("TX Hash: {}", tx_hash);
        println!("Block: {}", result["result"]["transaction_outcome"]["block_hash"]);
        println!("\n⏳ Wait 10-20 seconds for indexer, then check:");
        println!("  curl 'http://localhost:3001/v1/kv/query?accountId=kampouse.near&contractId=kampouse.near'");
    } else {
        println!("\nResult: {}", serde_json::to_string_pretty(&result)?);
    }
    
    Ok(())
}
