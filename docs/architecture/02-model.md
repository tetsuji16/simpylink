> Source: https://github.com/tetsuji16/simpylink/issues/18 (copied on 2026-11-07)

> 計画ID: `DES-02` ／ 親: #16 ／ Blocked by: #17

# [DES-02] 設計契約：モデルデータ構造・JSON形式 v1・パラメータ・Undo/Redo・診断

> **設計契約**。sim_core の型はここに書いたフィールド名・serde 表現どおりに実装する。

## 1. ID・幾何・パス（`sim_core::ids`, `geom`, `path`）

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlockId(pub u32);   // Simulink の SID に相当。モデル全体（全階層）で一意。1 始まり。

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LineId(pub u32);    // モデル全体で一意。1 始まり。

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Point { pub x: f64, pub y: f64 }

/// Simulink の Position = [left, top, right, bottom]。y は下向き正。
/// JSON では配列 [l, t, r, b] として表現する（serde は from/into [f64;4] で実装）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect { pub left: f64, pub top: f64, pub right: f64, pub bottom: f64 }
impl Rect {
    pub fn new(left: f64, top: f64, right: f64, bottom: f64) -> Rect;
    pub fn from_min_size(min: Point, w: f64, h: f64) -> Rect;
    pub fn width(&self) -> f64;  pub fn height(&self) -> f64;
    pub fn center(&self) -> Point;
    pub fn contains(&self, p: Point) -> bool;          // 境界を含む（<=）
    pub fn translated(&self, dx: f64, dy: f64) -> Rect;
    pub fn intersects(&self, other: &Rect) -> bool;    // 境界接触も true
    pub fn normalized(a: Point, b: Point) -> Rect;     // 2点から min/max を取って作る
}

/// ルートから対象システムまでのサブシステムブロックの SID 列。ルート = 空。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SystemPath(pub Vec<BlockId>);
```

## 2. モデル（`sim_core::model`）

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Model {
    pub format: String,            // 常に "simpylink-model"
    pub format_version: u32,       // 常に 1
    pub name: String,              // Python 識別子化して生成ファイル名に使う
    pub config: SimConfig,
    pub next_sid: u32,             // 次に割り当てる SID（既存最大+1 以上）
    pub next_line_id: u32,
    pub root: System,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimConfig {
    pub start_time: f64,           // 既定 0.0
    pub stop_time: f64,            // 既定 10.0
    pub solver: SolverKind,        // 既定 Auto
    pub rel_tol: f64,              // 既定 1e-3
    pub abs_tol: f64,              // 既定 1e-6
    pub max_step: Option<f64>,     // 既定 None（= 無制限）
    pub output_step: f64,          // 既定 0.01。ログ出力時刻の間隔（#19）
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SolverKind { Auto, Ode45, Ode23, Ode15s }   // M6 で固定ステップ追加予定

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct System {
    pub blocks: Vec<Block>,        // 不変条件：sid 昇順。描画の重なり順も sid 昇順（後ろが上）
    pub lines: Vec<Line>,          // 不変条件：id 昇順
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub sid: BlockId,
    pub block_type: String,        // Simulink BlockType 名（"Gain" など）
    pub name: String,              // システム内で一意
    pub position: Rect,
    #[serde(default)]
    pub orientation: Orientation,  // 既定 Right
    pub params: BTreeMap<String, String>,   // 値は Simulink と同じく文字列（式）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<System>,    // SubSystem のみ（M6）
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation { #[default] Right, Left, Up, Down }   // M1 では Right のみ描画対応

/// 出力ポート / 入力ポートの参照。port は 1 始まり（Simulink と同じ）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PortRef { pub block: BlockId, pub port: u32 }

/// 親経路上の分岐点。segment = 親の描画折れ線の線分番号（0 始まり）、t = 線分上の比率 [0,1]。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TapPoint { pub segment: u32, pub t: f64 }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Line {
    pub id: LineId,
    pub src: PortRef,                    // 出力ポート
    #[serde(default)]
    pub points: Vec<Point>,              // 中間折れ点（絶対モデル座標）。空 = 自動配線
    #[serde(default)]
    pub dst: Option<PortRef>,            // 入力ポート（None なら branches のみ、または未接続）
    #[serde(default)]
    pub branches: Vec<Branch>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,                    // 信号名（M6 で編集 UI）
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Branch {
    pub tap: TapPoint,                   // 親（Line または親 Branch）の折れ線上の分岐位置
    #[serde(default)]
    pub points: Vec<Point>,
    #[serde(default)]
    pub dst: Option<PortRef>,
    #[serde(default)]
    pub branches: Vec<Branch>,
}
```

