//! الطبقة 7: Error handling موحّد.
//! كل خطأ ممكن يحصل في أي مكان في التطبيق (parsing، auth، منطق أعمال) بيتحول
//! لـ AppError واحد، وده بدوره بيتحول بشكل موحّد لاستجابة HTTP واضحة للعميل.
//! الفايدة: أي handler ممكن يستخدم `?` عادي، والتحويل النهائي لاستجابة بيحصل
//! في مكان واحد بس (middleware / server.rs).

use crate::http::response::Response;
use crate::json;

#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    Internal(String),
}

impl AppError {
    pub fn status_code(&self) -> u16 {
        match self {
            AppError::BadRequest(_) => 400,
            AppError::Unauthorized(_) => 401,
            AppError::Forbidden(_) => 403,
            AppError::NotFound(_) => 404,
            AppError::Internal(_) => 500,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            AppError::BadRequest(m)
            | AppError::Unauthorized(m)
            | AppError::Forbidden(m)
            | AppError::NotFound(m)
            | AppError::Internal(m) => m,
        }
    }

    /// يحوّل الخطأ لاستجابة HTTP بصيغة JSON موحّدة: {"error": "..."}
    pub fn to_response(&self) -> Response {
        let body = json::Value::obj(vec![("error".into(), json::Value::str(self.message()))]);
        Response::json(self.status_code(), body)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.status_code(), self.message())
    }
}

impl std::error::Error for AppError {}
