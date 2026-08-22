//! الطبقة 2: HTTP Parser يدوي بالكامل.
//! بياخد أي قارئ (BufRead) بيمثل اتصال TCP، ويفكك النص الخام حسب صيغة HTTP/1.1:
//!   1. Request line: "METHOD /path?query HTTP/1.1"
//!   2. Headers: سطر لكل header بصيغة "Key: Value" لحد سطر فاضي
//!   3. Body: بالظبط Content-Length بايت (لو موجود)
//! مفيش أي مكتبة HTTP هنا - كله قراءة نص وتقطيع يدوي.

use std::collections::HashMap;
use std::io::BufRead;

use crate::error::AppError;
use crate::http::request::Request;
use crate::util::split_path_query;

pub fn parse_request<R: BufRead>(reader: &mut R) -> Result<Request, AppError> {
    // --- 1. Request line ---
    let mut request_line = String::new();
    let bytes_read = reader
        .read_line(&mut request_line)
        .map_err(|e| AppError::BadRequest(format!("فشل في قراءة سطر الطلب: {e}")))?;

    if bytes_read == 0 {
        // العميل قفل الاتصال من غير ما يبعت حاجة
        return Err(AppError::BadRequest("اتصال فارغ".to_string()));
    }

    let request_line = request_line.trim_end();
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| AppError::BadRequest("سطر الطلب ناقص method".to_string()))?
        .to_string();
    let full_path = parts
        .next()
        .ok_or_else(|| AppError::BadRequest("سطر الطلب ناقص path".to_string()))?
        .to_string();
    // الجزء الثالث (HTTP version) بنقرأه بس مش بنتحقق منه بصرامة في هذه المرحلة
    let _version = parts.next().unwrap_or("HTTP/1.1");

    let (path, query) = split_path_query(&full_path);

    // --- 2. Headers ---
    let mut headers: HashMap<String, String> = HashMap::new();
    loop {
        let mut line = String::new();
        let n = reader
            .read_line(&mut line)
            .map_err(|e| AppError::BadRequest(format!("فشل في قراءة headers: {e}")))?;
        if n == 0 {
            break; // الاتصال اتقفل قبل ما نلاقي سطر فاضي
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break; // السطر الفاضي بيفصل الـ headers عن الـ body
        }
        if let Some((key, value)) = line.split_once(':') {
            headers.insert(key.trim().to_lowercase(), value.trim().to_string());
        }
        // أي سطر header مش بالصيغة الصحيحة بيتجاهل بدل ما يوقف السيرفر كله
    }

    // --- 3. Body (حسب Content-Length) ---
    let content_length: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    // حد أقصى بسيط للحماية من body ضخم يستهلك الذاكرة (10 ميجا)
    const MAX_BODY_SIZE: usize = 10 * 1024 * 1024;
    if content_length > MAX_BODY_SIZE {
        return Err(AppError::BadRequest("جسم الطلب أكبر من الحد المسموح".to_string()));
    }

    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        std::io::Read::read_exact(reader, &mut body)
            .map_err(|e| AppError::BadRequest(format!("فشل في قراءة جسم الطلب: {e}")))?;
    }

    Ok(Request {
        method,
        path,
        query,
        headers,
        body,
        params: HashMap::new(),
    })
}
