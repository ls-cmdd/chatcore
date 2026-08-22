mod auth;
mod crypto;
mod error;
mod handlers;
mod http;
mod json;
mod middleware;
mod router;
mod server;
mod store;
mod thread_pool;
mod util;
mod ws;

use std::sync::Arc;

use router::Router;
use store::AppState;

fn main() {
    let state = Arc::new(AppState::new());
    let mut router = Router::new();

    // --- تسجيل كل الـ routes (الطبقة 5 + 12) ---
    router.add("GET", "/health", handlers::health);

    router.add("POST", "/register", handlers::register);
    router.add("POST", "/login", handlers::login);

    router.add("POST", "/conversations", handlers::create_conversation);
    router.add("GET", "/conversations", handlers::list_conversations);

    router.add("POST", "/conversations/:id/messages", handlers::post_message);
    router.add("GET", "/conversations/:id/messages", handlers::get_messages);

    // ملاحظة: /ws مش مسجل هنا كـ route عادي - بيتم اعتراضه في server.rs
    // قبل ما يوصل للـ router، لأنه بيتطلب معاملة خاصة (upgrade من HTTP لـ WS).

    const ADDRESS: &str = "127.0.0.1:8080";
    const WORKER_COUNT: usize = 8;

    server::run(ADDRESS, WORKER_COUNT, router, state);
}
