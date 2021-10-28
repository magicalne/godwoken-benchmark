use anyhow::Result;
use godwoken_stress_test::godwoken;

#[tokio::main(flavor = "multi_thread", worker_threads = 200)]
pub async fn main() -> Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    log::info!("test");
    let mut godwoken = godwoken::Godwoken::new(
        "./accounts",
        "http://localhost:8119",
        "./scripts-deploy-result.json",
    )
    .await?;
    godwoken.check_balance(300).await?;
    // godwoken.transfer_to_first_empty_account().await?;
    let account = godwoken
        .get_account_id_by_short_address("63da2f11825dc79e7f4db47b1c25efa8bb897f0d")
        .await;
    log::info!("account: {:?}", account);
    Ok(())
}
