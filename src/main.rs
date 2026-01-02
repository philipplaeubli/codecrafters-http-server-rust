use std::env;

use crate::request::HttpRequest;
use crate::response::HttpResponse;
use anyhow::{Context, Result};
use bytes::BytesMut;
use flate2::Compression;
use flate2::write::GzEncoder;
use std::io::Write;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

mod request;
mod response;

#[derive(Debug, Clone)]
struct ServerConfig {
    static_directory: Option<String>,
}
impl ServerConfig {}
#[tokio::main]
async fn main() -> Result<()> {
    let mut args = env::args();
    let mut config = ServerConfig {
        static_directory: None,
    };
    println!("Arguments: {:?}", args);
    if args.len() > 2 {
        let directory_flag = args.nth(1).context("unable to parse directory flag")?;
        if directory_flag == "--directory" {
            let abs_directory = args.next().context("unable to parse absolute directory")?;
            config.static_directory = Some(abs_directory);
        }
    }

    let listener = TcpListener::bind("127.0.0.1:4221")
        .await
        .context("Unable to bind port")?;

    println!("Service ready with config: {:?}", config);
    loop {
        let (stream, _) = listener.accept().await?;
        let config = config.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, config).await {
                eprintln!("Connection error: {e:?}");
            }
        });
    }
}

async fn handle_connection(mut stream: TcpStream, config: ServerConfig) -> Result<()> {
    loop {
        let mut input = BytesMut::with_capacity(1024);

        let _ = stream
            .read_buf(&mut input)
            .await
            .context("Failed to read")?;

        let request = HttpRequest::from_bytes(input)?;
        if let Some(connection) = request.headers.get("Connection") {
            if connection == "close" {
                break;
            }
        }

        let response = handle_request(&request, &config);

        let result = match response {
            Ok(mut resp) => {
                if let Some(accept_encoding) = &request.headers.get("Accept-Encoding") {
                    println!("Accept-Encoding: {:?}", accept_encoding);

                    if accept_encoding.contains("gzip") {
                        let mut e = GzEncoder::new(Vec::new(), Compression::default());
                        e.write_all(&resp.body).unwrap();
                        resp.body = e.finish().unwrap();
                        resp.set_header("Content-Encoding".to_string(), "gzip".to_string());
                        resp.set_header("Content-Length".to_string(), resp.body.len().to_string());
                    }
                }

                resp
            }
            Err(_) => HttpResponse::internal_server_error(),
        };

        let _res = stream
            .write(result.encode().as_slice())
            .await
            .context("Unable to write")?;
    }
    Ok(())
}

fn handle_request(request: &HttpRequest, config: &ServerConfig) -> Result<HttpResponse> {
    let segments = request
        .path
        .split("/")
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<&str>>();

    if let Some(first_segment) = segments.first() {
        let resp = match *first_segment {
            "files" => {
                let Some(file_path) = segments.get(1) else {
                    return Ok(HttpResponse::not_found());
                };

                let Some(root_dir) = &config.static_directory else {
                    return Ok(HttpResponse::not_found());
                };

                let file_path = format!("{}{}", root_dir, file_path);

                match request.method.as_str() {
                    "POST" => {
                        if let Err(_err) = std::fs::write(&file_path, request.body.clone()) {
                            eprintln!("Error writing file: {:?}", _err);
                            return Ok(HttpResponse::internal_server_error());
                        }
                        HttpResponse::created()
                    }

                    "GET" => {
                        if let Ok(metadata) = std::fs::metadata(&file_path) {
                            if metadata.is_file() {
                                let mut resp = HttpResponse::ok();
                                resp.set_header(
                                    "Content-Type".to_string(),
                                    "application/octet-stream".to_string(),
                                );
                                resp.set_header(
                                    "Content-Length".to_string(),
                                    metadata.len().to_string(),
                                );
                                let body_content =
                                    std::fs::read(file_path).context("Failed to read file")?;
                                resp.set_body(body_content);
                                resp
                            } else {
                                HttpResponse::not_found()
                            }
                        } else {
                            HttpResponse::not_found()
                        }
                    }
                    _ => {
                        eprintln!("Unsupported method");
                        HttpResponse::internal_server_error()
                    }
                }
            }
            "echo" => {
                let message = *segments.get(1).unwrap_or(&"");

                let mut resp = HttpResponse::ok();
                resp.set_header("Content-Type".to_string(), "text/plain".to_string());
                resp.set_header("Content-Length".to_string(), message.len().to_string());
                resp.set_body(message.as_bytes().into());
                resp
            }
            "user-agent" => {
                if let Some(user_agent) = request.headers.get("User-Agent") {
                    let mut resp = HttpResponse::ok();
                    resp.set_header("Content-Type".to_string(), "text/plain".to_string());
                    resp.set_header("Content-Length".to_string(), user_agent.len().to_string());
                    resp.set_body(user_agent.as_bytes().into());
                    resp
                } else {
                    HttpResponse::internal_server_error()
                }
            }
            _ => HttpResponse::not_found(),
        };
        Ok(resp)
    } else {
        Ok(HttpResponse::ok())
    }
}

#[test]
fn tests_handle_request() {
    let config = ServerConfig {
        static_directory: None,
    };

    let actual = handle_request(
        &HttpRequest {
            body: vec![],
            path: "/".to_string(),
            method: "GET".to_string(),
            headers: std::collections::HashMap::new(),
        },
        &config,
    )
    .unwrap()
    .status_code;
    assert_eq!(200, actual);

    let actual = handle_request(
        &HttpRequest {
            method: "GET".to_string(),
            path: "".to_string(),
            headers: std::collections::HashMap::new(),
            body: vec![],
        },
        &config,
    )
    .unwrap()
    .status_code;
    assert_eq!(200, actual);

    let actual = handle_request(
        &HttpRequest {
            method: "GET".to_string(),
            path: "/something".to_string(),
            headers: std::collections::HashMap::new(),
            body: vec![],
        },
        &config,
    )
    .unwrap()
    .status_code;
    assert_eq!(404, actual);

    let actual = handle_request(
        &HttpRequest {
            method: "GET".to_string(),
            path: "/something/something".to_string(),
            headers: std::collections::HashMap::new(),
            body: vec![],
        },
        &config,
    )
    .unwrap()
    .status_code;
    assert_eq!(404, actual);

    let actual = handle_request(
        &HttpRequest {
            method: "GET".to_string(),
            path: "/echo/something".to_string(),
            headers: std::collections::HashMap::new(),
            body: vec![],
        },
        &config,
    )
    .unwrap()
    .status_code;
    assert_eq!(200, actual);
}
