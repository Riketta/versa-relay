use anyhow::Result;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    thread,
};

pub struct TcpRelay {
    server_addr: String,
    server_port: u16,
}

impl TcpRelay {
    pub fn new(address: String, port: u16) -> TcpRelay {
        TcpRelay {
            server_addr: address,
            server_port: port,
        }
    }

    pub fn start(&self) -> Result<()> {
        let listener = TcpListener::bind(("0.0.0.0", self.server_port)).unwrap();

        loop {
            let (client, addr) = listener.accept().unwrap();
            let server = TcpStream::connect((self.server_addr.as_str(), self.server_port)).unwrap(); // TODO: handle.

            println!("[{addr}] New client connecting to server.");

            thread::spawn(move || TcpRelay::handle_connection(client, server));
        }
    }

    fn handle_connection(client: TcpStream, server: TcpStream) -> Result<()> {
        let (client_read, client_write) = (client.try_clone().unwrap(), client);
        let (server_read, server_write) = (server.try_clone().unwrap(), server);

        // Forward data from client to server.
        let identifier = format!("{} -> Server", client_write.peer_addr().unwrap());
        let _handle =
            thread::spawn(move || TcpRelay::forwarder(client_read, server_write, identifier));

        // Forward data from server to client.
        let identifier = format!("{} <- Server", client_write.peer_addr().unwrap());
        TcpRelay::forwarder(server_read, client_write, identifier);

        Ok(())
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
