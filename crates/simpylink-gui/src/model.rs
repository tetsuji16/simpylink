//! Model types mirroring schemas/model.schema.json (GUI side).
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub name: String,
    pub version: String,
    pub blocks: Vec<Block>,
    pub connections: Vec<Connection>,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            name: "untitled".into(),
            version: "0.1.0".into(),
            blocks: vec![],
            connections: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub id: String,
    #[serde(rename = "type")]
    pub block_type: String,
    pub position: Position,
    #[serde(default)]
    pub params: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub from: PortRef,
    pub to: PortRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortRef {
    pub block: String,
    pub port: String,
}

/// Palette entry returned by GET /blocks.
#[derive(Debug, Clone, Deserialize)]
pub struct BlockDef {
    #[serde(rename = "type")]
    pub block_type: String,
    pub label: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub outputs: Vec<String>,
    #[serde(default)]
    pub params: Vec<ParamDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ParamDef {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,
    pub default: serde_json::Value,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Deserialize)]
pub struct BlocksResponse {
    pub blocks: Vec<BlockDef>,
}

#[derive(Debug, Deserialize)]
pub struct SimulateResponse {
    #[allow(dead_code)]
    pub status: String,
    #[serde(default)]
    pub outputs: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub trace: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct TimeSimResponse {
    #[allow(dead_code)]
    pub status: String,
    #[serde(default)]
    pub time: Vec<f64>,
    #[serde(default)]
    pub outputs: HashMap<String, Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    #[serde(default)]
    pub detail: String,
}
