use anyhow::Result;
use lazy_static::lazy_static;
use log::{error, info};
use monoio::io::{AsyncReadRent, AsyncWriteRentExt};
use monoio::join;
use monoio::net::{TcpListener, TcpStream};

use fairy_common::kv_store::local_kv_store::local_file_kv_store::LocalFileKVStore;
use fairy_common::metrics::{INCOMING_REQUESTS, RESPONSE_TIME_COLLECTOR};
use fairy_common::settings;
use hyper_service::{hyper_handler, serve_http};
use service_registry::etcd::{ServiceRegistry, ServiceRegistryError};
use settings::SETTINGS;

pub mod h2_service;
pub mod hyper_service;

mod service_registry;

lazy_static! {
    static ref KV_STORE: LocalFileKVStore =
        LocalFileKVStore::new(settings::parse_with_prefix("worker"));
    static ref H2_ADDR: String = format!("127.0.0.1:{}", SETTINGS.http2_port);
}

#[tokio::main]
async fn main() -> Result<()> {
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

        let h2_service = fairy_common::h2::h2_service::H2Service::new(&KV_STORE, H2_ADDR.as_str());
        let h2_service = h2_service.serve_h2();

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

async fn register() -> Result<(), ServiceRegistryError> {
    let registry = ServiceRegistry::new(&SETTINGS.etcd_uris).await?;
    registry.run().await?;

    Ok(())
}
