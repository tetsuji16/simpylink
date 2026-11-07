> Source: https://github.com/tetsuji16/simpylink/issues/21 (copied on 2026-11-07)

> 計画ID: `DES-05` ／ 親: #16 ／ Blocked by: #18

# [DES-05] 設計契約：キャンバス座標系・操作状態機械・Simulink R2026b 操作対応表

> **設計契約**。GUI は (1) 入力変換 (2) 純粋な状態機械 (3) モデル更新（Command）(4) 描画 に分離する。

## 1. レイヤー分割（sim_gui::canvas）

| モジュール | egui 依存 | 責務 | テスト |
|---|---|---|---|
| `view` | なし | `ViewTransform`（スクリーン⇔モデル座標） | 単体 |
| `geometry` | なし | ポート位置・ブロック図形 | 単体 |
| `route` | なし | 線の自動配線・折れ線・分岐点座標 | 単体 |
| `hit` | なし | ヒットテスト | 単体 |
| `controller` | なし | `CanvasController`：`CanvasEvent` → 状態遷移 → `Vec<CanvasAction>` | 単体（イベント列） |
| `widget` | あり | egui 入力 → `CanvasEvent` 変換、`Painter` による描画 | 手動 + スモーク |

egui 非依存モジュールは独自の型を使う：
```rust
#[derive(Debug, Clone, Copy, PartialEq)] pub struct ScreenPos { pub x: f32, pub y: f32 }   // キャンバス矩形左上が (0,0)
#[derive(Debug, Clone, Copy, PartialEq)] pub struct ScreenVec { pub x: f32, pub y: f32 }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)] pub struct Modifiers { pub ctrl: bool, pub shift: bool, pub alt: bool }
// ctrl は macOS では Command キー（egui の Modifiers::command を使う）
```

## 2. 座標系（`view`）

- モデル座標：`sim_core::geom::Point`（f64、Simulink の Position と同じピクセル単位、y 下向き）。
- スクリーン座標：`ScreenPos`（f32、キャンバス左上原点の論理ピクセル）。
```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewTransform { pub offset: ScreenVec /* モデル原点のスクリーン位置 */, pub zoom: f32 /* 1.0 = 100% */ }
impl ViewTransform {
    pub const MIN_ZOOM: f32 = 0.1;  pub const MAX_ZOOM: f32 = 10.0;
    pub fn identity() -> Self;                                   // offset (0,0), zoom 1.0
    pub fn to_screen(&self, p: Point) -> ScreenPos;              // s = offset + p*zoom
    pub fn to_model(&self, s: ScreenPos) -> Point;               // p = (s - offset)/zoom
    pub fn screen_delta_to_model(&self, v: ScreenVec) -> (f64, f64);  // v/zoom
    pub fn zoom_about(&mut self, anchor: ScreenPos, factor: f32);     // anchor 下のモデル点を固定してズーム、範囲クランプ
    pub fn pan(&mut self, v: ScreenVec);                         // offset += v
    pub fn fit(&mut self, bounds: Rect, viewport: ScreenVec, margin: f32);  // bounds を viewport 中央に収める。zoom は min(…,1.0) を上限とせず範囲クランプのみ
}
```
`zoom_about`：`m = to_model(anchor)`、`zoom = clamp(zoom*factor)`、`offset = anchor - m*zoom`。

## 3. ブロック形状とポート位置（`geometry`、Orientation::Right）

- 入力ポート i（1..=n）：`(left, top + h*(2i-1)/(2n))`、出力ポート j（1..=m）：`(right, top + h*(2j-1)/(2m))`。h = bottom-top。
- ポート描画：外向き三角形（シェブロン）、モデル座標で幅 6・高さ 8。
- ポートのヒット半径：**スクリーン 8px**（ズームに依存しない）。

## 4. ヒットテスト優先順位（`hit`）

`hit_test(system, view, pos) -> HitTarget`：
1. 出力ポート（距離 <= 8px の最も近いもの、同距離なら sid 大きい方 = 上に描画されている方）
2. 入力ポート（同上）
3. ブロック本体（Rect 内、sid の大きい順）
4. ライン／分岐の線分（距離 <= 5px、最も近いもの）
5. `HitTarget::Empty`
```rust
pub enum HitTarget { OutputPort(PortRef), InputPort(PortRef), Block(BlockId), Line { line: LineId, branch: Vec<usize> /* 分岐パス */, segment: u32, t: f64 }, Empty }
```

