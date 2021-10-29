use anyhow::Result;
use clap::{App, Arg};
use godwoken_stress_test::godwoken::Plan;

#[tokio::main(flavor = "multi_thread", worker_threads = 200)]
pub async fn main() -> Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    let m = App::new("gw benchmark")
        .arg(
            Arg::with_name("interval")
                .short("i")
                .takes_value(true)
                .help("interval"),
        )
        .arg(
            Arg::with_name("batch")
                .short("b")
                .takes_value(true)
                .help("batch"),
        )
        .arg(
            Arg::with_name("account-path")
                .short("p")
                .takes_value(true)
                .help("accounts path"),
        )
        .arg(
            Arg::with_name("url")
                .short("u")
                .takes_value(true)
                .help("godwoken url"),
        )
        .arg(
            Arg::with_name("scripts_deployment_path")
                .short("s")
                .takes_value(true)
                .help("scripts_deployment path"),
        )
        .get_matches();

    let interval = m.value_of("interval").unwrap();
    let batch = m.value_of("batch").unwrap();
    let path = m.value_of("account-path").unwrap_or("accounts");
    let url = m.value_of("url").unwrap_or("localhost");
    let scripts_deployment_path = m.value_of("scripts_deployment_path").unwrap();
    Plan::new(
        interval.parse()?,
        batch.parse()?,
        path,
        url,
        scripts_deployment_path,
    )
    .await?
    .run()
    .await;

    Ok(())
}
