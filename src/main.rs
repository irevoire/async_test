use tokio::io;
use tokio::sync::Mutex;
use tokio::net::{TcpListener, TcpStream};

use futures::future::try_select;
use futures::FutureExt;
use futures::TryFutureExt;
use std::env;
use std::error::Error;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let listen_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8081".to_string());
    let server_addr = env::args()
        .nth(2)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    println!("Listening on: {}", listen_addr);
    println!("Proxying to: {}", server_addr);

    let mut listener = TcpListener::bind(listen_addr).await?;
    let cpt = Mutex::new(0);
    let cpt = Arc::new(cpt);

    while let Ok((inbound, _)) = listener.accept().await {
        let server_addr = server_addr.clone();
        let cpt = cpt.clone();
        let connection = start_connection(cpt.clone())
            .then(move |_| transfer(inbound, server_addr))
            .then(move |_| end_connection(cpt.clone()))
            .map(|_| ());

        tokio::spawn(connection);
    }

    Ok(())
}

async fn start_connection(cpt: Arc<Mutex<usize>>) -> Result<(), Box<dyn Error>> {
    let mut base = cpt.lock().await;
    if *base == 0 {
        println!("First connection");
    }
    *base += 1;
    Ok(())
}

async fn end_connection(cpt: Arc<Mutex<usize>>) -> Result<(), Box<dyn Error>> {
    let mut base = cpt.lock().await;
    if *base == 0 {
        panic!("wtf happened");
    }
    if *base == 1 {
        println!("No connection left");
    }
    *base -= 1;
    Ok(())
}

async fn transfer(mut inbound: TcpStream, proxy_addr: String) -> Result<(), Box<dyn Error>> {
    let mut outbound = TcpStream::connect(proxy_addr).await?;

    let (mut ri, mut wi) = inbound.split();
    let (mut ro, mut wo) = outbound.split();

    let client_to_server = io::copy(&mut ri, &mut wo);
    let server_to_client = io::copy(&mut ro, &mut wi);

    // why the fuck can’t I just return the error
    try_select(client_to_server, server_to_client).map_err(|e| {
        match e {
            futures::future::Either::Left((e, _)) => e,
            futures::future::Either::Right((e, _)) => e,
        }}).await?;

    Ok(())
}
