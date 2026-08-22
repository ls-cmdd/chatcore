//! الطبقة 1 + نقطة التجميع: TCP Listener، واستقبال كل اتصال وتوجيهه
//! إما لمسار HTTP عادي (parse -> router -> response) أو لمسار WebSocket
//! (handshake -> اتصال مستمر لحد ما يتقفل).

use std::io::{BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Instant;

use crate::http;
use crate::middleware;
use crate::router::Router;
use crate::store::AppState;
use crate::thread_pool::ThreadPool;
use crate::ws;

pub fn run(address: &str, worker_count: usize, router: Router, state: Arc<AppState>) {
    let listener = TcpListener::bind(address)
        .unwrap_or_else(|e| panic!("فشل في فتح العنوان {address}: {e}"));

    println!("🚀 السيرفر شغال على http://{address}");
    println!("   WebSocket متاح على ws://{address}/ws?token=<token>");
    println!("   عدد الـ workers في الـ thread pool: {worker_count}");

    let pool = ThreadPool::new(worker_count);
    let router = Arc::new(router);

    // حلقة القبول الرئيسية: تسلسلية عمدًا (thread واحد بيستقبل الاتصالات)،
    // لكن معالجة كل اتصال بتتوزع على الـ thread pool فورًا - القبول نفسه
    // عملية خفيفة جدًا (accept syscall) فمش محتاجة توازي.
    for incoming in listener.incoming() {
        match incoming {
            Ok(stream) => {
                let router = Arc::clone(&router);
                let state = Arc::clone(&state);
                pool.execute(move || {
                    handle_connection(stream, &router, &state);
                });
            }
            Err(e) => eprintln!("⚠️  فشل في قبول اتصال: {e}"),
        }
    }
}

fn handle_connection(stream: TcpStream, router: &Router, state: &Arc<AppState>) {
    // مرحلة الـ HTTP parsing: بنستخدم BufReader<&TcpStream> مؤقتًا (borrow فقط)
    // عشان لو الاتصال اتحول لـ WebSocket بعد كده، نقدر ناخد ownership كامل
    // لـ `stream` من غير مشاكل lifetime.
    let parsed = {
        let mut reader = BufReader::new(&stream);
        http::parser::parse_request(&mut reader)
    };

    let req = match parsed {
        Ok(r) => r,
        Err(e) => {
            let _ = (&stream).write_all(&e.to_response().to_bytes());
            return;
        }
    };

    middleware::log_request(&req);
    let start = Instant::now();

    // --- مسار WebSocket ---
    if ws::handshake::is_websocket_upgrade(&req) {
        match ws::handshake::build_handshake_response(&req) {
            Ok(handshake_bytes) => {
                if (&stream).write_all(&handshake_bytes).is_err() {
                    return;
                }
                middleware::log_response(&req.method, &req.path, 101, start.elapsed().as_millis());
                // من هنا الاتصال ملك handle_ws_connection بالكامل (owned TcpStream)
                ws::connection::handle_ws_connection(stream, req, Arc::clone(state));
            }
            Err(e) => {
                let _ = (&stream).write_all(&e.to_response().to_bytes());
            }
        }
        return;
    }

    // --- مسار HTTP عادي ---
    let method = req.method.clone();
    let path = req.path.clone();

    let response = router
        .dispatch(req, state)
        .unwrap_or_else(|e| e.to_response());

    let status = response.status;
    let _ = (&stream).write_all(&response.to_bytes());
    middleware::log_response(&method, &path, status, start.elapsed().as_millis());
}
