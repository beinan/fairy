use crate::kv_store::local_kv_store::local_file_kv_store::LocalFileKVStore;
use bytes::Bytes;
use h2::RecvStream;
use h2::server::SendResponse;
use log::{error, info, debug};
use monoio::net::{TcpListener, TcpStream};
use monoio_compat::StreamWrapper;
use http::Request;

pub struct H2Service<'a>
{
    kv_store: &'a LocalFileKVStore,
    addr: &'a str
}

impl H2Service <'static>{
    pub fn new(kv_store: &'static LocalFileKVStore, addr: &'static str) -> Self {
        H2Service {
            kv_store,
            addr,
        }
    }

    pub async fn serve_h2(&'static self) {
        let listener = TcpListener::bind(self.addr).unwrap();
        loop {
            if let Ok((socket, peer_addr)) = listener.accept().await {
                monoio::spawn(async move {
                    debug!("h2 connection received from {}", peer_addr);
                    if let Err(e) = self.serve(socket).await {
                        error!("h2 serve error  -> err={:?} peer={}", e, peer_addr);
                    }
                });
            }
        }
    }

    async fn serve(&'static self, socket: TcpStream) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let socket_wrapper = StreamWrapper::new(socket);
        let mut connection = h2::server::handshake(socket_wrapper).await?;
        debug!("H2 connection bound");

        let http_service = self;
        while let Some(result) = connection.accept().await {
            let (request, respond) = result?;
            monoio::spawn(async move {
                if let Err(e) = http_service.handle_request(request, respond).await {
                    error!("error while handling request: {e}");
                }
            });
        }

        debug!("H2 connection close.");
        Ok(())
    }

    async fn handle_request(
        &self,
        request: http::Request<h2::RecvStream>,
        respond: h2::server::SendResponse<bytes::Bytes>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("GOT request: {request:?}");
        let uri_parse_result = self.parse_uri(&request);
        match uri_parse_result {
            ("get", id) => self.get_object(id, respond).await,
            ("put", id) => self.put_object(id, request, respond).await,
            _ => {
                error!("unsupported ops {:?}", uri_parse_result);
                Ok(())
            }
        }
    }

    fn parse_uri(&self, request: &http::Request<h2::RecvStream>) -> (&str, String){
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
        &self,
        id: String,
        request: Request<RecvStream>,
        mut respond: SendResponse<Bytes>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!(">>>> receive {}", id);
        let mut body = request.into_body();//request.body_mut();

        while let Some(chunk) = body.data().await {
            debug!("GOT CHUNK = {:?}", chunk.unwrap());
        }
        let response = http::Response::new(());
        let mut send = respond.send_response(response, false)?;
        send.send_data(bytes::Bytes::from_static(b"world\n"), true)?;
        Ok(())
    }

    async fn get_object(
        &self,
        id: String,
        mut respond: h2::server::SendResponse<bytes::Bytes>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let chunk_size = 128;

        let response = http::Response::new(());
        let mut send = respond.send_response(response, false)?;
        debug!("h2 is sending data {}", id);

        let metadata = std::fs::metadata("README.md")?;
        let file_size = metadata.len();
        let buf = vec![0; file_size as usize];
        let buf = self.kv_store.get(id, buf).await.expect("read data failed from local");
        send.send_data(bytes::Bytes::from(buf), true)?;
        Ok(())
    }
}