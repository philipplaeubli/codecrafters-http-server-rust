use std::collections::HashMap;

use anyhow::{Context, Error};
use bytes::BytesMut;

pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    pub fn from_bytes(bytes: BytesMut) -> Result<HttpRequest, Error> {
        let header_end = bytes
            .windows(4)
            .position(|word| word == b"\r\n\r\n")
            .context("Unable to find header/body seperator")?;

        let header_data = &bytes[..header_end];
        let body_data = &bytes[header_end + 4..];

        let header_str = std::str::from_utf8(header_data).context("unable to parse header")?;

        let mut lines = header_str.lines();

        let request_line = lines.next().context("No request line")?;
        let request_line_parts: Vec<&str> = request_line.split_whitespace().collect();
        if request_line_parts.len() != 3 {
            anyhow::bail!(
                "invalid request line: expected 3 parts, got {}",
                request_line_parts.len()
            );
        }
        let mut request_headers = HashMap::new();
        for header in lines {
            if header.is_empty() {
                break;
            }
            let parts: Vec<&str> = header.split(": ").collect();
            if parts.len() != 2 {
                anyhow::bail!("invalid header: expected 2 parts, got {}", parts.len());
            }
            request_headers.insert(parts[0].to_string(), parts[1].to_string());
        }
        let content_length: usize = request_headers
            .get("Content-Length")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let body = body_data[..content_length.min(body_data.len())].to_vec();

        Ok(HttpRequest {
            method: request_line_parts[0].to_string(),
            path: request_line_parts[1].to_string(),
            headers: request_headers,
            body,
        })
    }
}
