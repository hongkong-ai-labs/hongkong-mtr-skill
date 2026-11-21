#[tokio::main]
async fn main() {
    if let Err(error) = mtr_hk::server::run_http_from_env().await {
        eprintln!("mtr-hk-mcp-http: {error}");
        std::process::exit(1);
    }
}
