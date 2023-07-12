use monoio::net::{TcpListener, TcpStream};
use monoio_compat::StreamWrapper;

use monoio::fs::File;

pub async fn serve_h2(addr: String) {
    let listener = TcpListener::bind(addr).unwrap();
    loop {
        if let Ok((socket, _peer_addr)) = listener.accept().await {
            monoio::spawn(async move {
                println!("h2 received!");
                if let Err(e) = serve(socket).await {
                    println!("  -> err={e:?}");
                }
            });
        }
    }
}

async fn serve(socket: TcpStream) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let socket_wrapper = StreamWrapper::new(socket);
    let mut connection = h2::server::handshake(socket_wrapper).await?;
    println!("H2 connection bound");

    while let Some(result) = connection.accept().await {
        let (request, respond) = result?;
        monoio::spawn(async move {
            if let Err(e) = handle_request(request, respond).await {
                println!("error while handling request: {e}");
            }
        });
    }

    println!("~~~~~~~~~~~ H2 connection CLOSE !!!!!! ~~~~~~~~~~~");
    Ok(())
}

async fn handle_request(
    request: http::Request<h2::RecvStream>,
    respond: h2::server::SendResponse<bytes::Bytes>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("GOT request: {request:?}");
    let rest_uri: Vec<&str> = request.uri().path().split('/').collect();
    match rest_uri.as_slice() {
        ["", "get", id] => get_object(id, respond).await,
        ["", "put", id] => get_object(id, respond).await,
        _ => {
            println!("unsupported ops {:?}", rest_uri);
            Ok(())
        }
    }

    // let body = request.body_mut();
    // while let Some(data) = body.data().await {
    //     let data = data?;
    //     println!("<<<< recv {data:?}");
    //     let _ = body.flow_control().release_capacity(data.len());
    // }
}

async fn get_object(
    id: &str,
    mut respond: h2::server::SendResponse<bytes::Bytes>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let chunk_size = 128;

    let response = http::Response::new(());
    let mut send = respond.send_response(response, false)?;
    println!(">>>> send {}", id);

    let metadata = std::fs::metadata("README.md")?;
    let file_size = metadata.len();

    let f = File::open("README.md").await?;

    let mut pos = 0;

    while pos < file_size {
        let read_length = if file_size - pos < chunk_size as u64 {
            (file_size - pos) as usize
        } else {
            chunk_size
        };
        let buffer = vec![0; read_length];

        let (_, buffer) = f.read_exact_at(buffer, 0).await;

        pos += read_length as u64;

        println!("pos {}, {:?}", pos, &buffer);

        send.send_data(bytes::Bytes::from(buffer), pos == file_size)?;
    }

    // Close the file
    f.close().await?;
    Ok(())
}
