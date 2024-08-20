use versa_relay::{TcpRelay, ThreadPool};

fn main() {
    let thread_pool = ThreadPool::builder().max_threads(256).idle_threads(8).build();
    let tcp_relay = TcpRelay::new(thread_pool, "192.168.1.35".to_string(), 1080);
    tcp_relay.start();
}
