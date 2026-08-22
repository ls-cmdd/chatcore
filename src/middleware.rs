//! الطبقة 6: Middleware.
//! هنا middleware بسيط للـ logging بيتنفذ حوالين كل request (قبل وبعد التنفيذ).
//! التصميم مبسّط عمدًا (دالتين تُستدعيان في server.rs) بدل نظام middleware chain
//! عام بالكامل بـ generics معقدة — القرار ده موضّح في الشرح المرفق مع هذه الطبقة.

use crate::http::Request;

pub fn log_request(req: &Request) {
    println!("→ {} {}", req.method, req.path);
}

pub fn log_response(method: &str, path: &str, status: u16, elapsed_ms: u128) {
    println!("← {method} {path} [{status}] ({elapsed_ms}ms)");
}
