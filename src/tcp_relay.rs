use anyhow::Result;
use bytes::BytesMut;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpStream,
    },
};

pub struct TcpRelay {
    server_addr: String,
    server_port: u16,
}

impl TcpRelay {
    pub async fn new(address: String, port: u16) -> TcpRelay {
        TcpRelay {
            server_addr: address,
            server_port: port,
        }
    }

    pub async fn start(&self) -> Result<()> {
        let listener = TcpListener::bind(("0.0.0.0", self.server_port))
            .await
            .unwrap();

        loop {
            let (local, _) = listener.accept().await?;
            let remote = TcpStream::connect((self.server_addr.as_str(), self.server_port))
                .await
                .unwrap(); // TODO: handle.

            println!("[{:?}] New client connecting to server.", local.peer_addr());

            tokio::spawn(async move { TcpRelay::handle_connection(local, remote).await.unwrap() });
        }
    }

    async fn handle_connection(client: TcpStream, server: TcpStream) -> Result<()> {
        let (client_read, client_write) = client.into_split();
        let (server_read, server_write) = server.into_split();

        // Forward data from client to server.
        let identifier = format!("{} -> Server", client_write.peer_addr().unwrap());
        let _handle = tokio::spawn(async move {
            TcpRelay::forwarder(client_read, server_write, identifier).await;
        });

        // Forward data from server to client.
        let identifier = format!("{} <- Server", client_write.peer_addr().unwrap());
        TcpRelay::forwarder(server_read, client_write, identifier).await;

        // handle.await?;

        Ok(())
    }

    async fn forwarder(mut read: OwnedReadHalf, mut write: OwnedWriteHalf, identifier: String) {
        loop {
            let mut buffer = BytesMut::with_capacity(1500);

            let _bytes_received = match read.read_buf(&mut buffer).await {
                Ok(0) => {
                    println!("[{identifier}] Connection closed by the sender.");
                    break;
                }
                Ok(bytes_received) => bytes_received,
                Err(e) => {
                    eprintln!("[{identifier}] Failed to read data from the sender: {e}.");
                    break;
                }
            };
            // println!(
            //     "[{identifier}] Received {} byte(s) from the sender.",
            //     bytes_received
            // );

            match write.write_buf(&mut buffer).await {
                Ok(0) => {
                    eprintln!("[{identifier}] The receiver is no longer accepting data.",);
                    break;
                }
                Err(e) => {
                    eprintln!("[{identifier}] Error writing to the receiver: {}.", e);
                    break;
                }
                _ => (),
            }
        }
    }
}
