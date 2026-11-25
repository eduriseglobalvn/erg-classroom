//! `erg-gateway [bind-address]` (default 0.0.0.0:47810). Runs on Windows or Linux.
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();
    let bind = std::env::args().nth(1).unwrap_or_else(|| "0.0.0.0:47810".into());
    let addr: std::net::SocketAddr = bind.parse().unwrap_or_else(|e| {
        eprintln!("bad address {bind}: {e}");
        std::process::exit(2);
    });
    tracing::info!("ERG gateway listening on {addr}");
    if let Err(e) = erg_gateway::run(addr, None).await {
        eprintln!("gateway stopped: {e}");
        std::process::exit(1);
    }
}
