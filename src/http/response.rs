use crate::json;

/// تمثيل نظيف لـ HTTP response قبل تحويله لبايتات خام تتبعت عبر TCP.
#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn new(status: u16, body: Vec<u8>) -> Response {
        Response {
            status,
            headers: Vec::new(),
            body,
        }
    }

    pub fn with_header(mut self, key: &str, value: &str) -> Response {
        self.headers.push((key.to_string(), value.to_string()));
        self
    }

    pub fn text(status: u16, body: &str) -> Response {
        Response::new(status, body.as_bytes().to_vec())
            .with_header("Content-Type", "text/plain; charset=utf-8")
    }

    pub fn json(status: u16, value: json::Value) -> Response {
        let body = value.to_string();
        Response::new(status, body.into_bytes())
            .with_header("Content-Type", "application/json; charset=utf-8")
    }

    fn status_text(status: u16) -> &'static str {
        match status {
            200 => "OK",
            201 => "Created",
            204 => "No Content",
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            404 => "Not Found",
            405 => "Method Not Allowed",
            500 => "Internal Server Error",
            _ => "Unknown",
        }
    }

    /// يحوّل الـ Response لبايتات خام جاهزة تُكتب مباشرة على TcpStream،
    /// بصيغة HTTP/1.1 صحيحة (status line + headers + سطر فاضي + body).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = format!(
            "HTTP/1.1 {} {}\r\n",
            self.status,
            Response::status_text(self.status)
        );
        out.push_str(&format!("Content-Length: {}\r\n", self.body.len()));
        for (k, v) in &self.headers {
            out.push_str(&format!("{k}: {v}\r\n"));
        }
        // كل الاتصالات هنا close-per-request للتبسيط (بدون keep-alive) —
        // قرار مقصود لتفادي تعقيد إدارة connection pooling في هذه المرحلة.
        out.push_str("Connection: close\r\n");
        out.push_str("\r\n");

        let mut bytes = out.into_bytes();
        bytes.extend_from_slice(&self.body);
        bytes
    }
}
