use anyhow::Result;
use clap::{App, Arg, SubCommand};
use godwoken_benchmark::{benchmark, utils};

#[tokio::main(flavor = "multi_thread", worker_threads = 200)]
pub async fn main() -> Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    let m = App::new("gw benchmark")
        .subcommand(
            SubCommand::with_name("run")
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
                    Arg::with_name("timeout")
                        .short("t")
                        .takes_value(true)
                        .default_value("120")
                        .help("tiemout in second"),
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
                .arg(
                    Arg::with_name("rollup_type_hash")
                        .short("h")
                        .takes_value(true)
                        .help("Rollup type hash"),
                ),
        )
        .subcommand(
            SubCommand::with_name("privkey-to-eth-addr")
                .arg(Arg::with_name("privkey").short("pk").takes_value(true)),
        )
        .get_matches();

    if let Some(m) = m.subcommand_matches("run") {
        let interval = m.value_of("interval").unwrap();
        let batch = m.value_of("batch").unwrap();
        let timeout = m.value_of("timeout").unwrap();
        let path = m.value_of("account-path").unwrap_or("accounts");
        let url = m.value_of("url").unwrap_or("localhost");
        let rollup_type_hash = m.value_of("rollup_type_hash").unwrap();
        let scripts_deployment_path = m.value_of("scripts_deployment_path").unwrap();
        benchmark::run(
            interval.parse()?,
            batch.parse()?,
            timeout.parse()?,
            path,
            url,
            scripts_deployment_path,
            rollup_type_hash.to_string(),
        )
        .await?;
    }

    if let Some(m) = m.subcommand_matches("privkey-to-eth-addr") {
        let privkey = m.value_of("privkey").unwrap().to_string();
        let privkey = utils::read_privkey(privkey)?;
        let eth_addr = utils::privkey_to_eth_address(&privkey)?;
        println!("{}", hex::encode(&eth_addr));
    }

    Ok(())
}
