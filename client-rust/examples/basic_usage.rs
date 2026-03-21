use key_manager_client::{PrivateKV, OutLayerAdapter, FastKVAdapter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let kv = PrivateKV::new(
        OutLayerAdapter::new(
            "https://api.outlayer.fastnear.com",
            "wk_your_api_key"
        ),
        FastKVAdapter::new("https://your-fastkv-server.com"),
    );

    // Store encrypted value
    kv.set("api-key", "sk_live_1234567890", "user.near/private").await?;
    println!("✅ Value encrypted and stored");

    // Retrieve and decrypt
    let value = kv.get("api-key", "user.near/private").await?;
    println!("✅ Decrypted: {:?}", value);

    // Delete
    kv.delete("api-key", "user.near/private").await?;
    println!("✅ Value deleted");

    Ok(())
}
