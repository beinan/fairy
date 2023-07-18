mod ufs;

use bytes::Bytes;
use h2::client::SendRequest;
use monoio::net::TcpStream;
use monoio_compat::StreamWrapper;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let s3_client = ufs::create_s3_client().await;
    //
    // ufs::list_objects(&s3_client, "beinan-test").await;

    let mut rt = monoio::RuntimeBuilder::<monoio::FusionDriver>::new()
        .with_entries(256)
        .enable_timer()
        .build()
        .unwrap();
    rt.block_on(async {
        let tcp = TcpStream::connect("127.0.0.1:5928").await.unwrap();
        let tcp_wrapper = StreamWrapper::new(tcp);
        let (mut client, h2) = h2::client::handshake(tcp_wrapper).await.unwrap();

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

async fn get(client: &mut SendRequest<Bytes>) {
    let request = http::Request::builder()
        .uri("/get/1111")
        .body(())
        .unwrap();

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

async fn put(client: &mut SendRequest<Bytes>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    //let mut client = client.ready().await.unwrap();
    let request = http::Request::builder()
        .uri("/put/1111")
        .body(())
        .unwrap();

    let (response, mut stream) = client.send_request(request, false).unwrap();


    stream.send_data(bytes::Bytes::from_static(b"world\n"), false)?;

    let mut trailers = http::HeaderMap::new();
    trailers.insert("zomg", "hello".parse().unwrap());
    stream.send_trailers(trailers).unwrap();


    let response = response.await.unwrap();
    println!("GOT PUT RESPONSE: {response:?}");

    Ok(())
}
