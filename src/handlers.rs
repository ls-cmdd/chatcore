//! الطبقة 12: منطق أعمال الشات (Business Logic).
//! كل دالة هنا handler كامل لـ endpoint واحد. الشكل موحّد:
//! fn(&Request, &Arc<AppState>) -> Result<Response, AppError>
//! عشان تتسجل مباشرة في الـ Router.

use std::sync::Arc;

use crate::auth;
use crate::error::AppError;
use crate::http::{Request, Response};
use crate::json::Value;
use crate::store::AppState;

/// POST /register
/// body: {"username": "...", "password": "..."}
pub fn register(req: &Request, state: &Arc<AppState>) -> Result<Response, AppError> {
    let body = req.json_body()?;

    let username = body
        .get("username")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("مطلوب حقل username".into()))?;
    let password = body
        .get("password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("مطلوب حقل password".into()))?;

    if username.trim().is_empty() || password.len() < 4 {
        return Err(AppError::BadRequest(
            "username لا يجب أن يكون فارغًا، وpassword لازم 4 أحرف على الأقل".into(),
        ));
    }

    let user = state
        .create_user(username, password)
        .map_err(AppError::BadRequest)?;

    let token = state.create_token(&user.id);

    Ok(Response::json(
        201,
        Value::obj(vec![
            ("user_id".into(), Value::str(&user.id)),
            ("username".into(), Value::str(&user.username)),
            ("token".into(), Value::str(&token)),
        ]),
    ))
}

/// POST /login
/// body: {"username": "...", "password": "..."}
pub fn login(req: &Request, state: &Arc<AppState>) -> Result<Response, AppError> {
    let body = req.json_body()?;

    let username = body
        .get("username")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("مطلوب حقل username".into()))?;
    let password = body
        .get("password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("مطلوب حقل password".into()))?;

    let user = state
        .verify_login(username, password)
        .ok_or_else(|| AppError::Unauthorized("اسم المستخدم أو كلمة المرور غير صحيحة".into()))?;

    let token = state.create_token(&user.id);

    Ok(Response::json(
        200,
        Value::obj(vec![
            ("user_id".into(), Value::str(&user.id)),
            ("token".into(), Value::str(&token)),
        ]),
    ))
}

/// POST /conversations   (يتطلب Authorization: Bearer <token>)
/// body: {"participant_ids": ["id1", "id2", ...]}
/// المستخدم صاحب التوكن بينضاف تلقائيًا للمحادثة لو مش موجود في القائمة.
pub fn create_conversation(req: &Request, state: &Arc<AppState>) -> Result<Response, AppError> {
    let user_id = auth::require_auth(req, state)?;
    let body = req.json_body()?;

    let mut participant_ids: Vec<String> = body
        .get("participant_ids")
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::BadRequest("مطلوب مصفوفة participant_ids".into()))?
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();

    for participant_id in &participant_ids {
        if state.get_user(participant_id).is_none() {
            return Err(AppError::BadRequest(format!(
                "مستخدم غير موجود: {participant_id}"
            )));
        }
    }

    if !participant_ids.contains(&user_id) {
        participant_ids.push(user_id);
    }

    let conversation = state.create_conversation(participant_ids);

    Ok(Response::json(
        201,
        Value::obj(vec![
            ("conversation_id".into(), Value::str(&conversation.id)),
            (
                "participant_ids".into(),
                Value::arr(
                    conversation
                        .participant_ids
                        .iter()
                        .map(|s| Value::str(s))
                        .collect(),
                ),
            ),
        ]),
    ))
}

