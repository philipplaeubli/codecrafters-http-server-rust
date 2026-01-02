use std::net::TcpListener;
use std::{
    io::{BufRead, Read, Write},
    net::TcpStream,
};

use anyhow::{Context, Error, Result};
use bytes::BytesMut;

struct HttpRequest {
    method: String,
    path: String,
    headers: Vec<String>,
    body: Vec<u8>,
}
impl HttpRequest {
    fn from_bytes(bytes: BytesMut) -> Result<HttpRequest, Error> {
        let lines: Vec<String> = bytes
            .lines()
            .map(|line| line.context("Invalid request"))
            .collect::<Result<Vec<_>, _>>()?;

        let request_line = lines.get(0).context("No request line")?;
        let request_line_parts: Vec<&str> = request_line.split_whitespace().collect();
        if request_line_parts.len() != 3 {
            anyhow::bail!(
                "invalid request line: expected 3 parts, got {}",
                request_line_parts.len()
            );
        }
        Ok(HttpRequest {
            method: request_line_parts[0].to_string(),
            path: request_line_parts[1].to_string(),
            headers: vec![],
            body: vec![],
        })
    }
}

fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:4221").context("Unable to bind port")?;

    for stream in listener.incoming() {
        let stream = stream.context("Unable to accept connection")?;
        if let Err(e) = handle_connection(stream) {
            eprintln!("Connection error: {e:?}");
        };
    }
    Ok(())
}

fn handle_connection(mut stream: TcpStream) -> Result<()> {
    let mut input = BytesMut::zeroed(1024);
    let _ = stream.read(&mut input).context("Failed to read")?;
    let request = HttpRequest::from_bytes(input)?;
    let mut response: &[u8] = b"HTTP/1.1 404 Not Found\r\n\r\n";

    if request.path == "/" {
        response = b"HTTP/1.1 200 OK\r\n\r\n";
    }

    stream.write_all(response).context("Unable to write")
}
