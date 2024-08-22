use std::{
    marker::PhantomData,
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
};

use crate::{Connection, IgnorePoisoned, ThreadPool};

pub struct Ready;
pub struct Running;

pub struct TcpRelay<T = Ready> {
    thread_pool: Arc<Mutex<ThreadPool>>,
    server_addr: String,
    server_port: u16,
    state: PhantomData<T>,
}

impl TcpRelay {
    pub fn new(thread_pool: ThreadPool, address: String, port: u16) -> TcpRelay<Ready> {
        TcpRelay {
            thread_pool: Arc::new(Mutex::new(thread_pool)),
            server_addr: address,
            server_port: port,
            state: PhantomData,
        }
    }

    pub fn start(self) -> ! {
        let listener = TcpListener::bind(("0.0.0.0", self.server_port)).unwrap();

        loop {
            let (client, addr) = match listener.accept() {
                Ok(client) => client,
                Err(e) => {
                    eprintln!("Failed to accept client connection: {e}.");
                    continue;
                }
            };
            let server = match TcpStream::connect((self.server_addr.as_str(), self.server_port)) {
                Ok(server) => server,
                Err(e) => {
                    eprintln!("Failed to connect to server: {e}.");
                    client.shutdown(std::net::Shutdown::Both).ok();
                    continue;
                }
            };

            println!("[{addr}] New client connecting to server.");

            let thread_pool = Arc::clone(&self.thread_pool);
            self.thread_pool
                .lock()
                .ignore_poisoned()
                .execute(move || TcpRelay::handle_connection(client, server, thread_pool));
        }
    }

    fn handle_connection(
        client: TcpStream,
        server: TcpStream,
        thread_pool: Arc<Mutex<ThreadPool>>,
    ) {
        Connection::handle(client, server, thread_pool);
    }
}
