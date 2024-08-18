use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::runtime::Builder;
use versa_relay::TcpRelay;

fn main() {
    let runtime = Builder::new_multi_thread()
        .thread_name_fn(|| {
            static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
            let id = ATOMIC_ID.fetch_add(1, Ordering::SeqCst);
            format!("thread-{}", id)
        })
        .max_blocking_threads(1024)
        .worker_threads(8)
        .thread_stack_size(512 * 1024)
        .enable_io()
        .build()
        .unwrap();

    runtime.block_on(async {
        let tcp_relay = TcpRelay::new("192.168.1.35".to_string(), 8095).await;
        tcp_relay.start().await.unwrap();
    });
}
