> Source: https://github.com/tetsuji16/simpylink/issues/17 (copied on 2026-11-07)

> 計画ID: `DES-01` ／ 親: #16 ／ Blocked by: なし

# [DES-01] 設計契約：Cargo Workspace・クレート責務・依存方向・公開API・CLI

> この Issue は **設計契約** です。実装 Issue はここに書かれた名前・型・依存方向に従います。
> 契約変更が必要な場合は、実装せずにこの Issue にコメントしてください。

## 1. リポジトリ構成（確定）

リポジトリ直下を Workspace ルートとする（`simulator/` サブディレクトリは作らない）。

```text
simpylink/
├── Cargo.toml                 # [workspace] 定義のみ（パッケージなし）
├── rust-toolchain.toml        # channel = "stable"
├── AGENTS.md                  # 実装AI向け規約（#41）
├── README.md
├── LICENSE                    # MIT（既存を継続）
├── crates/
│   ├── sim_core/              # モデル・幾何・式・ブロック登録・検証・編集コマンド
│   ├── sim_ir/                # Model -> SimIr コンパイラ
│   ├── sim_codegen_py/        # SimIr -> Python ソース
│   ├── sim_runner/            # Python 子プロセス実行・結果読込（sim_* 非依存）
│   ├── sim_slx/               # SLX 読み書き
│   ├── sim_gui/               # egui アプリ本体（ライブラリ）
│   └── sim_app/               # バイナリ `simpylink`（GUI起動 + CLI）
├── python/
│   ├── requirements.txt       # numpy>=1.26, scipy>=1.11, pytest>=8
│   └── tests/                 # 生成コードの pytest
├── tests/
│   ├── fixtures/
│   │   ├── models/            # 共有テスト用モデル JSON
│   │   └── expected/          # ゴールデン Python / CSV
│   └── integration/           # パッケージ `sim_integration`（publish = false）
│       ├── Cargo.toml
│       └── tests/*.rs
├── docs/architecture/         # 設計Issueの写し（#41）
└── .github/workflows/ci.yml
```

## 2. ツールチェーンと依存クレート（確定バージョン）

| 項目 | 値 |
|---|---|
| Rust | stable、`rust-version = "1.95"`（eframe 0.36 の MSRV） |
| edition | 2021 |
| Python | 3.10 以上 |
| NumPy / SciPy | numpy>=1.26 / scipy>=1.11 |

ルート `Cargo.toml` の `[workspace.dependencies]`（各クレートは `xxx.workspace = true` で参照）：

```toml
[workspace]
resolver = "2"
members = [
  "crates/sim_core", "crates/sim_ir", "crates/sim_codegen_py", "crates/sim_runner",
  "crates/sim_slx", "crates/sim_gui", "crates/sim_app", "tests/integration",
]

[workspace.package]
version = "0.2.0"
edition = "2021"
rust-version = "1.95"
license = "MIT"
repository = "https://github.com/tetsuji16/simpylink"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
egui = "0.36"
eframe = "0.36"
egui_plot = "0.37"     # egui 0.36 に対応する版。必ず `cargo tree -i egui` で egui が1版のみであることを確認
rfd = "0.15"
clap = { version = "4.5", features = ["derive"] }
zip = { version = "8", default-features = false, features = ["deflate"] }
quick-xml = "0.41"
tempfile = "3"
sim_core = { path = "crates/sim_core" }
sim_ir = { path = "crates/sim_ir" }
sim_codegen_py = { path = "crates/sim_codegen_py" }
sim_runner = { path = "crates/sim_runner" }
sim_slx = { path = "crates/sim_slx" }
sim_gui = { path = "crates/sim_gui" }
```

## 3. クレート責務と依存方向（確定）

依存は下表の「依存してよい」以外を禁止する（循環防止）。

| クレート | 種別 | 依存してよい sim_* | 外部依存 | 責務 |
|---|---|---|---|---|
| sim_core | lib | なし | serde, serde_json, thiserror | ID・幾何・Model・JSON入出力・式パーサ・ブロック登録・構造検証・編集コマンド/History・クリップボード |
| sim_ir | lib | sim_core | thiserror | Model → SimIr（パラメータ評価・グラフ・実行順・サンプル時間・状態割当・診断） |
| sim_codegen_py | lib | sim_ir, sim_core（ID 型参照のみ） | thiserror | SimIr → Python ソース文字列 |
| sim_runner | lib | **なし** | serde, serde_json, thiserror | Python 探索・環境確認・子プロセス実行・停止・結果読込 |
| sim_slx | lib | sim_core | zip, quick-xml, thiserror | SLX ⇔ Model 変換・未対応要素保持 |
| sim_gui | lib | sim_core, sim_ir, sim_codegen_py, sim_runner, sim_slx | egui, eframe, egui_plot, rfd | GUI。キャンバス制御は egui 非依存の純粋ロジックに分離 |
| sim_app | bin `simpylink` | 全 sim_* | clap, eframe | エントリポイント・CLI |
| sim_integration | test only | 全 sim_* | tempfile | E2E テスト |

**GUI 描画構造（画面座標、色）は sim_core / sim_ir に入れない。** モデル座標（`Rect`）は sim_core に置く。

## 4. 公開API一覧（モジュール単位の契約）

型の詳細は #18（sim_core）、#19（sim_ir）、#20（codegen/runner）、#21（gui）、#22（slx）。

