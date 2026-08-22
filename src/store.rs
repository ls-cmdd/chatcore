//! الطبقة 10: In-memory Data Store.
//! كل البيانات (مستخدمين، محادثات، رسائل، اتصالات WebSocket نشطة) مخزنة في
//! الذاكرة فقط (HashMap) محمية بـ Mutex عشان تكون آمنة عبر threads متعددة
//! (كل اتصال بيتعالج في thread منفصل من الـ thread pool).
//! ملاحظة: البيانات كلها بتضيع لما السيرفر يقفل - مفيش قاعدة بيانات في هذه المرحلة.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::crypto;
use crate::ws::frame;

#[derive(Debug, Clone)]
pub struct User {
    pub id: String,
    pub username: String,
    pub password_salt: String,
    pub password_hash: String,
}

#[derive(Debug, Clone)]
pub struct Conversation {
    pub id: String,
    pub participant_ids: Vec<String>,
    pub created_at: u64,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub sender_id: String,
    pub content: String,
    pub timestamp: u64,
}

pub struct AppState {
    users_by_id: Mutex<HashMap<String, User>>,
    username_index: Mutex<HashMap<String, String>>, // username -> user_id
    tokens: Mutex<HashMap<String, String>>,          // token -> user_id
    conversations: Mutex<HashMap<String, Conversation>>,
    messages: Mutex<HashMap<String, Vec<Message>>>, // conversation_id -> رسائلها
    // user_id -> (conn_id -> sender قناة إرسال البايتات لهذا الاتصال المفتوح)
    ws_registry: Mutex<HashMap<String, HashMap<u64, mpsc::Sender<Vec<u8>>>>>,
    ws_conn_counter: AtomicU64,
}

impl AppState {
    pub fn new() -> AppState {
        AppState {
            users_by_id: Mutex::new(HashMap::new()),
            username_index: Mutex::new(HashMap::new()),
            tokens: Mutex::new(HashMap::new()),
            conversations: Mutex::new(HashMap::new()),
            messages: Mutex::new(HashMap::new()),
            ws_registry: Mutex::new(HashMap::new()),
            ws_conn_counter: AtomicU64::new(1),
        }
    }

    // ---------- المستخدمين والمصادقة ----------

    pub fn create_user(&self, username: &str, password: &str) -> Result<User, String> {
        let mut username_index = self.username_index.lock().unwrap();
        if username_index.contains_key(username) {
            return Err("اسم المستخدم مستخدم بالفعل".to_string());
        }

        let id = crypto::generate_id();
        let salt = crypto::generate_id();
        let password_hash = crypto::hash_password(password, &salt);

        let user = User {
            id: id.clone(),
            username: username.to_string(),
            password_salt: salt,
            password_hash,
        };

        username_index.insert(username.to_string(), id.clone());
        self.users_by_id.lock().unwrap().insert(id, user.clone());

        Ok(user)
    }

    pub fn verify_login(&self, username: &str, password: &str) -> Option<User> {
        let username_index = self.username_index.lock().unwrap();
        let user_id = username_index.get(username)?.clone();
        drop(username_index);

        let users = self.users_by_id.lock().unwrap();
        let user = users.get(&user_id)?;

        let attempt_hash = crypto::hash_password(password, &user.password_salt);
        if attempt_hash == user.password_hash {
            Some(user.clone())
        } else {
            None
        }
    }

    pub fn create_token(&self, user_id: &str) -> String {
        let token = crypto::generate_token();
        self.tokens
            .lock()
            .unwrap()
            .insert(token.clone(), user_id.to_string());
        token
    }

    pub fn user_id_for_token(&self, token: &str) -> Option<String> {
        self.tokens.lock().unwrap().get(token).cloned()
    }

    pub fn get_user(&self, user_id: &str) -> Option<User> {
        self.users_by_id.lock().unwrap().get(user_id).cloned()
    }

    // ---------- المحادثات والرسائل ----------