### 2.1 信号ネットの定義
1 本の `Line` は 1 つの出力ポート（`src`）から出る信号全体。`dst` と全 `branches`（再帰）の `dst` の集合がその信号の宛先集合。
- 入力ポートは **高々 1 つの宛先** にしかなれない（多重駆動はエラー `E_INPUT_MULTIPLY_DRIVEN`）。
- 1 つの出力ポートから複数の `Line` が出てもよい（Simulink でも可）。コンパイル時は宛先集合を合併する。

### 2.2 Model のメソッド（M0 で実装）
```rust
impl Model {
    pub fn new(name: &str) -> Model;                 // 既定 config、next_sid=1, next_line_id=1, 空の root
    pub fn alloc_sid(&mut self) -> BlockId;          // next_sid を返して +1
    pub fn alloc_line_id(&mut self) -> LineId;
    pub fn system(&self, path: &SystemPath) -> Option<&System>;
    pub fn system_mut(&mut self, path: &SystemPath) -> Option<&mut System>;
    pub fn max_sid(&self) -> u32;                    // 全階層の最大 SID（なければ 0）
    pub fn max_line_id(&self) -> u32;
}
impl System {
    pub fn block(&self, sid: BlockId) -> Option<&Block>;
    pub fn block_mut(&mut self, sid: BlockId) -> Option<&mut Block>;
    pub fn line(&self, id: LineId) -> Option<&Line>;
    pub fn line_mut(&mut self, id: LineId) -> Option<&mut Line>;
    pub fn insert_block_sorted(&mut self, b: Block);   // sid 昇順を保って挿入
    pub fn insert_line_sorted(&mut self, l: Line);
    pub fn block_by_name(&self, name: &str) -> Option<&Block>;
}
impl Line {
    pub fn destinations(&self) -> Vec<PortRef>;      // dst と全 branch の dst を深さ優先・宣言順で列挙
}
```

## 3. JSON 形式 v1（確定、`sim_core::json`）

> 追記（DES-07 §1・§2）：`variables`・固定ステップソルバー・`config.fixed_step` を使うモデルは `format_version: 2` で書く。読込は v1 と v2。使わないモデルは v1 のまま。

- serde_json の **pretty 出力（インデント2）** + 末尾改行 `\n`。キー順は構造体宣言順、`params` は BTreeMap 順。
- 読み込み時の検査：`format != "simpylink-model"` → `ModelIoError::WrongFormat`、`format_version != 1` → `ModelIoError::UnsupportedVersion(v)`。
- 読み込み後の補正：`next_sid <= max_sid()` の場合 `next_sid = max_sid()+1`（`next_line_id` 同様）。`blocks`/`lines` を sid/id 昇順にソート。
- 欠けているパラメータは補完しない（補完はブロック登録 `library::fill_default_params` が GUI 側で行う）。

```rust
#[derive(Debug, thiserror::Error)]
pub enum ModelIoError {
    #[error("I/O error: {0}")] Io(#[from] std::io::Error),
    #[error("JSON parse error at line {line}, column {column}: {message}")] Parse { line: usize, column: usize, message: String },
    #[error("not a simpylink model (format = {0:?})")] WrongFormat(String),
    #[error("unsupported format_version {0} (supported: 1)")] UnsupportedVersion(u32),
}
pub fn to_json_string(model: &Model) -> String;          // 失敗しない（f64 の NaN は事前検証で禁止）
pub fn from_json_str(s: &str) -> Result<Model, ModelIoError>;
pub fn save_to_path(model: &Model, path: &Path) -> Result<(), ModelIoError>;  // 一時ファイルに書いて rename（原子的保存）
pub fn load_from_path(path: &Path) -> Result<Model, ModelIoError>;
```

