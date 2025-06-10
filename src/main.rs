mod utils;
mod xhtml;

fn main() {
    println!("ksjdfkjsdnfkjbfs");
}

/*
use http_body_util::Empty;
use hyper::Request;
use hyper::body::Bytes;
use hyper_tls::HttpsConnector;
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;

use http_body_util::BodyExt;
use tokio::io::{self, AsyncWriteExt as _};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Hello, world!");
    /*
     * parser.select_all("a > p.hello", Selection{
     *   selection: ["id", "textContent"]
     * });
     * parser.select();
     * */
    // The authority of // Parse our URL...
    let url = "http://httpbin.org/ip".parse::<hyper::Uri>()?;

    // Get the host and the port
    let host = url.host().expect("uri has no host");
    let port = url.port_u16().unwrap_or(80);

    let address = format!("{}:{}", host, port);

    // Open a TCP connection to the remote host
    let stream = TcpStream::connect(address).await?;

    // Use an adapter to access something implementing `tokio::io` traits as if they implement
    // `hyper::rt` IO traits.
    let io = TokioIo::new(stream);

    // Create the Hyper client
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;

    // Spawn a task to poll the connection, driving the HTTP state
    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            println!("Connection failed: {:?}", err);
        }
    });

    let authority = url.authority().unwrap().clone();

    // Create an HTTP request with an empty body and a HOST header
    let req = Request::builder()
        .uri(url)
        .header(hyper::header::HOST, authority.as_str())
        .body(Empty::<Bytes>::new())?;

    // Await the response...
    let mut res = sender.send_request(req).await?;

    println!("Response status: {}", res.status());

    // Stream the body, writing each frame to stdout as it arrives
    while let Some(next) = res.frame().await {
        let frame = next?;
        if let Some(chunk) = frame.data_ref() {
            io::stdout().write_all(chunk).await?;
        }
    }

    Ok(())
}

fn latin1_bytes_to_utf8_string(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}
*/
