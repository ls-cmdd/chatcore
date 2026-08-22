# Chat Backend — مبني من الصفر بـ Rust

Backend كامل لـ Chat/Messaging API، مبني يدويًا بالكامل بلغة Rust بدون أي framework
(لا Actix، لا Axum، لا Tokio) وبدون أي dependency خارجي — `Cargo.toml` فاضي تمامًا.

## ⚠️ ملاحظة مهمة حول هذا الكود

الكود ده اتكتب في بيئة معزولة عن الإنترنت ومفيهاش Rust compiler مثبّت، فمقدرتش
أعمل `cargo build` وأتأكد إنه بيتصرّف 100% زي ما هو متوقع قبل ما أبعتهولك.
البنية والمنطق اتكتبوا بعناية ومراجعة يدوية دقيقة لكل التفاصيل (borrow checker،
أنواع البيانات، RFC 6455 لبروتوكول WebSocket)، لكن ممكن يطلع في التجميع الأول
أخطاء صغيرة (typo، نوع بيانات محتاج تحويل بسيط). **لو حصل كده، ابعتلي رسالة
الخطأ من `cargo build` وهصلحها فورًا.**

## المتطلبات

- Rust (نسخة حديثة، edition 2021) — ثبّته من https://rustup.rs لو مش موجود

## طريقة التشغيل

```bash
cd chat_backend
cargo build
cargo run
```

هيظهرلك:
```
🚀 السيرفر شغال على http://127.0.0.1:8080
   WebSocket متاح على ws://127.0.0.1:8080/ws?token=<token>
   عدد الـ workers في الـ thread pool: 8
```

## بنية المشروع (كل طبقة في ملف منفصل)

```
src/
├── main.rs           نقطة الدخول: تسجيل الـ routes وتشغيل السيرفر
├── server.rs         الطبقة 1: TCP Listener + توجيه HTTP مقابل WebSocket
├── http/
│   ├── parser.rs     الطبقة 2: HTTP parser يدوي (request line + headers + body)
│   ├── request.rs    الطبقة 3: struct Request
│   └── response.rs   الطبقة 3: struct Response + تحويلها لبايتات HTTP
├── thread_pool.rs    الطبقة 4: Thread pool يدوي (mpsc + workers)
├── router.rs         الطبقة 5: مطابقة (method, path) بـ handler + path params
├── middleware.rs     الطبقة 6: logging قبل/بعد كل request
├── error.rs          الطبقة 7: AppError موحّد -> HTTP response
├── ws/
│   ├── handshake.rs  الطبقة 8: ترقية HTTP -> WebSocket (RFC 6455)
│   ├── frame.rs      الطبقة 9: فك/بناء WebSocket frames
│   └── connection.rs إدارة دورة حياة اتصال WS (reader + writer threads)
├── store.rs          الطبقة 10: تخزين in-memory (Mutex<HashMap<...>>)
├── auth.rs           الطبقة 11: استخراج والتحقق من التوكن
├── handlers.rs        الطبقة 12: منطق أعمال الشات (endpoints)
├── json.rs           محرك JSON مكتوب يدويًا (parser + serializer)
├── crypto.rs         SHA-1 + Base64 يدويًا (لازمين لـ WebSocket handshake)
└── util.rs           تحليل query string + percent-decoding
```

## سيناريو اختبار كامل (end-to-end)

### 1. تسجيل مستخدمين

```bash
curl -s -X POST http://127.0.0.1:8080/register \
  -d '{"username":"ali","password":"1234"}'
# => {"token":"...","user_id":"...","username":"ali"}

curl -s -X POST http://127.0.0.1:8080/register \
  -d '{"username":"sara","password":"1234"}'
# => {"token":"...","user_id":"...","username":"sara"}
```

احفظ الـ `token` و`user_id` بتاع كل واحد من الاتنين — هتحتاجهم في الخطوات الجاية.

### 2. إنشاء محادثة بين المستخدمين

```bash
curl -s -X POST http://127.0.0.1:8080/conversations \
  -H "Authorization: Bearer <token_ali>" \
  -d '{"participant_ids":["<user_id_sara>"]}'
# => {"conversation_id":"...", "participant_ids":[...]}
```

### 3. فتح اتصال WebSocket (لاستقبال الرسائل فوريًا)

افتح ملف `test.html` في المتصفح، الصق توكن sara، اضغط "اتصال بـ WebSocket".
(أو استخدم أي أداة WebSocket client زي `websocat`:
`websocat "ws://127.0.0.1:8080/ws?token=<token_sara>"`)

### 4. بعت رسالة من ali

```bash
curl -s -X POST http://127.0.0.1:8080/conversations/<conversation_id>/messages \
  -H "Authorization: Bearer <token_ali>" \
  -d '{"content":"أهلاً يا سارة!"}'
```

المفروض تظهر الرسالة فورًا في نافذة `test.html` المفتوحة لـ sara (بث فوري عبر WebSocket).

### 5. جلب سجل الرسائل

```bash
curl -s http://127.0.0.1:8080/conversations/<conversation_id>/messages \
  -H "Authorization: Bearer <token_sara>"
```

## قرارات تصميمية مهمة (ولماذا)

- **Content-Length فقط، بدون chunked transfer encoding**: تبسيط مقصود؛ كافٍ لكل
  الـ endpoints الحالية.
- **Connection: close على كل response**: بدون keep-alive، لتفادي تعقيد إدارة
  اتصالات مستمرة في الطبقة الأساسية.
- **WebSocket frames بدون دعم fragmentation**: كل frame بيتعامل معه كرسالة كاملة
  (FIN=1) — كافٍ لرسائل شات نصية عادية.
- **التخزين in-memory بالكامل**: البيانات بتضيع عند إعادة تشغيل السيرفر. خطوة
  منطقية تالية (خارج نطاق هذا المشروع الحالي) هي إضافة SQLite أو Postgres.
- **hashing كلمة المرور بـ SHA-1 يدوي**: كافٍ لمنع تخزين نص صريح في مشروع تعليمي،
  لكنه **غير مناسب أمنيًا** لإنتاج حقيقي (يُفضّل bcrypt/argon2).
- **توليد التوكن/الـ IDs بدون CSPRNG حقيقي**: نفس الملاحظة أعلاه — مقبول تعليميًا،
  غير كافٍ أمنيًا لبيانات حساسة حقيقية.

## خطوات تالية ممكنة (خارج النطاق الحالي)

- إضافة قاعدة بيانات (SQLite) بدل in-memory
- دعم WebSocket message fragmentation الكامل
- Keep-alive connections بدل Connection: close
- Rate limiting وحماية من DoS
- استبدال hashing كلمة المرور بـ bcrypt/argon2 حقيقي