### 3.1 JSON サンプル（Step → Gain → Scope、`tests/fixtures/models/step_gain_scope.json`）
```json
{
  "format": "simpylink-model",
  "format_version": 1,
  "name": "step_gain_scope",
  "config": {
    "start_time": 0.0,
    "stop_time": 2.0,
    "solver": "auto",
    "rel_tol": 0.001,
    "abs_tol": 1e-6,
    "max_step": null,
    "output_step": 0.5
  },
  "next_sid": 4,
  "next_line_id": 3,
  "root": {
    "blocks": [
      { "sid": 1, "block_type": "Step", "name": "Step", "position": [50.0, 50.0, 80.0, 80.0], "orientation": "right",
        "params": { "After": "2", "Before": "0", "SampleTime": "0", "Time": "1" } },
      { "sid": 2, "block_type": "Gain", "name": "Gain", "position": [130.0, 50.0, 160.0, 80.0], "orientation": "right",
        "params": { "Gain": "3", "SampleTime": "-1" } },
      { "sid": 3, "block_type": "Scope", "name": "Scope", "position": [210.0, 49.0, 240.0, 81.0], "orientation": "right",
        "params": { "NumInputPorts": "1" } }
    ],
    "lines": [
      { "id": 1, "src": { "block": 1, "port": 1 }, "points": [], "dst": { "block": 2, "port": 1 }, "branches": [] },
      { "id": 2, "src": { "block": 2, "port": 1 }, "points": [], "dst": { "block": 3, "port": 1 }, "branches": [] }
    ]
  }
}
```
（上は可読性のため一部1行化。実際の出力は serde_json pretty 形式。`1e-6` は serde_json が `1e-6` と出力する。）

## 4. 数値式（`sim_core::expr`）

