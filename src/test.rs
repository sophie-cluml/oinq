//! Shared test code

use async_once::AsyncOnce;
use lazy_static::lazy_static;
use quinn::{RecvStream, SendStream};
use tokio::sync::RwLock;

pub(crate) struct Channel {
    pub(crate) server: Endpoint,
    pub(crate) client: Endpoint,
}

pub(crate) struct Endpoint {
    pub(crate) send: SendStream,
    pub(crate) recv: RecvStream,
}

lazy_static! {
    pub(crate) static ref CHANNEL: AsyncOnce<RwLock<Channel>> = AsyncOnce::new(channel());
}

/// Creates a bidirectional channel, returning server's send and receive and
/// client's send and receive streams.
pub(crate) async fn channel() -> RwLock<Channel> {
    use std::net::{IpAddr, Ipv6Addr, SocketAddr};

    use futures::StreamExt;

    const TEST_SERVER_NAME: &str = "test-server";
    const TEST_PORT: u16 = 60190;

    let cert =
        rcgen::generate_simple_self_signed([TEST_SERVER_NAME.to_string()]).expect("infallible");
    let cert_chain = vec![rustls::Certificate(
        cert.serialize_der().expect("infallible"),
    )];
    let key_der = rustls::PrivateKey(cert.serialize_private_key_der());
    let server_config =
        quinn::ServerConfig::with_single_cert(cert_chain, key_der).expect("infallible");
    let server_addr = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), TEST_PORT);
    let (_server_endpoint, mut incoming) =
        quinn::Endpoint::server(server_config, server_addr).unwrap();

    let mut root_cert_store = rustls::RootCertStore::empty();
    root_cert_store.add_parsable_certificates(&[cert.serialize_der().expect("infallible")]);
    let client_endpoint =
        quinn::Endpoint::client(SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0)).unwrap();
    let client_config = quinn::ClientConfig::with_root_certificates(root_cert_store);
    let client_connecting = client_endpoint
        .connect_with(client_config, server_addr, TEST_SERVER_NAME)
        .unwrap();

    let server_connecting = incoming.next().await.unwrap();

    let client_new_connection = client_connecting.await.unwrap();

    let mut server_new_connection = server_connecting.await.unwrap();

    let (mut client_send, client_recv) = client_new_connection.connection.open_bi().await.unwrap();
    client_send.write_all(b"ready").await.unwrap();

    let (server_send, mut server_recv) = server_new_connection
        .bi_streams
        .next()
        .await
        .unwrap()
        .unwrap();
    let mut server_buf = [0; 5];
    server_recv.read_exact(&mut server_buf).await.unwrap();

    RwLock::new(Channel {
        server: self::Endpoint {
            send: server_send,
            recv: server_recv,
        },
        client: self::Endpoint {
            send: client_send,
            recv: client_recv,
        },
    })
}
