use versa_relay::TcpRelay;

#[tokio::main]
async fn main() {
    let tcp_relay = TcpRelay::new("192.168.1.35".to_string(), 8095).await;
    tcp_relay.start().await.unwrap();
}
