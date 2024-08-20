use anyhow::Result;
use std::{
    io::{BufRead, BufReader, Read, Write},
    marker::PhantomData,
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
};

use crate::{IgnorePoisoned, ThreadPool};

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
        let (client_read, client_write) = (client.try_clone().unwrap(), client);
        let (server_read, server_write) = (server.try_clone().unwrap(), server);

        // Forward data from client to server.
        let identifier = format!("{} -> Server", client_write.peer_addr().unwrap());
        thread_pool
            .lock()
            .ignore_poisoned()
            .execute(move || TcpRelay::forwarder(client_read, server_write, identifier));

        // Forward data from server to client.
        let identifier = format!("{} <- Server", client_write.peer_addr().unwrap());
        TcpRelay::forwarder(server_read, client_write, identifier);
    }

    fn forwarder<S: Read, R: Write>(mut sender: S, mut receiver: R, identifier: String) {
        let mut reader = BufReader::with_capacity(1500, &mut sender);

        loop {
            let buffer = match reader.fill_buf() {
                Ok(buffer) => buffer,
                Err(e) => {
                    eprintln!("[{identifier}] Failed to read data from the sender: {e}.");
                    break;
                }
            };

            let bytes_received = buffer.len();
            if bytes_received == 0 {
                println!("[{identifier}] Connection closed by the sender.");
                break;
            }

            // println!(
            //     "[{identifier}] Received {} byte(s) from the sender.",
            //     bytes_received
            // );

            let bytes_sent = match receiver.write(buffer) {
                Ok(0) => {
                    eprintln!("[{identifier}] The receiver is no longer accepting data.",);
                    break;
                }
                Ok(bytes_sent) => bytes_sent,
                Err(e) => {
                    eprintln!("[{identifier}] Error writing to the receiver: {}.", e);
                    break;
                }
            };

            reader.consume(bytes_sent);
        }
    }
}
