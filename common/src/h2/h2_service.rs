use bytes::Bytes;
use h2::server::SendResponse;
use h2::RecvStream;
use http::Request;
use log::{debug, error};
use monoio::net::{TcpListener, TcpStream};
use monoio_compat::StreamWrapper;

use crate::kv_store::local_kv_store::local_file_kv_store::LocalFileKVStore;

pub struct H2Service {
    kv_store: &'static LocalFileKVStore,
    addr: &'static str,
}

impl H2Service {
    pub fn new(kv_store: &'static LocalFileKVStore, addr: &'static str) -> Self {
        H2Service { kv_store, addr }
    }

    pub async fn serve_h2(&self) {
        let listener = TcpListener::bind(self.addr).unwrap();
        loop {
            if let Ok((socket, peer_addr)) = listener.accept().await {
                let kv_store = self.kv_store;
                monoio::spawn(async move {
                    debug!("h2 connection received from {}", peer_addr);
                    if let Err(e) = H2Service::serve(socket, kv_store).await {
                        error!("h2 serve error  -> err={:?} peer={}", e, peer_addr);
                    }
                });
            }
        }
    }

    async fn serve(
        socket: TcpStream,
        kv_store: &'static LocalFileKVStore,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let socket_wrapper = StreamWrapper::new(socket);
        let mut connection = h2::server::handshake(socket_wrapper).await?;
        debug!("H2 connection bound");

        while let Some(result) = connection.accept().await {
            let (request, respond) = result?;
            monoio::spawn(async move {
                if let Err(e) = H2Service::handle_request(request, respond, kv_store).await {
                    error!("error while handling request: {e}");
                }
            });
        }

        debug!("H2 connection close.");
        Ok(())
    }

    async fn handle_request(
        request: http::Request<h2::RecvStream>,
        respond: h2::server::SendResponse<bytes::Bytes>,
        kv_store: &LocalFileKVStore,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("GOT request: {request:?}");
        let uri_parse_result = H2Service::parse_uri(&request);
        match uri_parse_result {
            ("get", id) => H2Service::get_object(id, respond, kv_store).await,
            ("put", id) => H2Service::put_object(id, request, respond, kv_store).await,
            _ => {
                error!("unsupported ops {:?}", uri_parse_result);
                Ok(())
            }
        }
    }

    fn parse_uri(request: &http::Request<h2::RecvStream>) -> (&str, String) {
        let rest_uri: Vec<&str> = {
            let uri = request.uri().path();
            uri.split('/').collect::<Vec<&str>>()
        };
        match rest_uri.as_slice() {
            ["", "get", id] => ("get", id.to_string()),
            ["", "put", id] => ("put", id.to_string()),
            _ => {
                error!("unsupported ops {:?}", rest_uri);
                ("none", String::from("n/a"))
            }
        }
    }

    async fn put_object(
        id: String,
        request: Request<RecvStream>,
        mut respond: SendResponse<Bytes>,
        kv_store: &LocalFileKVStore,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!(">>>> receive {}", id);
        //let mut body = request.into_body();//request.body_mut();
        let (_head, mut body) = request.into_parts();
        if let Some(chunk) = body.data().await {
            //println!("receive data {:?}{:?}", head, chunk.unwrap());
            kv_store
                .put(id, chunk.unwrap())
                .await
                .expect("TODO: panic message");
        }
        let response = http::Response::new(());
        let mut send = respond.send_response(response, false)?;
        send.send_data(bytes::Bytes::from_static(b"world\n"), true)?;
        Ok(())
    }

    async fn get_object(
        id: String,
        mut respond: h2::server::SendResponse<bytes::Bytes>,
        kv_store: &LocalFileKVStore,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let response = http::Response::new(());
        let mut send = respond.send_response(response, false)?;
        debug!("h2 is sending data {}", id);

        let buf = kv_store.get(id).await.expect("read data failed from local");
        send.send_data(bytes::Bytes::from(buf), true)?;
        Ok(())
    }
}
