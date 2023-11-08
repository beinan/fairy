mod ufs;

use bytes::Bytes;
use monoio::net::TcpStream;
use monoio_compat::StreamWrapper;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Turn trace information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Mount {
        mountpoint: PathBuf,
    },
    MountPassthrough {
        mountpoint: PathBuf,
        source: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    fairy_common::logging::setup_logger().unwrap();
    let cli = Cli::parse();
    match &cli.command {
        Some(Commands::Mount { mountpoint }) => {
            fairy_fuse::uring_mount(mountpoint);
        }
        Some(Commands::MountPassthrough { mountpoint, source }) => {
            fairy_fuse::mount_passthrough(mountpoint, source);
        }
        None => {}
    }
    // let s3_client = ufs::create_s3_client().await;
    //
    // ufs::list_objects(&s3_client, "beinan-test").await;
    // fairy_fuse::mount();
    let mut rt = monoio::RuntimeBuilder::<monoio::FusionDriver>::new()
        .with_entries(256)
        .enable_timer()
        .build()
        .unwrap();
    rt.block_on(async {
        let tcp = TcpStream::connect("127.0.0.1:5928").await.unwrap();
        let tcp_wrapper = StreamWrapper::new(tcp);
        let (client, h2) = h2::client::handshake(tcp_wrapper).await.unwrap();

        // Spawn a task to run the conn...
        monoio::spawn(async move {
            if let Err(e) = h2.await {
                println!("GOT ERR={e:?}");
            }
        });

        let mut client = client.ready().await.unwrap();
        let _ = put(&mut client).await;

        let _ = get(&mut client).await;
    });
    Ok(())
}

#[allow(clippy::needless_pass_by_ref_mut)]
async fn get(client: &mut h2::client::SendRequest<bytes::Bytes>) {
    let request = http::Request::builder().uri("/get/1111").body(()).unwrap();

    let mut trailers = http::HeaderMap::new();
    trailers.insert("zomg", "hello".parse().unwrap());

    let (response, mut stream) = client.send_request(request, false).unwrap();

    // send trailers
    stream.send_trailers(trailers).unwrap();

    let response = response.await.unwrap();
    println!("GOT GET RESPONSE: {response:?}");

    // Get the body
    let mut body = response.into_body();

    while let Some(chunk) = body.data().await {
        println!("GOT CHUNK = {:?}", chunk.unwrap());
    }

    if let Some(trailers) = body.trailers().await.unwrap() {
        println!("GOT TRAILERS: {trailers:?}");
    }
}
#[allow(clippy::needless_pass_by_ref_mut)]
async fn put(
    client: &mut h2::client::SendRequest<Bytes>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    //let mut client = client.ready().await.unwrap();
    let request = http::Request::builder().uri("/put/1111").body(()).unwrap();

    let (response, mut stream) = client.send_request(request, false).unwrap();

    stream.send_data(bytes::Bytes::from_static(b"world\n"), false)?;

    let mut trailers = http::HeaderMap::new();
    trailers.insert("zomg", "hello".parse().unwrap());
    stream.send_trailers(trailers).unwrap();

    let response = response.await.unwrap();
    println!("GOT PUT RESPONSE: {response:?}");

    Ok(())
}
