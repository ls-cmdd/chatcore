//! الطبقة 11: Auth بسيط.
//! تسجيل/دخول المستخدمين وتوليد tokens بيحصل في store.rs (منطق البيانات)
//! وhandlers.rs (منطق الـ endpoints). هنا بس دوال مساعدة لاستخراج
//! والتحقق من التوكن من الـ request (سواء HTTP عادي أو WebSocket).

use std::sync::Arc;

use crate::error::AppError;
use crate::http::Request;
use crate::store::AppState;

/// يستخرج التوكن من header بصيغة: Authorization: Bearer <token>
fn extract_bearer_token(req: &Request) -> Option<String> {
    let header = req.header("authorization")?;
    header.strip_prefix("Bearer ").map(|t| t.trim().to_string())
}

/// يتحقق من التوكن الموجود في Authorization header ويرجع user_id المرتبط بيه.
/// بيُستخدم في كل endpoint محتاج تسجيل دخول (زي إنشاء محادثة أو إرسال رسالة).
pub fn require_auth(req: &Request, state: &Arc<AppState>) -> Result<String, AppError> {
    let token = extract_bearer_token(req)
        .ok_or_else(|| AppError::Unauthorized("مطلوب تسجيل دخول (Authorization header)".into()))?;

    state
        .user_id_for_token(&token)
        .ok_or_else(|| AppError::Unauthorized("التوكن غير صحيح أو منتهي".into()))
}

/// نفس التحقق لكن للـ WebSocket، حيث التوكن بيتبعت كـ query parameter
/// (زي ws://host/ws?token=xxx) لأن اتصال WebSocket الأولي مش بيدعم
/// custom headers بسهولة من كل الـ clients (خصوصًا JS في المتصفح).
pub fn require_auth_from_query(req: &Request, state: &Arc<AppState>) -> Result<String, AppError> {
    let token = req
        .query
        .get("token")
        .ok_or_else(|| AppError::Unauthorized("مطلوب ?token= في رابط WebSocket".into()))?;

    state
        .user_id_for_token(token)
        .ok_or_else(|| AppError::Unauthorized("التوكن غير صحيح أو منتهي".into()))
}