/// GET /conversations   (يتطلب Authorization: Bearer <token>)
/// يرجع كل المحادثات اللي المستخدم الحالي مشارك فيها.
pub fn list_conversations(req: &Request, state: &Arc<AppState>) -> Result<Response, AppError> {
    let user_id = auth::require_auth(req, state)?;
    let conversations = state.conversations_for_user(&user_id);

    let items: Vec<Value> = conversations
        .into_iter()
        .map(|c| {
            Value::obj(vec![
                ("conversation_id".into(), Value::str(&c.id)),
                (
                    "participant_ids".into(),
                    Value::arr(c.participant_ids.iter().map(|s| Value::str(s)).collect()),
                ),
                ("created_at".into(), Value::num(c.created_at as f64)),
            ])
        })
        .collect();

    Ok(Response::json(
        200,
        Value::obj(vec![("conversations".into(), Value::arr(items))]),
    ))
}

/// POST /conversations/:id/messages   (يتطلب Authorization: Bearer <token>)
/// body: {"content": "..."}
/// بيحفظ الرسالة، وبعدين يبثها فوريًا لكل المشاركين المتصلين عبر WebSocket.
pub fn post_message(req: &Request, state: &Arc<AppState>) -> Result<Response, AppError> {
    let user_id = auth::require_auth(req, state)?;
    let conversation_id = req
        .param("id")
        .ok_or_else(|| AppError::BadRequest("مفقود: معرّف المحادثة".into()))?;

    let conversation = state
        .get_conversation(conversation_id)
        .ok_or_else(|| AppError::NotFound("المحادثة غير موجودة".into()))?;

    if !conversation.participant_ids.iter().any(|p| p == &user_id) {
        return Err(AppError::Forbidden("أنت لست عضوًا في هذه المحادثة".into()));
    }

    let body = req.json_body()?;
    let content = body
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("مطلوب حقل content".into()))?;

    if content.trim().is_empty() {
        return Err(AppError::BadRequest("لا يمكن إرسال رسالة فارغة".into()));
    }

    let message = state.add_message(conversation_id, &user_id, content);

    // البث الفوري: أي مشارك متصل حاليًا عبر WebSocket هيستلم الرسالة على طول
    state.broadcast_message(&conversation, &message);

    Ok(Response::json(
        201,
        Value::obj(vec![
            ("message_id".into(), Value::str(&message.id)),
            ("conversation_id".into(), Value::str(&message.conversation_id)),
            ("sender_id".into(), Value::str(&message.sender_id)),
            ("content".into(), Value::str(&message.content)),
            ("timestamp".into(), Value::num(message.timestamp as f64)),
        ]),
    ))
}

/// GET /conversations/:id/messages   (يتطلب Authorization: Bearer <token>)
/// يرجع سجل الرسائل الكامل للمحادثة (بدون pagination في هذه المرحلة).
pub fn get_messages(req: &Request, state: &Arc<AppState>) -> Result<Response, AppError> {
    let user_id = auth::require_auth(req, state)?;
    let conversation_id = req
        .param("id")
        .ok_or_else(|| AppError::BadRequest("مفقود: معرّف المحادثة".into()))?;

    let conversation = state
        .get_conversation(conversation_id)
        .ok_or_else(|| AppError::NotFound("المحادثة غير موجودة".into()))?;

    if !conversation.participant_ids.iter().any(|p| p == &user_id) {
        return Err(AppError::Forbidden("أنت لست عضوًا في هذه المحادثة".into()));
    }

    let messages = state.messages_for_conversation(conversation_id);
    let items: Vec<Value> = messages
        .into_iter()
        .map(|m| {
            Value::obj(vec![
                ("message_id".into(), Value::str(&m.id)),
                ("sender_id".into(), Value::str(&m.sender_id)),
                ("content".into(), Value::str(&m.content)),
                ("timestamp".into(), Value::num(m.timestamp as f64)),
            ])
        })
        .collect();

    Ok(Response::json(
        200,
        Value::obj(vec![("messages".into(), Value::arr(items))]),
    ))
}

/// GET /health — endpoint بسيط للتأكد إن السيرفر شغال (مفيد أثناء التطوير)
pub fn health(_req: &Request, _state: &Arc<AppState>) -> Result<Response, AppError> {
    Ok(Response::json(200, Value::obj(vec![("status".into(), Value::str("ok"))])))
}
