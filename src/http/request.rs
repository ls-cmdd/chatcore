use std::collections::HashMap;

/// تمثيل نظيف لـ HTTP request بعد تفكيكه من البايتات الخام (الطبقة 3).
#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub path: String,
    /// query parameters من الـ URL (بعد '?')
    pub query: HashMap<String, String>,
    /// headers بمفاتيح lowercase دايمًا (HTTP headers case-insensitive حسب المواصفة)
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    /// path parameters اللي بيملاها الـ Router بعد الـ matching (زي :id)
    pub params: HashMap<String, String>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(&name.to_lowercase()).map(|s| s.as_str())
    }

    pub fn param(&self, name: &str) -> Option<&str> {
        self.params.get(name).map(|s| s.as_str())
    }

    pub fn body_as_str(&self) -> String {
        String::from_utf8_lossy(&self.body).to_string()
    }

    /// يحاول يفسّر الـ body كـ JSON. يرجع AppError واضح لو فشل التحليل.
    pub fn json_body(&self) -> Result<crate::json::Value, crate::error::AppError> {
        let text = self.body_as_str();
        crate::json::parse(&text)
            .map_err(|e| crate::error::AppError::BadRequest(format!("JSON غير صحيح: {e}")))
    }
}
