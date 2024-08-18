use versa_relay::TcpRelay;

fn main() {
    let tcp_relay = TcpRelay::new("192.168.1.35".to_string(), 8095);
    tcp_relay.start().unwrap();
}
