#[allow(unused_imports)]
use std::net::TcpListener;
use std::{
    io::{Read, Write},
    net::TcpStream,
};

use anyhow::{Context, Result};
use bytes::BytesMut;

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

    println!("accepted new connection");
    stream
        .write_all(b"HTTP/1.1 200 OK\r\n\r\n")
        .context("Unable to write")
}