    pub fn create_conversation(&self, participant_ids: Vec<String>) -> Conversation {
        let conversation = Conversation {
            id: crypto::generate_id(),
            participant_ids,
            created_at: now_unix(),
        };
        self.conversations
            .lock()
            .unwrap()
            .insert(conversation.id.clone(), conversation.clone());
        self.messages
            .lock()
            .unwrap()
            .insert(conversation.id.clone(), Vec::new());
        conversation
    }

    pub fn get_conversation(&self, id: &str) -> Option<Conversation> {
        self.conversations.lock().unwrap().get(id).cloned()
    }

    pub fn conversations_for_user(&self, user_id: &str) -> Vec<Conversation> {
        self.conversations
            .lock()
            .unwrap()
            .values()
            .filter(|c| c.participant_ids.iter().any(|p| p == user_id))
            .cloned()
            .collect()
    }

    pub fn add_message(&self, conversation_id: &str, sender_id: &str, content: &str) -> Message {
        let message = Message {
            id: crypto::generate_id(),
            conversation_id: conversation_id.to_string(),
            sender_id: sender_id.to_string(),
            content: content.to_string(),
            timestamp: now_unix(),
        };

        self.messages
            .lock()
            .unwrap()
            .entry(conversation_id.to_string())
            .or_default()
            .push(message.clone());

        message
    }

    pub fn messages_for_conversation(&self, conversation_id: &str) -> Vec<Message> {
        self.messages
            .lock()
            .unwrap()
            .get(conversation_id)
            .cloned()
            .unwrap_or_default()
    }

    // ---------- سجل اتصالات WebSocket ----------

    /// يسجّل اتصال WebSocket جديد لمستخدم معين، ويرجع معرّف فريد للاتصال
    /// (لازم نحتفظ بيه عشان نقدر نشيله بعدين لما الاتصال يتقفل).
    pub fn register_ws_sender(&self, user_id: &str, tx: mpsc::Sender<Vec<u8>>) -> u64 {
        let conn_id = self.ws_conn_counter.fetch_add(1, Ordering::Relaxed);
        self.ws_registry
            .lock()
            .unwrap()
            .entry(user_id.to_string())
            .or_default()
            .insert(conn_id, tx);
        conn_id
    }

    pub fn unregister_ws_sender(&self, user_id: &str, conn_id: u64) {
        if let Some(map) = self.ws_registry.lock().unwrap().get_mut(user_id) {
            map.remove(&conn_id);
        }
    }

    /// يبعت رسالة (بايتات frame جاهزة) لكل اتصالات WebSocket المفتوحة لمستخدم معين
    /// (المستخدم ممكن يكون فاتح أكتر من تبويب/جهاز في نفس الوقت).
    fn broadcast_to_user(&self, user_id: &str, bytes: &[u8]) {
        if let Some(map) = self.ws_registry.lock().unwrap().get(user_id) {
            for tx in map.values() {
                let _ = tx.send(bytes.to_vec());
            }
        }
    }

    /// يبعت رسالة جديدة لكل المشاركين في محادثة معينة عبر WebSocket (فوريًا).
    pub fn broadcast_message(&self, conversation: &Conversation, message: &Message) {
        let payload = crate::json::Value::obj(vec![
            ("type".into(), crate::json::Value::str("message")),
            (
                "conversation_id".into(),
                crate::json::Value::str(&message.conversation_id),
            ),
            ("message_id".into(), crate::json::Value::str(&message.id)),
            (
                "sender_id".into(),
                crate::json::Value::str(&message.sender_id),
            ),
            ("content".into(), crate::json::Value::str(&message.content)),
            (
                "timestamp".into(),
                crate::json::Value::num(message.timestamp as f64),
            ),
        ])
        .to_string();

        let frame_bytes = frame::encode_text_frame(&payload);

        for participant_id in &conversation.participant_ids {
            self.broadcast_to_user(participant_id, &frame_bytes);
        }
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
