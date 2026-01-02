use std::collections::HashMap;
use std::env;
use std::io::{BufRead, Read, Write};
use std::ops::ControlFlow;

use anyhow::{Context, Error, Result};
use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl HttpRequest {
    fn from_bytes(bytes: BytesMut) -> Result<HttpRequest, Error> {
        let lines: Vec<String> = bytes
            .lines()
            .map(|line| line.context("Invalid request"))
            .collect::<Result<Vec<_>, _>>()?; // convert to Result<Vec<String>, Error>  due to maping lines and unwrapping the it

        let request_line = lines.get(0).context("No request line")?;
        let request_line_parts: Vec<&str> = request_line.split_whitespace().collect();
        if request_line_parts.len() != 3 {
            anyhow::bail!(
                "invalid request line: expected 3 parts, got {}",
                request_line_parts.len()
            );
        }
        let mut request_headers = HashMap::new();
        if let Some(pos) = lines.iter().position(|x| x == "") {
            let recieved_headers = &lines[1..pos];
            let _body = &lines[pos + 1..];
            for header in recieved_headers {
                let parts: Vec<&str> = header.split(": ").collect();
                if parts.len() != 2 {
                    anyhow::bail!("invalid header: expected 2 parts, got {}", parts.len());
                }
                request_headers.insert(parts[0].to_string(), parts[1].to_string());
            }
        } else {
            return Err(anyhow::anyhow!("No request body found"));
        }

        Ok(HttpRequest {
            method: request_line_parts[0].to_string(),
            path: request_line_parts[1].to_string(),
            headers: request_headers,
            body: vec![],
        })
    }
}

struct HttpResponse {
    status_code: u16,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}
impl HttpResponse {
    pub fn new(status_code: u16) -> Self {
        HttpResponse {
            status_code,
            headers: HashMap::new(),
            body: vec![],
        }
    }

    pub fn not_found() -> Self {
        HttpResponse::new(404)
    }
    pub fn ok() -> Self {
        HttpResponse::new(200)
    }
    pub fn internal_server_error() -> Self {
        HttpResponse::new(500)
    }

    fn set_header(&mut self, header: String, value: String) {
        self.headers.insert(header, value);
    }

    fn set_body(&mut self, body: Vec<u8>) {
        self.body = body;
    }

    fn reason(&self) -> String {
        match self.status_code {
            200 => "OK".to_string(),
            201 => "Created".to_string(),
            404 => "Not Found".to_string(),
            500 => "Internal Server Error".to_string(),
            _ => "Unknown".to_string(),
        }
    }

    fn encode(&self) -> Vec<u8> {
        let mut response =
            format!("HTTP/1.1 {} {}\r\n", self.status_code, self.reason()).into_bytes();
        for (header, value) in &self.headers {
            response.extend(format!("{}: {}\r\n", header, value).into_bytes());
        }
        response.extend(b"\r\n");
        response.extend(&self.body);
        response
    }
}

#[derive(Debug, Clone)]
struct ServerConfig {
    static_directory: Option<String>,
}

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
            let abs_directory = args.nth(0).context("unable to parse absolute directory")?;
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
    let mut input = BytesMut::zeroed(1024);

    let _ = stream
        .read_buf(&mut input)
        .await
        .context("Failed to read")?;
    let request = HttpRequest::from_bytes(input)?;
    let response = handle_request(request, &config);
    let result = match response {
        Ok(resp) => resp,
        Err(_) => HttpResponse::internal_server_error(),
    };

    let _res = stream
        .write(result.encode().as_slice())
        .await
        .context("Unable to write")?;
    Ok(())
}

fn handle_request(request: HttpRequest, config: &ServerConfig) -> Result<HttpResponse> {
    let segments = request
        .path
        .split("/")
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<&str>>();

    println!("Path Segments: {:?}", segments);

    if let Some(first_segment) = segments.first() {
        let resp = match *first_segment {
            "files" => {
                let file_path = segments.get(1).unwrap_or(&"");
                let Some(root_dir) = &config.static_directory else {
                    return Ok(HttpResponse::not_found());
                };

                let file_path = format!("{}/{}", root_dir, file_path);

                if let Ok(metadata) = std::fs::metadata(&file_path) {
                    if metadata.is_file() {
                        let mut resp = HttpResponse::ok();
                        resp.set_header(
                            "Content-Type".to_string(),
                            "application/octet-stream".to_string(),
                        );
                        resp.set_header("Content-Length".to_string(), metadata.len().to_string());
                        resp.set_body(std::fs::read(file_path).unwrap().into());
                        resp
                    } else {
                        HttpResponse::not_found()
                    }
                } else {
                    HttpResponse::not_found()
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
        return Ok(resp);
    } else {
        return Ok(HttpResponse::ok());
    }
}

#[test]
fn tests_handle_request() {
    let actual = handle_request(HttpRequest {
        body: vec![],
        path: "/".to_string(),
        method: "GET".to_string(),
        headers: HashMap::new(),
    })
    .unwrap()
    .status_code;
    assert_eq!(200, actual);

    let actual = handle_request(HttpRequest {
        method: "GET".to_string(),
        path: "".to_string(),
        headers: HashMap::new(),
        body: vec![],
    })
    .unwrap()
    .status_code;
    assert_eq!(200, actual);

    let actual = handle_request(HttpRequest {
        method: "GET".to_string(),
        path: "/something".to_string(),
        headers: HashMap::new(),
        body: vec![],
    })
    .unwrap()
    .status_code;
    assert_eq!(404, actual);

    let actual = handle_request(HttpRequest {
        method: "GET".to_string(),
        path: "/something/something".to_string(),
        headers: HashMap::new(),
        body: vec![],
    })
    .unwrap()
    .status_code;
    assert_eq!(404, actual);

    let actual = handle_request(HttpRequest {
        method: "GET".to_string(),
        path: "/echo/something".to_string(),
        headers: HashMap::new(),
        body: vec![],
    })
    .unwrap()
    .status_code;
    assert_eq!(200, actual);
}
