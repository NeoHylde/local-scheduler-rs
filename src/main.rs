use std::sync::Arc;

use axum::{
    Json, Router, http::{HeaderMap, StatusCode}, routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Semaphore;

#[derive(Clone)]
struct AppState {
    client: reqwest::Client,
    gpu: Arc<Semaphore>,
    upstream: String,
}

#[tokio::main]
async fn main() {
    let state = AppState {
        client: reqwest::Client::new(),
        gpu: Arc::new(Semaphore::new(1)),
        upstream: "http://127.0.0.1:11434".to_string(),
    };
    // initialize tracing
    tracing_subscriber::fmt::init();

    // build our application with a route
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/", post(root))
        .route("/generate", post(handle_generate))
        .route("/chat", post(handle_chat))
        .with_state(state)
        ;

    let listener = tokio::net::TcpListener::bind("0.0.0.0:11435").await.unwrap();
    let _ = axum::serve(listener, app).await;
}

async fn handle_generate(headers: HeaderMap, Json(_payload): Json<GenerateRequest>) -> StatusCode {
    let session = headers.get("x-vramd-session").and_then(|v| v.to_str().ok());

    StatusCode::NOT_IMPLEMENTED
}

async fn handle_chat(headers: HeaderMap, Json(_payload): Json<ChatRequest>) -> StatusCode {
    let session = headers.get("x-vramd-session").and_then(|v| v.to_str().ok());

    StatusCode::NOT_IMPLEMENTED
}

async fn root() -> &'static str {
    "Hello World!"
}

// ollama specific stuff
#[derive(Serialize, Deserialize, Default)]
struct Options {
    #[serde(skip_serializing_if = "Option::is_none")] temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")] top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")] top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")] num_ctx: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")] num_predict: Option<i32>, // -1 = infinite
    #[serde(skip_serializing_if = "Option::is_none")] seed: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")] stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")] repeat_penalty: Option<f32>,
}

// 
#[derive(Serialize, Deserialize, Clone)]
struct Message {
    role: String,                 // "system" | "user" | "assistant" | "tool"
    content: String,              // required; "" on the final chat chunk
    #[serde(skip_serializing_if = "Option::is_none")] thinking: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] images: Option<Vec<String>>,   // base64
    #[serde(skip_serializing_if = "Option::is_none")] tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")] tool_name: Option<String>,     // role="tool" replies
}

#[derive(Deserialize, Default)]
struct Stats {
    done_reason: Option<String>,
    total_duration: Option<u64>,      // ns
    load_duration: Option<u64>,       // ns
    prompt_eval_count: Option<u64>,   // prompt tokens
    prompt_eval_duration: Option<u64>,// ns
    eval_count: Option<u64>,          // generated tokens
    eval_duration: Option<u64>,       // ns
}

impl Stats {
    fn tokens_per_sec(&self) -> Option<f64> {
        match (self.eval_count, self.eval_duration) {
            (Some(c), Some(d)) if d > 0 => Some(c as f64 / (d as f64 / 1e9)),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")] suffix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] images: Option<Vec<String>>, // base64
    #[serde(skip_serializing_if = "Option::is_none")] format: Option<Value>,        // "json" | schema obj
    #[serde(skip_serializing_if = "Option::is_none")] options: Option<Options>,
    #[serde(skip_serializing_if = "Option::is_none")] think: Option<Value>,         // bool | "low"/"high"
    #[serde(skip_serializing_if = "Option::is_none")] raw: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")] keep_alive: Option<String>,   // "5m", "0", "-1"
}

#[derive(Deserialize)]
struct GenerateChunk {
    model: String,
    created_at: String,
    #[serde(default)] response: String,        // token delta; full text if stream=false
    #[serde(default)] thinking: Option<String>,
    done: bool,
    #[serde(flatten)] stats: Stats,            // populated only when done=true
    #[serde(default)] context: Option<Vec<i64>>, // deprecated, still emitted
}

#[derive(Deserialize, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")] tools: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")] format: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] options: Option<Options>,
    #[serde(skip_serializing_if = "Option::is_none")] think: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] keep_alive: Option<String>,
}

#[derive(Deserialize)]
struct ChatChunk {
    model: String,
    created_at: String,
    message: Message,              // delta lives in message.content
    done: bool,
    #[serde(flatten)] stats: Stats,
}

#[derive(Serialize, Deserialize, Clone)]
struct ToolCall { function: FunctionCall }

#[derive(Serialize, Deserialize, Clone)]
struct FunctionCall {
    name: String,
    arguments: Value,             // arbitrary JSON object
}