## 5. 操作状態機械（`controller`）

```rust
pub enum CanvasEvent {
    PointerDown { pos: ScreenPos, button: PointerButton, mods: Modifiers },
    PointerMove { pos: ScreenPos, mods: Modifiers },
    PointerUp { pos: ScreenPos, button: PointerButton, mods: Modifiers },
    DoubleClick { pos: ScreenPos, mods: Modifiers },
    Scroll { pos: ScreenPos, delta: ScreenVec, mods: Modifiers },
    Key { key: CanvasKey, pressed: bool, mods: Modifiers },
    PointerLeft,
    DropBlock { pos: ScreenPos, block_type: String },     // ライブラリからのドロップ
}
pub enum PointerButton { Primary, Secondary, Middle }
pub enum CanvasKey { Delete, Backspace, Escape, Space, ArrowLeft, ArrowRight, ArrowUp, ArrowDown, A, Z, Y, C, X, V, D, Plus, Minus, Zero, Enter }

enum Interaction {                      // private
    Idle,
    Pressed { start: ScreenPos, target: HitTarget, button: PointerButton, mods: Modifiers },
    DraggingBlocks { start_model: Point, blocks: Vec<BlockId>, copy: bool, axis_lock: Option<Axis> },
    RubberBand { start: ScreenPos, current: ScreenPos, additive: bool },
    DrawingLine { from: LineStart, current: ScreenPos },
    Panning { last: ScreenPos },
    SpaceHeld { used_for_pan: bool },
}
pub enum LineStart { Output(PortRef), Input(PortRef), Branch { line: LineId, branch: Vec<usize>, tap: TapPoint } }

pub enum CanvasAction {
    Execute(Command),                   // History::execute に渡す
    OpenBlockDialog(BlockId),
    Undo, Redo, Copy, Cut, Paste { at: Point },
    DuplicateBlocks { blocks: Vec<BlockId>, dx: f64, dy: f64 },   // Ctrl+ドラッグ / 右ドラッグ
    FitToView,
    QuickInsert { at: Point },          // #54
    Status(String),                     // ステータスバー表示（例 "Input port already connected"）
}
pub struct CanvasController { pub view: ViewTransform, pub selection: Selection, pub viewport: ScreenVec, pub last_pointer: Option<ScreenPos>, /* private interaction */ }
pub struct Selection { pub blocks: BTreeSet<BlockId>, pub lines: BTreeSet<LineId> }
impl CanvasController {
    pub fn handle(&mut self, ev: CanvasEvent, model: &Model, path: &SystemPath) -> Vec<CanvasAction>;
    pub fn preview(&self) -> CanvasPreview;  // 描画用：ドラッグ中オフセット、ラバーバンド矩形、描画中の線
}
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CanvasPreview {
    pub drag: Option<(Vec<BlockId>, f64, f64)>,   // 移動中ブロックとモデル座標オフセット
    pub rubber_band: Option<Rect>,                // モデル座標
    pub drawing_line: Option<(Point, Point)>,     // 始点（ポート/分岐点）と現在位置（モデル座標）
}
```

### 5.1 遷移表（DRAG_THRESHOLD = 4px）