パラメータ値は文字列。数値として使うときに `parse_value` で評価する。

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Matrix { pub rows: usize, pub cols: usize, pub data: Vec<f64> }   // 行優先
#[derive(Debug, Clone, PartialEq)]
pub enum Value { Scalar(f64), Matrix(Matrix) }
impl Value {
    pub fn as_scalar(&self) -> Option<f64>;       // Scalar、または 1x1 Matrix
    pub fn as_vector(&self) -> Option<Vec<f64>>;  // Scalar→[v]、1xN または Nx1 Matrix→要素列、空 [] → 空 Vec
    pub fn as_matrix(&self) -> Matrix;            // Scalar→1x1
}
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{message} (at byte {pos})")]
pub struct ExprError { pub message: String, pub pos: usize }
pub fn parse_value(src: &str) -> Result<Value, ExprError>;
```

文法（MATLAB 数値リテラルの部分集合。変数参照は M6）：
```text
value   := ws expr ws EOF
expr    := term (('+' | '-') term)*
term    := unary (('*' | '/') unary)*
unary   := ('-' | '+') unary | power
power   := primary ('^' unary)?
primary := NUMBER | 'pi' | 'inf' | 'Inf' | 'eps' | 'NaN' は禁止 | '(' expr ')' | matrix
matrix  := '[' rows? ']'
rows    := row (';' row)*
row     := expr ((',' | WS_SEP) expr)*
NUMBER  := [0-9]+ ('.' [0-9]*)? ([eE] [+-]? [0-9]+)? | '.' [0-9]+ ([eE] [+-]? [0-9]+)?
```
- 行列内の区切り規則：`[1 -2]` は 2 要素、`[1 - 2]` は 1 要素（-1）、`[1 -2+3]` は 2 要素（1, 1）。判定：行列内で空白の直後に `+`/`-` があり、その直後が空白でない場合、新しい要素の開始とみなす。
- 行列の要素は Scalar でなければならない（入れ子行列はエラー）。各行の要素数が異なればエラー。
- 行列同士・行列とスカラーの演算はエラー（`"matrix arithmetic is not supported"`）。
- 結果が NaN になる式（`inf-inf`）はエラー。`NaN` 識別子もエラー。未知の識別子 → `"unknown identifier 'K'"`。

## 5. ブロック登録（`sim_core::library`）

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    Expr { shape: Shape },          // parse_value で評価できること + 形状条件
    SampleTime,                     // "-1" | "0" | "inf" | 正のスカラー | "[Ts, offset]"（0<=offset<Ts）
    SumSigns,                       // Sum の Inputs："|", "+", "-" のみの並び（+/- が1個以上）または正整数 "3"
    Enum(&'static [&'static str]),  // 列挙値のいずれか（完全一致）
    PositiveInt,                    // "1" 以上の整数
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape { Scalar, Vector, Matrix }
pub struct ParamSpec { pub name: &'static str, pub label: &'static str, pub default: &'static str, pub kind: ParamKind }
pub struct BlockSpec {
    pub block_type: &'static str,
    pub display_name: &'static str,     // ライブラリ表示名（"Transfer Fcn" 等）
    pub library: &'static str,          // "Sources" | "Sinks" | "Math Operations" | "Continuous" | "Discrete" | "Ports & Subsystems"
    pub default_size: (f64, f64),       // (幅, 高さ)
    pub params: &'static [ParamSpec],
    pub ports: fn(&BTreeMap<String, String>) -> Result<PortCounts, String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortCounts { pub inputs: u32, pub outputs: u32 }
pub fn registry() -> &'static [BlockSpec];
pub fn spec(block_type: &str) -> Option<&'static BlockSpec>;
pub fn port_counts(block: &Block) -> Result<PortCounts, String>;
pub fn validate_param(kind: ParamKind, value: &str) -> Result<(), String>;
pub fn fill_default_params(block: &mut Block);   // 欠けた param を default で補完
pub fn unique_block_name(system: &System, base: &str) -> String;  // "Gain","Gain1","Gain2",...
pub fn new_block(system: &System, block_type: &str, sid: BlockId, top_left: Point) -> Result<Block, String>;
```

M0 で登録するブロック（後続 Issue が追加する）：

| block_type | display | library | size | params（name=default） | ports (in/out) |
|---|---|---|---|---|---|
| Constant | Constant | Sources | 30x30 | Value="1"(Expr Scalar), SampleTime="inf"(SampleTime) | 0/1 |
| Step | Step | Sources | 30x30 | Time="1", Before="0", After="1"(Expr Scalar), SampleTime="0" | 0/1 |
| Gain | Gain | Math Operations | 30x30 | Gain="1"(Expr Scalar), SampleTime="-1" | 1/1 |
| Sum | Sum | Math Operations | 20x20 | Inputs="\|++"(SumSigns), IconShape="round"(Enum round/rectangular), SampleTime="-1" | 符号数/1 |
| Scope | Scope | Sinks | 30x32 | NumInputPorts="1"(PositiveInt) | N/0 |
| Inport | In1 | Ports & Subsystems | 30x14 | Port="1"(PositiveInt) | 0/1 |
| Outport | Out1 | Ports & Subsystems | 30x14 | Port="1"(PositiveInt) | 1/0 |

Sum の入力数：`Inputs` が整数文字列 `"n"` なら n 個（全て +）、それ以外は `+` と `-` の個数（`|` は数えない）。

