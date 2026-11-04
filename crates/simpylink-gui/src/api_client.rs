//! HTTP client for the Python backend. GUI never computes block math.
use crate::model::{BlocksResponse, ErrorResponse, Model, SimulateResponse, TimeSimResponse};
use serde_json::json;

pub struct ApiClient {
    base_url: String,
    client: reqwest::blocking::Client,
}

impl Default for ApiClient {
    fn default() -> Self {
        let base = std::env::var("SIMPYLINK_BACKEND_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8765".into());
        Self {
            base_url: base,
            client: reqwest::blocking::Client::new(),
        }
    }
}

impl ApiClient {
    pub fn fetch_blocks(&self) -> Result<Vec<crate::model::BlockDef>, String> {
        let resp = self
            .client
            .get(format!("{}/blocks", self.base_url))
            .send()
            .map_err(|e| {
                format!(
                    "backend unreachable at {} ({e}). Start it: python backend/run.py",
                    self.base_url
                )
            })?;
        if !resp.status().is_success() {
            return Err(format!("GET /blocks -> {}", resp.status()));
        }
        let parsed: BlocksResponse = resp.json().map_err(|e| e.to_string())?;
        Ok(parsed.blocks)
    }

    fn post(
        &self,
        path: &str,
        body: serde_json::Value,
    ) -> Result<reqwest::blocking::Response, String> {
        self.client
            .post(format!("{}{}", self.base_url, path))
            .json(&body)
            .send()
            .map_err(|e| format!("backend unreachable ({e})"))
    }

    fn parse<T: for<'de> serde::Deserialize<'de>>(
        &self,
        resp: reqwest::blocking::Response,
    ) -> Result<T, String> {
        if resp.status().is_success() {
            resp.json().map_err(|e| e.to_string())
        } else {
            let err: ErrorResponse = resp.json().unwrap_or(ErrorResponse {
                error: "unknown".into(),
                detail: String::new(),
            });
            Err(format!("{}: {}", err.error, err.detail))
        }
    }

    pub fn simulate(&self, model: &Model) -> Result<SimulateResponse, String> {
        let body = json!({"model": model, "inputs": {}});
        self.parse(self.post("/simulate", body)?)
    }


    pub fn run_codegen(&self, model: &Model, mode: &str, t_stop: f64, dt: f64) -> Result<String, String> {
        use std::process::Command;
        let python = std::env::var("SIMPYLINK_PYTHON").unwrap_or_else(|_| "python".into());
        let backend_dir = std::env::var("SIMPYLINK_BACKEND_DIR").unwrap_or_else(|_| "backend".into());
        let tmp = std::env::temp_dir().join(format!("simpylink_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).map_err(|e| format!("tempdir: {e}"))?;
        let model_path = tmp.join("model.json");
        std::fs::write(&model_path, serde_json::to_string_pretty(model).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let gen_path = tmp.join("model.py");
        let out = Command::new(&python)
            .args([
                "-m",
                "simpylink_backend.codegen",
                &model_path.to_string_lossy(),
                "--output",
                &gen_path.to_string_lossy(),
                "--run",
                "--mode",
                mode,
                "--t_stop",
                &t_stop.to_string(),
                "--dt",
                &dt.to_string(),
            ])
            .env("PYTHONPATH", &backend_dir)
            .output()
            .map_err(|e| format!("failed to spawn {python} ({e}). Set SIMPYLINK_PYTHON / SIMPYLINK_BACKEND_DIR"))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(format!("codegen failed: {err}"));
        }
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let json_line = stdout.lines().last().unwrap_or("").trim();
        let v: serde_json::Value = serde_json::from_str(json_line)
            .map_err(|e| format!("codegen output not JSON ({e}): {stdout}"))?;
        if v.get("status").and_then(|s| s.as_str()) != Some("ok") {
            return Err(format!("codegen: {v}"));
        }
        Ok(json_line.to_string())
    }
    pub fn simulate_time(
        &self,
        model: &Model,
        t_stop: f64,
        dt: f64,
    ) -> Result<TimeSimResponse, String> {
        let body =
            json!({"model": model, "inputs": {}, "t_start": 0.0, "t_stop": t_stop, "dt": dt});
        self.parse(self.post("/simulate_time", body)?)
    }
}
