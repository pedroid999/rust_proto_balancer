use serde_json::{json, Value};

#[derive(Debug)]
pub struct JsonRpcErrorResponse {
    pub id: Value,
    pub jsonrpc: String,
    pub error: ApplicationError,
}

impl JsonRpcErrorResponse {
    pub fn new(error: ApplicationError) -> Self {
        Self {
            id: Value::Null,
            jsonrpc: "2.0".to_string(),
            error,
        }
    }

    pub fn to_json(&self) -> String {
        json!({
            "id": &self.id,
            "jsonrpc": &self.jsonrpc,
            "error": &self.error.format_error(),
        })
            .to_string()
    }
}

impl From<ApplicationError> for JsonRpcErrorResponse {
    fn from(error: ApplicationError) -> Self {
        Self::new(error)
    }
}

#[derive(Debug)]
pub enum ErrorCode {
    BadRequest = 400,
    NotFound = 404,
    InternalServerError = 500,
    RequestTimeout = 408,
    HandleConnectionError = 502,
    UnknownError = 520,
}

impl ErrorCode {
    fn as_str(&self) -> &'static str {
        match *self {
            ErrorCode::BadRequest => "400",
            ErrorCode::NotFound => "404",
            ErrorCode::InternalServerError => "500",
            ErrorCode::RequestTimeout => "408",
            ErrorCode::HandleConnectionError => "502",
            ErrorCode::UnknownError => "520",
        }
    }
}

#[derive(Debug)]
pub struct ApplicationError {
    pub code: ErrorCode,
    pub message: String,
}

impl ApplicationError {
    pub fn new(code: ErrorCode, message: String) -> Self {
        Self { code, message }
    }

    pub fn format_error(&self) -> String {
        format!("{}: {}", self.code.as_str(), self.message)
    }
}