### sim_core
```rust
pub mod ids;       // BlockId, LineId（u32 newtype、serde transparent）
pub mod geom;      // Point, Rect
pub mod path;      // SystemPath
pub mod model;     // Model, SimConfig, SolverKind, System, Block, Orientation, Line, Branch, PortRef, TapPoint
pub mod json;      // to_json_string, from_json_str, save_to_path, load_from_path, ModelIoError
pub mod expr;      // parse_value, Value, Matrix, ExprError
pub mod library;   // BlockSpec, ParamSpec, ParamKind, registry, spec, port_counts, new_block, unique_block_name
pub mod diag;      // Diagnostic, Severity, Location, codes
pub mod validate;  // validate_model
pub mod edit;      // Command, apply, History, EditError
pub mod clipboard; // ClipboardPayload, copy_selection, paste_payload
```
### sim_ir
```rust
pub fn compile(model: &sim_core::model::Model) -> Result<CompiledModel, CompileFailure>;
pub struct CompiledModel { pub ir: SimIr, pub diagnostics: Vec<sim_core::diag::Diagnostic> }
pub struct CompileFailure { pub diagnostics: Vec<sim_core::diag::Diagnostic> } // 少なくとも1件 Error を含む
```
### sim_codegen_py
```rust
pub fn generate(ir: &sim_ir::SimIr, opts: &CodegenOptions) -> Result<GeneratedCode, CodegenError>;
pub struct CodegenOptions { pub generator_version: String }
pub struct GeneratedCode { pub file_name: String, pub source: String }
```
### sim_runner
```rust
pub struct RunnerConfig { pub python: Option<std::path::PathBuf>, pub timeout: Option<std::time::Duration> }
pub struct PythonEnv { pub program: PathBuf, pub prefix_args: Vec<String>, pub version: String }
pub fn discover_python(cfg: &RunnerConfig) -> Result<PythonEnv, RunnerError>;
pub fn check_environment(env: &PythonEnv) -> Result<EnvReport, RunnerError>;
pub struct RunHandle { /* private */ }
impl RunHandle {
    pub fn start(env: &PythonEnv, script: &Path, out_dir: &Path) -> Result<RunHandle, RunnerError>;
    pub fn poll_events(&mut self) -> Vec<RunEvent>;
    pub fn stop(&mut self);
    pub fn is_finished(&self) -> bool;
}
pub fn run_blocking(env: &PythonEnv, script: &Path, out_dir: &Path, timeout: Option<Duration>) -> Result<RunOutcome, RunnerError>;
pub fn load_results(out_dir: &Path) -> Result<SimResults, ResultsError>;
```
### sim_slx（M5）
```rust
pub fn read_slx(path: &Path) -> Result<SlxImport, SlxError>;
pub fn write_slx(model: &Model, preserved: Option<&PreservedData>, path: &Path, opts: &WriteOptions) -> Result<WriteReport, SlxError>;
```
### sim_gui
```rust
pub struct SimpylinkApp; impl eframe::App for SimpylinkApp { ... }
pub fn run_gui(initial_file: Option<PathBuf>) -> eframe::Result<()>;
pub mod canvas { pub mod view; pub mod hit; pub mod route; pub mod controller; pub mod widget; }
```

## 5. CLI（sim_app、確定）

バイナリ名 `simpylink`。clap derive で実装。

| コマンド | 動作 | 終了コード |
|---|---|---|
| `simpylink` / `simpylink gui [FILE]` | GUI 起動（FILE は .json、M5以降 .slx も可） | 0 |
| `simpylink codegen <MODEL.json> -o <OUT.py>` | コンパイル + Python 生成。警告は stderr に `warning[CODE]: msg` | 0 成功 / 3 モデル・コンパイルエラー / 4 I/O エラー |
| `simpylink run <MODEL.json> --out-dir <DIR> [--python <PATH>] [--keep-script]` | 生成 → `<DIR>/<name>.py` に保存 → 実行 → 結果確認 | 0 成功 / 1 Python 実行失敗 / 3 モデルエラー / 4 I/O / 5 Python 環境不備 |
| `simpylink check-python [--python <PATH>]` | Python と numpy/scipy の確認結果を表示 | 0 OK / 5 不備 |

エラー出力形式（stderr、1行1件）：`error[E_CODE]: <message> (block <sid> "<name>")`。clap の引数エラーは clap 既定（終了コード 2）。

## 6. テストコマンド（全 Issue 共通）

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python -m pip install -r python/requirements.txt
python -m pytest python/tests -q
```

## 7. 設計原則

1. **決定性**：同じ Model からは常にバイト単位で同じ JSON・同じ Python を生成する（HashMap の反復順に依存しない。`BTreeMap` / ソート済み `Vec` を使う）。
2. **純粋ロジックの分離**：GUI 操作は `canvas::controller`（egui 非依存）でテストし、egui 層は入力変換と描画のみ。
3. **エラーを握りつぶさない**：`unwrap()` / `expect()` はテストコード以外禁止（`main` での最終報告を除く）。
4. **パニック禁止**：ライブラリ関数は `Result` を返す。
5. **Simulink 名称の踏襲**：ブロック型名・パラメータ名は Simulink の `BlockType` / パラメータ名（例 `Gain`, `Sum`, `Inputs`）を使う。

## 8. 参照
- 旧実装：コミット `3d77194`（破棄、#16 参照）


---
_Generated by [Claude Code](https://claude.ai/code)_
