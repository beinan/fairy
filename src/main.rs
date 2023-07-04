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
use metrics::push_metrics;
use metrics::{INCOMING_REQUESTS, RESPONSE_TIME_COLLECTOR};

use settings::SETTINGS;

mod service_registry;
use service_registry::etcd::ServiceRegistry;

use std::error::Error;

use log::{error, info};
use std::time::SystemTime;

use fern::colors::{Color, ColoredLevelConfig};

use std::time::Duration;
use tokio::time::sleep;


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger().unwrap();

    let _ = register().await;
    let _ = push().await;

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
        
        let socket_service = async {
            let listener = TcpListener::bind(format!("127.0.0.1:{}", SETTINGS.socket_port)).unwrap();
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
    
        join!(hyper_service, socket_service);
    
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

async fn register() -> Result<(), Box<dyn Error>>{
    let registry = ServiceRegistry::new(&SETTINGS.etcd_uris).await?;
    registry.run().await?;

    Ok(())
}
async fn push() -> Result<(), Box<dyn Error>> {
    tokio::spawn(async move {
        loop{
            tokio::task::spawn_blocking(move || { 
                let _ = push_metrics();    
            });
            sleep(Duration::from_secs(30)).await; 
        }
    });

    tokio::task::yield_now().await;
    Ok(())
}

fn setup_logger() -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            let colors_line = ColoredLevelConfig::new()
                .info(Color::Green)
                .warn(Color::Yellow)
                .error(Color::Red);
            out.finish(format_args!(
                "{} [{} {} {}] {}",
                format_args!(
                    "\x1B[{}m",
                    colors_line.get_color(&record.level()).to_fg_str()),
                humantime::format_rfc3339_seconds(SystemTime::now()),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .level_for("fairy", log::LevelFilter::Trace) 
        .chain(std::io::stdout())
        .chain(fern::log_file("output.log")?)
        .apply()?;
    Ok(())
}