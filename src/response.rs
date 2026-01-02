use std::collections::HashMap;

#[derive(Debug)]
pub struct HttpResponse {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
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
    pub fn created() -> Self {
        HttpResponse::new(201)
    }
    pub fn internal_server_error() -> Self {
        HttpResponse::new(500)
    }

    pub fn set_header(&mut self, header: String, value: String) {
        self.headers.insert(header, value);
    }

    pub fn set_body(&mut self, body: Vec<u8>) {
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

    pub fn encode(&self) -> Vec<u8> {
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