## 6. 診断（`sim_core::diag`）

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity { Error, Warning, Info }
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Location { pub path: SystemPath, pub block: Option<BlockId>, pub line: Option<LineId> }
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic { pub severity: Severity, pub code: String, pub message: String, pub location: Location }
impl Diagnostic { pub fn error(code: &str, msg: impl Into<String>, loc: Location) -> Self; pub fn warning(..) -> Self; }
pub fn has_errors(diags: &[Diagnostic]) -> bool;
pub mod codes { pub const E_DUPLICATE_SID: &str = "E_DUPLICATE_SID"; /* 下表すべて */ }
```

| コード | 重大度 | 発行元 | 条件 |
|---|---|---|---|
| E_DUPLICATE_SID | Error | validate | 全階層で SID 重複 |
| E_DUPLICATE_LINE_ID | Error | validate | LineId 重複 |
| E_DUPLICATE_BLOCK_NAME | Error | validate | 同一システム内でブロック名重複 |
| E_UNKNOWN_BLOCK_TYPE | Error | validate | registry に無い block_type |
| E_INVALID_PARAM | Error | validate | validate_param 失敗／必須 param 欠落 |
| E_LINE_SRC_MISSING | Error | validate | src.block がシステム内に無い |
| E_LINE_DST_MISSING | Error | validate | dst.block がシステム内に無い |
| E_PORT_OUT_OF_RANGE | Error | validate | port が 0 または ポート数超過 |
| E_INPUT_MULTIPLY_DRIVEN | Error | validate | 同じ入力ポートが2回以上宛先 |
| E_INVALID_CONFIG | Error | validate | stop<=start、output_step<=0、rel_tol/abs_tol<=0、max_step<=0、非有限値 |
| W_UNCONNECTED_INPUT | Warning | validate | 入力ポートに宛先なし（計算時 0.0） |
| W_UNCONNECTED_OUTPUT | Warning | validate | 出力ポートから Line なし |
| W_DANGLING_LINE | Warning | validate | Line/Branch が dst も branches も持たない |
| E_PARAM_EVAL / E_PARAM_DOMAIN / E_ALGEBRAIC_LOOP / E_SAMPLE_TIME / E_UNSUPPORTED_BLOCK / E_UNSUPPORTED_FEATURE | Error | sim_ir | #19 参照 |

`validate_model(model: &Model) -> Vec<Diagnostic>`：上表の validate 分を、**ルートから深さ優先、ブロック sid 昇順 → ライン id 昇順** の決定的な順序で返す。

## 7. 編集コマンドと Undo/Redo（`sim_core::edit`）

全ての GUI からのモデル変更は `Command` 経由。`apply` は変更を行い、**逆コマンド** を返す。

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    AddBlock { path: SystemPath, block: Block },
    DeleteItems { path: SystemPath, blocks: Vec<BlockId>, lines: Vec<LineId> },
    RestoreItems { path: SystemPath, blocks: Vec<Block>, lines: Vec<Line>, modified_lines: Vec<Line> },
    MoveBlocks { path: SystemPath, blocks: Vec<BlockId>, dx: f64, dy: f64 },
    SetBlockPosition { path: SystemPath, block: BlockId, position: Rect },
    SetParams { path: SystemPath, block: BlockId, params: Vec<(String, Option<String>)> }, // None = キー削除
    RenameBlock { path: SystemPath, block: BlockId, name: String },
    AddLine { path: SystemPath, line: Line },
    RemoveLine { path: SystemPath, line: LineId },
    ReplaceLine { path: SystemPath, line: Line },
    SetConfig { config: SimConfig },
    Batch { label: String, commands: Vec<Command> },
}
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EditError {
    #[error("system not found")] SystemNotFound,
    #[error("block {0:?} not found")] BlockNotFound(BlockId),
    #[error("line {0:?} not found")] LineNotFound(LineId),
    #[error("block {0:?} already exists")] DuplicateBlock(BlockId),
    #[error("line {0:?} already exists")] DuplicateLine(LineId),
    #[error("block name {0:?} already used")] DuplicateName(String),
    #[error("input port {0:?} is already connected")] InputAlreadyConnected(PortRef),
}
pub fn apply(model: &mut Model, cmd: Command) -> Result<Command, EditError>;
```

