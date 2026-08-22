//! الطبقة 5: Router.
//! بيربط كل (method, path pattern) بـ handler function.
//! بيدعم path parameters بصيغة ":name" (زي /conversations/:id/messages)
//! عشان نقدر نمسك قيم ديناميكية من الـ URL.

use std::collections::HashMap;
use std::sync::Arc;

use crate::error::AppError;
use crate::http::{Request, Response};
use crate::store::AppState;

/// نوع الـ handler: دالة بتاخد Request + AppState وترجع Response أو AppError.
/// Arc<dyn Fn...> عشان نقدر نخزنها في جدول ونستدعيها من أي thread.
pub type Handler =
    Arc<dyn Fn(&Request, &Arc<AppState>) -> Result<Response, AppError> + Send + Sync>;

#[derive(Clone)]
enum Segment {
    Static(String),
    Param(String),
}

struct Route {
    method: String,
    segments: Vec<Segment>,
    handler: Handler,
}

pub struct Router {
    routes: Vec<Route>,
}

impl Router {
    pub fn new() -> Router {
        Router { routes: Vec::new() }
    }

    /// يسجّل route جديد. pattern بصيغة "/conversations/:id/messages" مثلاً.
    pub fn add<F>(&mut self, method: &str, pattern: &str, handler: F)
    where
        F: Fn(&Request, &Arc<AppState>) -> Result<Response, AppError> + Send + Sync + 'static,
    {
        let segments = pattern
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| {
                if let Some(name) = s.strip_prefix(':') {
                    Segment::Param(name.to_string())
                } else {
                    Segment::Static(s.to_string())
                }
            })
            .collect();

        self.routes.push(Route {
            method: method.to_string(),
            segments,
            handler: Arc::new(handler),
        });
    }

    /// يحاول يطابق (method, path) مع أحد الـ routes المسجلة.
    /// بيرجع الـ handler + path params المستخرجة لو لقى تطابق.
    fn match_route(&self, method: &str, path: &str) -> Option<(Handler, HashMap<String, String>)> {
        let path_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        for route in &self.routes {
            if route.method != method {
                continue;
            }
            if route.segments.len() != path_segments.len() {
                continue;
            }

            let mut params = HashMap::new();
            let mut matched = true;

            for (pattern_seg, actual_seg) in route.segments.iter().zip(path_segments.iter()) {
                match pattern_seg {
                    Segment::Static(s) => {
                        if s != actual_seg {
                            matched = false;
                            break;
                        }
                    }
                    Segment::Param(name) => {
                        params.insert(name.clone(), actual_seg.to_string());
                    }
                }
            }

            if matched {
                return Some((Arc::clone(&route.handler), params));
            }
        }

        None
    }

    /// ينفّذ الـ handler المطابق، أو يرجع AppError مناسب (404 أو 405).
    pub fn dispatch(&self, mut req: Request, state: &Arc<AppState>) -> Result<Response, AppError> {
        // أولاً: هل فيه route بنفس الـ path بغض النظر عن الـ method؟
        // (عشان نفرق بين 404 حقيقي و405 method not allowed)
        let path_exists = self.routes.iter().any(|r| {
            let path_segments: Vec<&str> = req.path.split('/').filter(|s| !s.is_empty()).collect();
            r.segments.len() == path_segments.len()
                && r.segments.iter().zip(path_segments.iter()).all(|(p, a)| match p {
                    Segment::Static(s) => s == a,
                    Segment::Param(_) => true,
                })
        });

        match self.match_route(&req.method, &req.path) {
            Some((handler, params)) => {
                req.params = params;
                handler(&req, state)
            }
            None if path_exists => Err(AppError::BadRequest(format!(
                "الطريقة {} غير مدعومة على هذا المسار",
                req.method
            ))),
            None => Err(AppError::NotFound(format!("المسار غير موجود: {}", req.path))),
        }
    }
}
