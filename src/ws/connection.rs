//! يدير دورة حياة اتصال WebSocket واحد بعد ما الـ handshake يخلص بنجاح.
//!
//! التصميم: كل اتصال WebSocket بيستخدم اتنين thread:
//!   - Reader thread (الحالي، جوه handle_ws_connection نفسها): بيقرا frames
//!     جايه من العميل (ping/close بشكل أساسي، لأن منطق الشات هنا HTTP-driven
//!     مش WS-driven - العميل بيبعت الرسائل عبر POST /conversations/:id/messages
//!     والسيرفر بيـ push الرسائل الجديدة عبر WS).
//!   - Writer thread: بيستنى على mpsc channel، وأي bytes توصله بيكتبها فورًا
//!     على الـ socket. الفصل ده ضروري عشان broadcast_message (من thread تاني
//!     خالص، وقت ما حد يبعت رسالة عبر HTTP) يقدر "يدفع" بيانات لاتصال WS
//!     من غير ما يحتاج قفل مباشر على الـ TcpStream نفسه.

use std::io::{BufReader, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use crate::auth;
use crate::http::Request;
use crate::store::AppState;
use crate::ws::frame::{self, OPCODE_CLOSE, OPCODE_PING};

pub fn handle_ws_connection(stream: TcpStream, req: Request, state: Arc<AppState>) {
    // المصادقة بتحصل بعد الـ handshake (الـ handshake نفسه بيتم بغض النظر عن
    // هوية المستخدم، لأنه جزء من بروتوكول النقل مش من منطق التطبيق).
    // لو التوكن غير صحيح، بنقفل الاتصال فورًا بعد إرسال close frame.
    let user_id = match auth::require_auth_from_query(&req, &state) {
        Ok(id) => id,
        Err(_) => {
            let mut stream = stream;
            let _ = stream.write_all(&frame::encode_close_frame());
            return;
        }
    };

    println!("WebSocket متصل: user_id={user_id}");

    let write_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };

    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let conn_id = state.register_ws_sender(&user_id, tx.clone());

    // Writer thread: مسؤول بس عن كتابة أي bytes توصله عبر القناة على الـ socket.
    let writer_handle = thread::spawn(move || {
        let mut write_stream = write_stream;
        while let Ok(bytes) = rx.recv() {
            if write_stream.write_all(&bytes).is_err() {
                break;
            }
        }
    });

    // Reader loop: بيفضل شغال لحد ما العميل يقفل الاتصال أو يبعت close frame.
    let mut reader = BufReader::new(&stream);
    loop {
        match frame::read_frame(&mut reader) {
            Ok(Some(f)) => match f.opcode {
                OPCODE_CLOSE => {
                    let _ = tx.send(frame::encode_close_frame());
                    break;
                }
                OPCODE_PING => {
                    // رد Pong إجباري حسب المواصفة عشان نثبت إن الاتصال لسه حي
                    let _ = tx.send(frame::encode_frame(frame::OPCODE_PONG, &f.payload));
                }
                _ => {
                    // العميل ممكن يبعت frames (مثلاً heartbeat خاص به) - بنتجاهلها
                    // حاليًا لأن منطق إرسال الرسائل بيمر عبر HTTP مش WS في هذا المشروع.
                }
            },
            Ok(None) => break, // العميل قفل الاتصال (TCP FIN)
            Err(_) => break,   // خطأ قراءة (اتصال اتقطع فجأة، إلخ)
        }
    }

    // تنظيف: نشيل الاتصال من الـ registry ونسيب الـ writer thread يخرج
    // (بعد ما آخر Sender لهذه القناة يتقفل، rx.recv() هيرجع Err فيخرج الـ loop).
    state.unregister_ws_sender(&user_id, conn_id);
    drop(tx);
    let _ = writer_handle.join();

    println!("WebSocket اتقفل: user_id={user_id}");
}
