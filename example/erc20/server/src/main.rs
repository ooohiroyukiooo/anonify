use actix_web::{web, App, HttpServer};
use anonify_eth_driver::eth::*;
use erc20_server::handlers::*;
use erc20_server::Server;
use frame_host::EnclaveDir;
use std::{env, io, sync::Arc};

#[actix_web::main]
async fn main() -> io::Result<()> {
    tracing_subscriber::fmt::init();
    let anonify_url = env::var("ANONIFY_URL").expect("ANONIFY_URL is not set.");
    let num_workers: usize = env::var("NUM_WORKERS")
        .unwrap_or_else(|_| "16".to_string())
        .parse()
        .expect("Failed to parse NUM_WORKERS");

    // Enclave must be initialized in main function.
    let enclave = EnclaveDir::new()
        .init_enclave(true)
        .expect("Failed to initialize enclave.");
    let eid = enclave.geteid();
    let server = Arc::new(Server::<EthDeployer, EthSender, EventWatcher>::new(eid));

    HttpServer::new(move || {
        App::new()
            .data(server.clone())
            .route(
                "/api/v1/deploy",
                web::post().to(handle_deploy::<EthDeployer, EthSender, EventWatcher>),
            )
            .route(
                "/api/v1/join_group",
                web::post().to(handle_join_group::<EthDeployer, EthSender, EventWatcher>),
            )
            .route(
                "/api/v1/update_mrenclave",
                web::post().to(handle_update_mrenclave::<EthDeployer, EthSender, EventWatcher>),
            )
            .route(
                "/api/v1/init_state",
                web::post().to(handle_init_state::<EthDeployer, EthSender, EventWatcher>),
            )
            .route(
                "/api/v1/transfer",
                web::post().to(handle_transfer::<EthDeployer, EthSender, EventWatcher>),
            )
            .route(
                "/api/v1/key_rotation",
                web::post().to(handle_key_rotation::<EthDeployer, EthSender, EventWatcher>),
            )
            .route(
                "/api/v1/approve",
                web::post().to(handle_approve::<EthDeployer, EthSender, EventWatcher>),
            )
            .route(
                "/api/v1/transfer_from",
                web::post().to(handle_transfer_from::<EthDeployer, EthSender, EventWatcher>),
            )
            .route(
                "/api/v1/mint",
                web::post().to(handle_mint::<EthDeployer, EthSender, EventWatcher>),
            )
            .route(
                "/api/v1/burn",
                web::post().to(handle_burn::<EthDeployer, EthSender, EventWatcher>),
            )
            .route(
                "/api/v1/allowance",
                web::get().to(handle_allowance::<EthDeployer, EthSender, EventWatcher>),
            )
            .route(
                "/api/v1/balance_of",
                web::get().to(handle_balance_of::<EthDeployer, EthSender, EventWatcher>),
            )
            .route(
                "/api/v1/start_sync_bc",
                web::get().to(handle_start_sync_bc::<EthDeployer, EthSender, EventWatcher>),
            )
            .route(
                "/api/v1/set_contract_addr",
                web::get().to(handle_set_contract_addr::<EthDeployer, EthSender, EventWatcher>),
            )
            .route(
                "/api/v1/register_notification",
                web::post()
                    .to(handle_register_notification::<EthDeployer, EthSender, EventWatcher>),
            )
            .route(
                "/api/v1/encrypting_key",
                web::get().to(handle_encrypting_key::<EthDeployer, EthSender, EventWatcher>),
            )
            .route(
                "/api/v1/register_report",
                web::post().to(handle_register_report::<EthDeployer, EthSender, EventWatcher>),
            )
    })
    .bind(anonify_url)?
    .workers(num_workers)
    .run()
    .await
}
