#[tokio::main]
async fn main() {
    if let Err(error) = mtr_hk::server::run_stdio().await {
        eprintln!("mtr-hk-mcp: {error}");
        std::process::exit(1);
    }
}
