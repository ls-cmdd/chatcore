//! الطبقة 9: WebSocket Frame Parsing (RFC 6455 section 5).
//!
//! بعد الـ handshake، أي بيانات بتتبعت على الاتصال بتكون متغلفة في "frames"
//! بصيغة ثابتة (مش نص خام زي HTTP). شكل الـ frame تقريبًا:
//!
//!  byte 0: FIN(1 bit) + RSV(3 bits) + Opcode(4 bits)
//!  byte 1: MASK(1 bit) + Payload length(7 bits)
//!  [بايتات إضافية للطول لو كان أكبر من 125]
//!  [4 بايت mask key لو MASK=1]
//!  [الـ payload نفسه، لو كان masked بيتفك بالـ XOR مع الـ mask key]
//!
//! قاعدة مهمة من المواصفة: أي frame من العميل للسيرفر *لازم* يكون masked،
//! وأي frame من السيرفر للعميل *لازم* يكون غير masked. ده إجراء أمني
//! (منع cache poisoning attacks على بروكسيات معينة) - مش اختياري.
//!
//! Opcodes المستخدمة هنا: 0x1 (نص), 0x8 (إغلاق), 0x9 (ping), 0xA (pong).
//! ملاحظة: التطبيق ده بيتعامل مع كل frame كرسالة كاملة (FIN=1) ومش بيدعم
//! تجميع الرسائل المجزّأة (fragmentation) - قرار مبسط مقبول لمشروع تعليمي،
//! لأن معظم عملاء WebSocket العادية (متصفحات، أدوات اختبار) بترسل frame واحد
//! كامل للرسائل الصغيرة زي رسائل الشات النصية.

use std::io::{self, Read};

pub const OPCODE_TEXT: u8 = 0x1;
#[allow(dead_code)]
pub const OPCODE_BINARY: u8 = 0x2;
pub const OPCODE_CLOSE: u8 = 0x8;
pub const OPCODE_PING: u8 = 0x9;
pub const OPCODE_PONG: u8 = 0xA;

#[derive(Debug)]
pub struct Frame {
    pub opcode: u8,
    pub payload: Vec<u8>,
}

/// يقرأ frame واحد من أي مصدر بيدعم Read (زي TcpStream).
/// يرجع Ok(None) لو الاتصال اتقفل بشكل نظيف (EOF قبل أي بايت جديد).
pub fn read_frame<R: Read>(reader: &mut R) -> io::Result<Option<Frame>> {
    let mut header = [0u8; 2];
    if let Err(e) = reader.read_exact(&mut header) {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            return Ok(None);
        }
        return Err(e);
    }

    let opcode = header[0] & 0x0F;
    let masked = header[1] & 0x80 != 0;
    let mut payload_len = (header[1] & 0x7F) as u64;

    if payload_len == 126 {
        let mut ext = [0u8; 2];
        reader.read_exact(&mut ext)?;
        payload_len = u16::from_be_bytes(ext) as u64;
    } else if payload_len == 127 {
        let mut ext = [0u8; 8];
        reader.read_exact(&mut ext)?;
        payload_len = u64::from_be_bytes(ext);
    }

    // حد أقصى بسيط للحماية من رسائل ضخمة تستهلك الذاكرة (1 ميجا كافي لرسائل شات نصية)
    const MAX_FRAME_SIZE: u64 = 1024 * 1024;
    if payload_len > MAX_FRAME_SIZE {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame أكبر من الحد المسموح"));
    }

    let mut mask_key = [0u8; 4];
    if masked {
        reader.read_exact(&mut mask_key)?;
    }

    let mut payload = vec![0u8; payload_len as usize];
    reader.read_exact(&mut payload)?;

    if masked {
        for (i, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask_key[i % 4];
        }
    }

    Ok(Some(Frame { opcode, payload }))
}

/// يبني frame جاهز للإرسال من السيرفر للعميل (بدون masking - ممنوع للسيرفر حسب المواصفة).
pub fn encode_frame(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 10);

    // FIN=1 (مش بندعم تجزئة الرسائل من السيرفر) + opcode
    out.push(0x80 | (opcode & 0x0F));

    let len = payload.len();
    if len < 126 {
        out.push(len as u8);
    } else if len <= 0xFFFF {
        out.push(126);
        out.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        out.push(127);
        out.extend_from_slice(&(len as u64).to_be_bytes());
    }

    out.extend_from_slice(payload);
    out
}

pub fn encode_text_frame(text: &str) -> Vec<u8> {
    encode_frame(OPCODE_TEXT, text.as_bytes())
}

pub fn encode_close_frame() -> Vec<u8> {
    encode_frame(OPCODE_CLOSE, &[])
}
