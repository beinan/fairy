/// A echo example.
///
/// Run the example and `nc 127.0.0.1 50002` in another shell.
/// All your input will be echoed out.
use monoio::io::{AsyncReadRent, AsyncWriteRentExt};
use monoio::net::{TcpListener, TcpStream};
use monoio::join;


pub mod hyper_service;
pub mod metrics;
pub mod settings;

use hyper_service::{serve_http, hyper_handler};
use metrics::register_custom_metrics;
use metrics::{INCOMING_REQUESTS};

use settings::SETTINGS;

mod service_registry;
use service_registry::etcd::ServiceRegistry;

use std::error::Error;

#[monoio::main]
async fn main() {
    register_custom_metrics();
    
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _ = register(&rt).await;

    let hyper_service = async {
        println!("Running http server on 0.0.0.0:{}", SETTINGS.http_port);
        let _ = serve_http(([0, 0, 0, 0], SETTINGS.http_port), hyper_handler).await;
    };
    
    let socket_service = async {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", SETTINGS.socket_port)).unwrap();
        println!("listening socket {}", SETTINGS.socket_port);
        loop {
            let incoming = listener.accept().await;
            match incoming {
                Ok((stream, addr)) => {
                    println!("accepted a connection from {}", addr);
                    monoio::spawn(echo(stream));
                }
                Err(e) => {
                    println!("accepted connection failed: {}", e);
                    return;
                }
            }
        }
    };
    join!(hyper_service, socket_service);
    let _ = register(&rt).await;
}

async fn echo(mut stream: TcpStream) -> std::io::Result<()> {
    let mut buf: Vec<u8> = Vec::with_capacity(8 * 1024);
    let mut res;
    loop {
        // read
        (res, buf) = stream.read(buf).await;
        if res? == 0 {
            return Ok(());
        }
        INCOMING_REQUESTS.inc();
        // write all
        (res, buf) = stream.write_all(buf).await;
        res?;

        // clear
        buf.clear();
    }
}

async fn register(rt: &tokio::runtime::Runtime) -> Result<(), Box<dyn Error>>{
    rt.block_on(async {
        let registry = ServiceRegistry::new(["localhost:2379"]).await?;
        registry.run().await?;
    
        Ok(())
    }) 
}