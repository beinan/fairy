use monoio::io::{AsyncReadRent, AsyncWriteRentExt};
use monoio::join;
use monoio::net::{TcpListener, TcpStream};

pub mod h2_service;
pub mod hyper_service;

use fairy_common::settings;

use fairy_common::metrics::{INCOMING_REQUESTS, RESPONSE_TIME_COLLECTOR};
use hyper_service::{hyper_handler, serve_http};

use settings::SETTINGS;

mod service_registry;
use service_registry::etcd::ServiceRegistry;

use std::error::Error;

use log::{error, info};
use fairy_common::settings::Settings;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    fairy_common::logging::setup_logger().unwrap();

    let _ = register().await;
    let _ = fairy_common::metrics::start_push().await;

    let mut rt = monoio::RuntimeBuilder::<monoio::FusionDriver>::new()
        .with_entries(256)
        .enable_timer()
        .build()
        .unwrap();
    rt.block_on(async {
        let hyper_service = async {
            info!("Running http server on 0.0.0.0:{}", SETTINGS.http_port);
            let _ = serve_http(([0, 0, 0, 0], SETTINGS.http_port), hyper_handler).await;
        };

        let worker_kv_options: settings::local_kv_options::LocalFileKVStoreOptions = settings::parse_with_prefix("worker");

        info!("local kv options {:?}", worker_kv_options);

        let h2_service = async {
            info!("Running http2 server on 0.0.0.0:{}", SETTINGS.http2_port);
            let _ = h2_service::serve_h2(format!("127.0.0.1:{}", SETTINGS.http2_port)).await;
        };

        let socket_service = async {
            let listener =
                TcpListener::bind(format!("127.0.0.1:{}", SETTINGS.socket_port)).unwrap();
            info!("listening socket {}", SETTINGS.socket_port);
            loop {
                let incoming = listener.accept().await;
                match incoming {
                    Ok((stream, addr)) => {
                        error!("accepted a connection from {}", addr);
                        monoio::spawn(echo(stream));
                    }
                    Err(e) => {
                        error!("accepted connection failed: {}", e);
                        return;
                    }
                }
            }
        };

        join!(hyper_service, socket_service, h2_service);
    });

    Ok(())
}

async fn echo(mut stream: TcpStream) -> std::io::Result<()> {
    let mut buf: Vec<u8> = Vec::with_capacity(8 * 1024);
    let mut res;
    loop {
        let _timer = RESPONSE_TIME_COLLECTOR.start_timer();
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

async fn register() -> Result<(), Box<dyn Error>> {
    let registry = ServiceRegistry::new(&SETTINGS.etcd_uris).await?;
    registry.run().await?;

    Ok(())
}