| 現状態 | イベント | 条件 | 次状態 | アクション |
|---|---|---|---|---|
| Idle | Down Primary | target=OutputPort | Pressed | – |
| Idle | Down Primary | target=Block、Ctrl なし | Pressed | 未選択なら：Shift なし→選択をそのブロックのみに、Shift→追加。選択済みなら維持（Shift→解除） |
| Idle | Down Primary | target=Block、Ctrl あり | Pressed | 選択を変更しない（Up 時に Ctrl+クリック接続、ドラッグならコピー） |
| Idle | Down Primary | target=Line | Pressed | 選択をそのラインのみ（Shift→トグル） |
| Idle | Down Primary | target=Empty | Pressed | Shift なしなら選択解除 |
| Idle | Down Middle | any | Panning | – |
| Idle | Down Secondary | target=Block | Pressed | （右ドラッグ＝コピー） |
| Idle | Down Secondary | target=Line | Pressed | （右ドラッグ＝分岐） |
| Pressed | Move | 移動 < 4px | Pressed | – |
| Pressed | Move ≥4px | OutputPort | DrawingLine{Output} | – |
| Pressed | Move ≥4px | InputPort（未接続） | DrawingLine{Input} | – |
| Pressed | Move ≥4px | Block、Primary、Ctrl なし | DraggingBlocks{copy:false} | – |
| Pressed | Move ≥4px | Block、Ctrl または Secondary | DraggingBlocks{copy:true} | – |
| Pressed | Move ≥4px | Line、Ctrl+Primary または Secondary | DrawingLine{Branch} | – |
| Pressed | Move ≥4px | Empty | RubberBand{additive: Shift} | – |
| Pressed | Up | Block、Ctrl、既存選択1ブロック A≠B | Idle | **Ctrl+クリック接続**：A の最初の未使用出力 → B の最初の未接続入力へ AddLine |
| Pressed | Up | その他 | Idle | – |
| DraggingBlocks | Move | Shift 押下（開始後） | 同 | axis_lock = 移動量の大きい軸 |
| DraggingBlocks | Up | copy=false | Idle | Execute(MoveBlocks{dx,dy})（0,0 なら何もしない）。dx,dy は四捨五入で整数化 |
| DraggingBlocks | Up | copy=true | Idle | DuplicateBlocks{blocks, dx, dy}（アプリ側がクリップボード機能で複製、#52） |
| RubberBand | Up | – | Idle | 矩形と交差するブロックを選択（additive なら追加） |
| DrawingLine | Up | 入力ポート上（未接続）、またはブロック上で最初の未接続入力がある | Idle | Execute(AddLine) または Execute(ReplaceLine)（分岐追加） |
| DrawingLine | Up | 接続済み入力ポート上 | Idle | Status("Input port already connected") |
| DrawingLine | Up | その他 | Idle | 何もしない（未接続線は M6 #98） |
| 任意 | Key Escape | ドラッグ系状態 | Idle | 取消（モデル変更なし） |
| 任意 | PointerLeft | ドラッグ系状態 | 維持 | – |
| Idle | DoubleClick | Block | Idle | OpenBlockDialog |
| Idle | DoubleClick | Empty | Idle | QuickInsert{at} |
| Idle | Scroll | mods なし | Idle | zoom_about(pos, 1.0015^delta.y) |
| Idle | Scroll | Ctrl | Idle | pan(0, delta.y)（縦パン） |
| Idle | Scroll | Shift | Idle | pan(delta.y, 0)（横パン） |
| Idle | Key Space down | – | SpaceHeld{false} | – |
| SpaceHeld | Down Primary | – | Panning | used_for_pan = true |
| SpaceHeld | Key Space up | used_for_pan=false | Idle | FitToView |
| Panning | Move | – | Panning | pan(pos-last) |
| Panning | Up | – | Idle（Space 押下中なら SpaceHeld{true}） | – |

### 5.2 キー（Idle 時、Ctrl は macOS で Cmd）
| キー | アクション |
|---|---|
| Delete / Backspace | Execute(DeleteItems{選択ブロック, 選択ライン})、選択クリア |
| Ctrl+Z | Undo |
| Ctrl+Y / Ctrl+Shift+Z | Redo |
| Ctrl+A | 全ブロック・全ライン選択 |
| Ctrl+Alt+A | 選択解除 |
| Ctrl+C / Ctrl+X / Ctrl+V | Copy / Cut / Paste{at: 最後のポインタ位置のモデル座標} |
| 矢印キー | 選択ブロックを 1 移動（MoveBlocks）。Simulink の「selection mode」相当 |
| Ctrl+Plus / Ctrl+Minus | キャンバス中央を基準に ×1.25 / ÷1.25 |
| Ctrl+0 | zoom = 1.0（キャンバス中央基準） |

アプリ全体のショートカット（Ctrl+N/O/S、Ctrl+T/F5 Run、Ctrl+Shift+T Stop、Ctrl+D Update、Ctrl+E 設定、Ctrl+Shift+L ライブラリ）は `SimpylinkApp` が処理する（#50, #67）。

## 6. 描画規約（`widget`）
| 要素 | 仕様 |
|---|---|
| 背景 | 白 `#FFFFFF`、グリッドなし |
| ブロック | 白塗り、枠 黒 1px（×zoom、最小1px）。選択時 枠 `#0072BD` 2px |
| ブロック名 | ブロック下 4px、中央揃え、12px×zoom（最小 8px で非表示しない） |
| アイコン | Constant=値文字列、Gain=三角形+値、Sum=円（round）/矩形+符号、Step=階段線、Scope=小画面、Integrator=`1/s`、その他=display_name |
| ライン | 黒 1px、宛先に塗り三角矢印（長さ8・幅6 モデル単位×zoom）。選択時 `#0072BD` 2px |
| 分岐点 | 半径 2.5px の黒丸 |
| 描画中の線 | 破線 `#0072BD` |
| ラバーバンド | 枠 `#0072BD` 1px、塗り `#0072BD` 透明度 10% |

