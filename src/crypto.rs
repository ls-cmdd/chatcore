//! تطبيق يدوي لـ SHA-1 وBase64 encoding.
//!
//! ليه هنا SHA-1 تحديدًا؟ بروتوكول WebSocket (RFC 6455) بينص إن الـ handshake
//! لازم يستخدم SHA-1 لحساب Sec-WebSocket-Accept من Sec-WebSocket-Key. مفيش بديل -
//! ده جزء من مواصفة البروتوكول نفسه، فمكتوب هنا يدويًا بدل استخدام crate خارجي.
//!
//! ملاحظة أمان: التوليد العشوائي هنا (random_u64/generate_token) بسيط ومبني على
//! الوقت + عداد ذري، وهو *غير* مناسب أمنيًا لمشروع إنتاج حقيقي (لازم CSPRNG حقيقي).
//! هنا كافٍ لمشروع تعليمي in-memory بدون بيانات حساسة فعلية.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// تطبيق خوارزمية SHA-1 القياسية (FIPS 180-1) على مصفوفة بايتات، وإرجاع digest بحجم 20 بايت.
pub fn sha1(message: &[u8]) -> [u8; 20] {
    let mut h0: u32 = 0x67452301;
    let mut h1: u32 = 0xEFCDAB89;
    let mut h2: u32 = 0x98BADCFE;
    let mut h3: u32 = 0x10325476;
    let mut h4: u32 = 0xC3D2E1F0;

    // Padding: نضيف بايت 0x80، بعدين أصفار لحد ما الطول يبقى 56 mod 64،
    // وبعدين طول الرسالة الأصلي بالبت في آخر 8 بايت (big-endian).
    let bit_len: u64 = (message.len() as u64) * 8;
    let mut msg = message.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;

        for (i, &wi) in w.iter().enumerate() {
            let (f, k) = if i < 20 {
                ((b & c) | ((!b) & d), 0x5A827999u32)
            } else if i < 40 {
                (b ^ c ^ d, 0x6ED9EBA1u32)
            } else if i < 60 {
                ((b & c) | (b & d) | (c & d), 0x8F1BBCDCu32)
            } else {
                (b ^ c ^ d, 0xCA62C1D6u32)
            };

            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut out = [0u8; 20];
    out[0..4].copy_from_slice(&h0.to_be_bytes());
    out[4..8].copy_from_slice(&h1.to_be_bytes());
    out[8..12].copy_from_slice(&h2.to_be_bytes());
    out[12..16].copy_from_slice(&h3.to_be_bytes());
    out[16..20].copy_from_slice(&h4.to_be_bytes());
    out
}

const BASE64_TABLE: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Base64 encoding قياسي (مع padding بـ '=').
pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        let triple = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);

        out.push(BASE64_TABLE[((triple >> 18) & 0x3F) as usize] as char);
        out.push(BASE64_TABLE[((triple >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 {
            BASE64_TABLE[((triple >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            BASE64_TABLE[(triple & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// مولد أرقام شبه عشوائي بسيط: nanoseconds من الساعة مخلوط بعداد ذري.
/// كافٍ لتوليد IDs وtokens فريدة في سياق تعليمي، وليس آمنًا تشفيريًا.
fn random_u64() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    nanos ^ c.wrapping_mul(0x9E3779B97F4A7C15)
}

/// يولّد معرّف فريد (يُستخدم كـ user_id / conversation_id / message_id).
pub fn generate_id() -> String {
    format!("{:016x}", random_u64())
}

/// يولّد token مصادقة (auth token) طويل بما يكفي لتقليل احتمالية التخمين.
pub fn generate_token() -> String {
    format!("{:016x}{:016x}", random_u64(), random_u64())
}

/// Hash بسيط لكلمة المرور: SHA-1(salt + password) + تخزين الـ salt معه.
/// ملاحظة صريحة: SHA-1 وحده *ليس* مناسبًا لتخزين كلمات مرور في نظام إنتاج حقيقي
/// (يُفضّل bcrypt/argon2 مع cost factor عالي). هنا الهدف تعليمي: تجنب تخزين
/// كلمة المرور كنص صريح، مش بناء نظام hashing مقاوم لهجمات production-grade.
pub fn hash_password(password: &str, salt: &str) -> String {
    let combined = format!("{salt}:{password}");
    let digest = sha1(combined.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}
