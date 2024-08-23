use std::{
    io::{BufRead, BufReader, Write},
    net::TcpStream,
    sync::{atomic::AtomicBool, Arc, Mutex},
};

use crate::{IgnorePoisoned, ThreadPool};

pub struct ConnectionHalf {
    sender: TcpStream,
    receiver: TcpStream,
    connected: Arc<AtomicBool>,
}

pub struct Connection;

impl Connection {
    fn prepare_stream(stream: TcpStream) -> (TcpStream, TcpStream) {
        (stream.try_clone().unwrap(), stream)
    }

    pub fn handle(client: TcpStream, server: TcpStream, thread_pool: Arc<Mutex<ThreadPool>>) {
        let client_addr = client.peer_addr().unwrap();

        let (client_reader, client_receiver) = Connection::prepare_stream(client);
        let (server_reader, server_receiver) = Connection::prepare_stream(server);

        let connection_status = Arc::new(AtomicBool::new(true));

        // Forward data from client to server.
        {
            let connection_status = Arc::clone(&connection_status);

            let identifier = format!("{client_addr} -> Server");
            thread_pool.lock().ignore_poisoned().execute(move || {
                Self::forwarding_loop(
                    client_reader,
                    server_receiver,
                    connection_status,
                    &identifier,
                );
            });
        }

        // Forward data from server to client.
        let identifier = format!("{client_addr} <- Server");
        Self::forwarding_loop(
            server_reader,
            client_receiver,
            connection_status,
            &identifier,
        );
    }

    fn forwarding_loop(
        mut sender: TcpStream,
        mut receiver: TcpStream,
        connection_status: Arc<AtomicBool>,
        identifier: &str,
    ) {
        let mut sender = BufReader::with_capacity(1500, &mut sender);

        while connection_status.load(std::sync::atomic::Ordering::Relaxed) {
            let buffer = match sender.fill_buf() {
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
                    eprintln!("[{identifier}] Error writing to the receiver: {e}.");
                    break;
                }
            };

            sender.consume(bytes_sent);
        }

        connection_status.store(false, std::sync::atomic::Ordering::Relaxed);
        if let Err(e) = receiver.shutdown(std::net::Shutdown::Both) {
            eprintln!("[{identifier}] Failed to close stream with receiver: {e}.");
        };
    }
}