## 7. Simulink R2026b 操作対応表（出典：https://www.mathworks.com/help/simulink/ug/summary-of-mouse-and-keyboard-actions.html 、R2026b 表示を 2026-11-06 に確認）

状態：**実装予定**（Issue 番号）／**部分**（差異あり、内容記載）／**対象外**（理由記載）。実機での操作感比較は未検証。

| Simulink 操作 | R2026b の操作 | simpylink | Issue |
|---|---|---|---|
| モデル新規/開く/保存 | Ctrl+N / Ctrl+O / Ctrl+S | 実装予定 | #53 |
| 切り取り/コピー/貼り付け | Ctrl+X / C / V | 実装予定 | #52 |
| 元に戻す/やり直し | Ctrl+Z / Ctrl+Y（Online: Ctrl+Shift+Z） | 実装予定（両方） | #42, #48 |
| 全体表示 | Space | 実装予定 | #48 |
| ズームイン/アウト/100% | Ctrl+Plus / Ctrl+Minus / Ctrl+0 | 実装予定（Alt+1, Space+0 は部分：未対応） | #48 |
| ホイールでズーム | スクロールホイール | 実装予定（係数は独自） | #48 |
| パン | 中ボタンドラッグ、Space+ドラッグ | 実装予定 | #48 |
| 縦/横パン | Ctrl+スクロール / Shift+スクロール | 実装予定 | #48 |
| 全選択 / 選択解除 | Ctrl+A / Ctrl+Alt+A | 実装予定 | #48 |
| 範囲選択 | 空白からドラッグ、Shift+ドラッグで追加 | 実装予定 | #46 |
| Shift+クリックで追加選択 | Shift+click | 実装予定 | #46 |
| 移動 | ドラッグ、開始後 Shift で軸固定 | 実装予定 | #46 |
| キーボード移動 | 矢印キー（selection mode） | 部分：M キーのモード切替なし、常に移動 | #48 |
| ブロックのコピー | Ctrl+ドラッグ、右ドラッグ | 実装予定 | #46 |
| 削除 | Delete / Backspace | 実装予定 | #48 |
| クイック挿入 | キャンバスをダブルクリックして名前入力、Ctrl+Period×2 | 実装予定（ダブルクリックのみ） | #54 |
| ライブラリブラウザ | Ctrl+Shift+L | 実装予定（左パネル表示切替） | #50 |
| ブロックパラメータ | ダブルクリック | 実装予定 | #51 |
| 接続（ドラッグ） | ポートからポートへドラッグ | 実装予定 | #47 |
| 接続（Ctrl+クリック） | 始点ブロック選択後、終点ブロックを Ctrl+クリック | 実装予定 | #47 |
| 接続（青シェブロン） | ポートをクリックし候補シェブロンをクリック | 対象外（M6 で検討） | #98 |
| 連続接続 | Shift を押しながら接続 | 対象外（M6） | #98 |
| 分岐 | 線から Ctrl+ドラッグ、右ドラッグ | 実装予定 | #47 |
| 障害物回避配線 | 描画中 Shift+ドラッグ | 対象外（M6） | #98 |
| ブロック切り離し | Shift+ドラッグ | 対象外（M6） | #98 |
| 回転 / 反転 | Ctrl+R / Ctrl+Shift+R / Ctrl+I | 対象外（M6、Orientation は保存のみ） | #98 |
| サブシステム化 | Ctrl+G | 対象外（M6） | #91 |
| サブシステムに入る/親へ | Enter / Esc | 対象外（M6） | #91 |
| 信号名 | 信号をダブルクリック | 対象外（M6） | #98 |
| シミュレーション開始/停止 | Ctrl+T または F5 / Ctrl+Shift+T | 実装予定 | #67 |
| モデル更新 | Ctrl+D | 実装予定（コンパイルして診断表示） | #67 |
| コンフィギュレーションパラメータ | Ctrl+E | 実装予定 | #79 |
| 自動整列 | Ctrl+Shift+A | 対象外（M6） | – |
| マスク関連 | Ctrl+M / Ctrl+U | 対象外 | – |
| デバッグ（Step over 等） | F10 / F11 | 対象外 | – |


---
_Generated by [Claude Code](https://claude.ai/code)_
