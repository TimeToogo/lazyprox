use std::{
    net::{SocketAddr, ToSocketAddrs},
    time::Duration,
};

use clap::Parser;
use futures::future;
use log::{debug, error, info};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::{TcpSocket, TcpStream, tcp::{OwnedReadHalf, OwnedWriteHalf}},
    sync::mpsc::{self, UnboundedSender},
    time,
};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(
        short,
        long,
        value_parser,
        help = "Bind socket address of TCP listener, eg 0.0.0.0:2222"
    )]
    listen: String,

    #[clap(
        short,
        long,
        value_parser,
        help = "Socket address of TCP destination, eg localhost:22"
    )]
    dest: String,

    #[clap(
        short,
        long,
        value_parser,
        help = "Number of seconds of idle time before exiting"
    )]
    idle_timeout_secs: u32,
}

#[tokio::main]
async fn main() {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );
    let args = Args::parse();

    let bind_addr: SocketAddr = args
        .listen
        .to_socket_addrs()
        .expect("Failed to parse --listen arg as socket address")
        .filter(|i| i.is_ipv4())
        .next()
        .unwrap();
    let dest_addr: SocketAddr = args
        .dest
        .to_socket_addrs()
        .expect("Failed to parse --dest arg as socket address")
        .filter(|i| i.is_ipv4())
        .next()
        .unwrap();

    let socket = TcpSocket::new_v4().unwrap();
    socket.set_reuseaddr(true).unwrap();
    socket
        .bind(bind_addr)
        .expect(&format!("Failed to bind to {}", bind_addr));

    let listener = socket.listen(1024).unwrap();
    info!("Listening on {}", bind_addr);

    let timeout = Duration::from_secs(args.idle_timeout_secs as _);
    let (activity_tx, mut activity_rx) = mpsc::unbounded_channel();

    loop {
        tokio::select! {
            accept = listener.accept() => {
                let (upstream, addr) = accept.expect("Failed to accept connection");
                info!("Received connection from {}", addr);

                let downstream = match TcpStream::connect(dest_addr).await {
                    Ok(dest) => dest,
                    Err(err) => {
                        error!("Failed to connect to {dest_addr}: {err}");
                        continue;
                    }
                };

                tokio::spawn(forward(upstream, downstream, activity_tx.clone()));
            },
            _ = activity_rx.recv() => {
                debug!("Notified of activity");
            }
            _ = time::timeout(timeout, future::pending::<()>()) => {
                info!("Timed out after {} seconds while waiting for activity, exiting...", args.idle_timeout_secs);
                break;
            }
        }
    }

    info!("Exiting");
}

async fn forward(upstream: TcpStream, downstream: TcpStream, tx: UnboundedSender<()>) {
    let upstream_addr = upstream.peer_addr().ok();
    let downstream_addr = downstream.peer_addr().ok();

    info!(
        "Forwarding from {:?} to {:?}",
        upstream_addr, downstream_addr
    );

    let (upstream_read, upstream_write) = upstream.into_split();
    let (downstream_read, downstream_write) = downstream.into_split();

    let res = tokio::select! {
        res = forward_loop(upstream_read, downstream_write, tx.clone()) => res,
        res = forward_loop(downstream_read, upstream_write, tx.clone()) => res,
    };

    match res {
        Ok(_) => info!(
            "Finished forwarding from {:?} to {:?}",
            upstream_addr, downstream_addr
        ),
        Err(err) => error!(
            "Error while forwarding from {:?} to {:?}: {}",
            upstream_addr, downstream_addr, err
        ),
    }

    let _ = tx.send(());
}

async fn forward_loop(
    mut input: OwnedReadHalf,
    mut output: OwnedWriteHalf,
    tx: UnboundedSender<()>,
) -> Result<(), io::Error> {
    let mut buff = [0u8; 1024];

    loop {
        if let Err(err) = output.writable().await {
            return Err(err);
        }

        let _ = tx.send(());

        let read = input.read(&mut buff).await?;
        let _ = tx.send(());

        if read == 0 {
            output.shutdown().await?;
            break;
        }

        output.write_all(&buff[..read]).await?;
        output.flush().await?;
        let _ = tx.send(());
    }

    Ok(())
}