ID カウンタ規則：`AddBlock` / `RestoreItems` は `model.next_sid = max(next_sid, 追加した最大sid+1)`、`AddLine` / `RestoreItems` / `ReplaceLine` は `next_line_id` を同様に引き上げる。**Undo しても下げない**（ID は再利用しない。Simulink の SID と同じ方針）。そのため Undo の同一性は `model.root` と `model.config` で判定する。

意味（逆コマンド）：
| コマンド | 処理 | 逆 |
|---|---|---|
| AddBlock | sid 重複・名前重複を検査して挿入 | DeleteItems{blocks:[sid], lines:[]} |
| DeleteItems | (1) 指定ラインを削除 (2) 指定ブロックを削除 (3) 削除ブロックが **src** のラインを丸ごと削除 (4) 削除ブロックが **dst** の分岐を剪定（#42 の剪定規則） | RestoreItems{削除したブロック, 丸ごと削除したライン, 剪定前の元ライン} |
| RestoreItems | ブロック・ラインを sorted 挿入、modified_lines で同 id を置換 | DeleteItems{blocks: 復元sid, lines: 復元lineのid} ※modified_lines は DeleteItems が再剪定で再現 |
| MoveBlocks | 各 position を平行移動。**指定ブロック同士を結ぶラインの points も平行移動** | MoveBlocks{同, -dx, -dy} |
| SetBlockPosition | 置換 | SetBlockPosition{旧} |
| SetParams | 各 (name, Some(v)) を設定、(name, None) はキー削除。同じ name が複数あれば後勝ち | SetParams{各要素の適用直前の値 (name, 旧値 Option) を **逆順** に並べたもの}。旧で未設定なら None |
| RenameBlock | 名前重複検査して変更 | RenameBlock{旧名} |
| AddLine | id 重複、src/dst のブロック存在、dst 入力ポート未接続を検査 | RemoveLine{id} |
| RemoveLine | 削除 | AddLine{削除したライン} |
| ReplaceLine | 同 id を置換 | ReplaceLine{旧} |
| SetConfig | 置換 | SetConfig{旧} |
| Batch | 先頭から順に apply。途中で失敗したら、それまでの逆を逆順に適用してロールバックし Err | Batch{同 label, 逆コマンドを逆順} |

```rust
pub struct History { /* private */ }
impl History {
    pub fn new(limit: usize) -> History;        // 既定 200
    pub fn execute(&mut self, model: &mut Model, cmd: Command) -> Result<(), EditError>;
    pub fn undo(&mut self, model: &mut Model) -> Result<bool, EditError>;  // 戻せたら true
    pub fn redo(&mut self, model: &mut Model) -> Result<bool, EditError>;
    pub fn can_undo(&self) -> bool; pub fn can_redo(&self) -> bool;
    pub fn undo_label(&self) -> Option<String>;   // メニュー表示用（Batch の label、その他は固定名）
    pub fn mark_saved(&mut self);
    pub fn is_dirty(&self) -> bool;
    pub fn clear(&mut self);                     // ファイルを開いたとき
}
```
`is_dirty` は「状態ID」で判定：各 execute で新しい状態ID（単調増加カウンタ）を割り当て、undo/redo でスタックに保存した状態IDへ戻す。`mark_saved` は現在の状態IDを記録。

## 8. クリップボード（`sim_core::clipboard`）

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipboardPayload { pub format: String /* "simpylink-clipboard" */, pub version: u32 /* 1 */, pub blocks: Vec<Block>, pub lines: Vec<Line> }
pub fn copy_selection(system: &System, blocks: &BTreeSet<BlockId>) -> ClipboardPayload;
pub fn paste_payload(model: &Model, path: &SystemPath, payload: &ClipboardPayload, offset: (f64, f64)) -> Result<(Command, Vec<BlockId>), EditError>;
```
詳細は #52。

## 9. 参照
- Simulink Position パラメータ（[left top right bottom]）：Common Block Properties — https://www.mathworks.com/help/simulink/slref/common-block-parameters.html


---
_Generated by [Claude Code](https://claude.ai/code)_
