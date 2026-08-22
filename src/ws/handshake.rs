//! الطبقة 8: WebSocket Handshake يدوي (RFC 6455).
//!
//! فكرة الـ handshake: العميل بيبعت HTTP request عادي فيه headers خاصة بتقول
//! "عايز أرقّي الاتصال ده من HTTP لـ WebSocket". لو السيرفر موافق، بيرد بـ
//! status code 101 (Switching Protocols) بدل الرد العادي، وبعد الرد ده،
//! نفس اتصال TCP بيفضل مفتوح لكن البيانات اللي بتتبعت عليه بقت "WebSocket frames"
//! مش HTTP requests/responses عادية.
//!
//! العميل بيبعت header اسمه Sec-WebSocket-Key (قيمة عشوائية base64).
//! السيرفر لازم يثبت إنه "فهم" طلب الترقية عن طريق:
//!   1. دمج المفتاح ده مع GUID ثابت معروف عالميًا لبروتوكول WebSocket
//!   2. عمل SHA-1 على الناتج
//!   3. Base64 encode للـ digest
//! ويبعت الناتج في header اسمه Sec-WebSocket-Accept.
//! الـ GUID ده جزء من المواصفة نفسها (RFC 6455 section 1.3) - مش سر، لكنه
//! ثابت لازم يتطابق بالظبط عشان يثبت إن السيرفر فعلاً بروتوكول WebSocket حقيقي.

use crate::crypto;
use crate::error::AppError;
use crate::http::Request;

const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// يتحقق هل الـ request ده طلب ترقية لـ WebSocket ولا لأ، عن طريق فحص
/// headers: Upgrade: websocket و Connection: Upgrade (حسب المواصفة).
pub fn is_websocket_upgrade(req: &Request) -> bool {
    let upgrade_header = req
        .header("upgrade")
        .map(|v| v.to_lowercase())
        .unwrap_or_default();
    let connection_header = req
        .header("connection")
        .map(|v| v.to_lowercase())
        .unwrap_or_default();

    upgrade_header.contains("websocket") && connection_header.contains("upgrade")
}

/// يبني استجابة الـ 101 Switching Protocols كبايتات خام جاهزة للإرسال مباشرة.
pub fn build_handshake_response(req: &Request) -> Result<Vec<u8>, AppError> {
    let key = req
        .header("sec-websocket-key")
        .ok_or_else(|| AppError::BadRequest("مفقود: Sec-WebSocket-Key".to_string()))?;

    let concatenated = format!("{key}{WEBSOCKET_GUID}");
    let digest = crypto::sha1(concatenated.as_bytes());
    let accept_value = crypto::base64_encode(&digest);

    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {accept_value}\r\n\
         \r\n"
    );

    Ok(response.into_bytes())
}
