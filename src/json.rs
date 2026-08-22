//! محرك JSON بسيط مكتوب يدويًا (parsing + serialization).
//! الهدف: تجنب أي dependency خارجي (زي serde) والحفاظ على مبدأ "من الصفر".
//! ده مش parser عام بمواصفات RFC 8259 الكاملة (مفيش دعم لـ unicode escapes معقدة
//! أو الأرقام العلمية e/E)، لكنه كافٍ تمامًا لكل الـ payloads اللي الـ API ده بيتعامل معها.

use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    // BTreeMap عشان نضمن ترتيب ثابت للمفاتيح لما نطبع (مفيد للاختبار والقراءة)
    Object(BTreeMap<String, Value>),
}

impl Value {
    pub fn str(s: &str) -> Value {
        Value::String(s.to_string())
    }

    pub fn num(n: f64) -> Value {
        Value::Number(n)
    }

    pub fn obj(pairs: Vec<(String, Value)>) -> Value {
        let mut map = BTreeMap::new();
        for (k, v) in pairs {
            map.insert(k, v);
        }
        Value::Object(map)
    }

    pub fn arr(items: Vec<Value>) -> Value {
        Value::Array(items)
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(map) => map.get(key),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    write!(f, "{}", *n as i64)
                } else {
                    write!(f, "{n}")
                }
            }
            Value::String(s) => write!(f, "\"{}\"", escape_string(s)),
            Value::Array(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            Value::Object(map) => {
                write!(f, "{{")?;
                for (i, (k, v)) in map.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "\"{}\":{}", escape_string(k), v)?;
                }
                write!(f, "}}")
            }
        }
    }
}

fn escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

/// Parser بسيط (recursive descent) لصيغة JSON.
pub fn parse(input: &str) -> Result<Value, String> {
    let chars: Vec<char> = input.chars().collect();
    let mut pos = 0usize;
    let value = parse_value(&chars, &mut pos)?;
    skip_whitespace(&chars, &mut pos);
    Ok(value)
}

fn skip_whitespace(chars: &[char], pos: &mut usize) {
    while *pos < chars.len() && chars[*pos].is_whitespace() {
        *pos += 1;
    }
}

fn parse_value(chars: &[char], pos: &mut usize) -> Result<Value, String> {
    skip_whitespace(chars, pos);
    if *pos >= chars.len() {
        return Err("نهاية غير متوقعة للنص أثناء تحليل JSON".to_string());
    }
    match chars[*pos] {
        '{' => parse_object(chars, pos),
        '[' => parse_array(chars, pos),
        '"' => parse_string(chars, pos).map(Value::String),
        't' | 'f' => parse_bool(chars, pos),
        'n' => parse_null(chars, pos),
        _ => parse_number(chars, pos),
    }
}

fn parse_object(chars: &[char], pos: &mut usize) -> Result<Value, String> {
    *pos += 1; // تخطي '{'
    let mut map = BTreeMap::new();
    skip_whitespace(chars, pos);
    if *pos < chars.len() && chars[*pos] == '}' {
        *pos += 1;
        return Ok(Value::Object(map));
    }
    loop {
        skip_whitespace(chars, pos);
        let key = parse_string(chars, pos)?;
        skip_whitespace(chars, pos);
        if *pos >= chars.len() || chars[*pos] != ':' {
            return Err("متوقع ':' بعد مفتاح الـ object".to_string());
        }
        *pos += 1;
        let value = parse_value(chars, pos)?;
        map.insert(key, value);
        skip_whitespace(chars, pos);
        if *pos >= chars.len() {
            return Err("نهاية غير متوقعة داخل object".to_string());
        }
        match chars[*pos] {
            ',' => {
                *pos += 1;
            }
            '}' => {
                *pos += 1;
                break;
            }
            _ => return Err("متوقع ',' أو '}' داخل object".to_string()),
        }
    }
    Ok(Value::Object(map))
}

fn parse_array(chars: &[char], pos: &mut usize) -> Result<Value, String> {
    *pos += 1; // تخطي '['
    let mut items = Vec::new();
    skip_whitespace(chars, pos);
    if *pos < chars.len() && chars[*pos] == ']' {
        *pos += 1;
        return Ok(Value::Array(items));
    }
    loop {
        let value = parse_value(chars, pos)?;
        items.push(value);
        skip_whitespace(chars, pos);
        if *pos >= chars.len() {
            return Err("نهاية غير متوقعة داخل array".to_string());
        }
        match chars[*pos] {
            ',' => {
                *pos += 1;
            }
            ']' => {
                *pos += 1;
                break;
            }
            _ => return Err("متوقع ',' أو ']' داخل array".to_string()),
        }
    }
    Ok(Value::Array(items))
}

fn parse_string(chars: &[char], pos: &mut usize) -> Result<String, String> {
    if chars.get(*pos) != Some(&'"') {
        return Err("متوقع بداية نص \"".to_string());
    }
    *pos += 1;
    let mut out = String::new();
    while *pos < chars.len() && chars[*pos] != '"' {
        let c = chars[*pos];
        if c == '\\' {
            *pos += 1;
            if *pos >= chars.len() {
                return Err("escape غير مكتمل في نهاية النص".to_string());
            }
            match chars[*pos] {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                '/' => out.push('/'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                other => out.push(other),
            }
        } else {
            out.push(c);
        }
        *pos += 1;
    }
    if *pos >= chars.len() {
        return Err("نص غير مغلق بـ \"".to_string());
    }
    *pos += 1; // تخطي الـ " الختامية
    Ok(out)
}

fn parse_bool(chars: &[char], pos: &mut usize) -> Result<Value, String> {
    if chars[*pos..].starts_with(&['t', 'r', 'u', 'e']) {
        *pos += 4;
        Ok(Value::Bool(true))
    } else if chars[*pos..].starts_with(&['f', 'a', 'l', 's', 'e']) {
        *pos += 5;
        Ok(Value::Bool(false))
    } else {
        Err("قيمة boolean غير صحيحة".to_string())
    }
}

fn parse_null(chars: &[char], pos: &mut usize) -> Result<Value, String> {
    if chars[*pos..].starts_with(&['n', 'u', 'l', 'l']) {
        *pos += 4;
        Ok(Value::Null)
    } else {
        Err("قيمة null غير صحيحة".to_string())
    }
}

fn parse_number(chars: &[char], pos: &mut usize) -> Result<Value, String> {
    let start = *pos;
    if *pos < chars.len() && (chars[*pos] == '-' || chars[*pos] == '+') {
        *pos += 1;
    }
    while *pos < chars.len() && (chars[*pos].is_ascii_digit() || chars[*pos] == '.') {
        *pos += 1;
    }
    let s: String = chars[start..*pos].iter().collect();
    s.parse::<f64>()
        .map(Value::Number)
        .map_err(|_| format!("رقم غير صحيح: {s}"))
}
