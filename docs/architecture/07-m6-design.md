# DES-07 設計契約：M6 互換性拡張（変数・固定ステップ・ベクトル信号・ゼロクロッシング・ブロック拡充）と M5/M7 の残り

> 対象 Issue：#81, #86, #87, #88, #91, #92, #94, #95, #99, #100（親 Issue）。実装は各 Sub-issue で行う。
> 本書の期待値は設計上の定義または解析解であり、**Simulink R2026b 実機での一致は未検証**（ユーザーは Simulink を持たない。#101 は保留）。
> 実ファイルの観察は公開 SLX コーパス（#82 のコメント、#143）に基づく。
> 骨格（データ型・公開 API の追加）は PR「design: M6 skeleton」で main に入っている。§ごとに「骨格で入ったもの」を明記する。

## 0. 優先順位（公開 SLX 226 件での影響から決定）

| 順 | 項目 | 根拠 |
|---|---|---|
| 1 | §1 モデル変数 | 未定義変数で失敗：91 件 |
| 2 | §5 ブロック拡充（スカラー） | 未対応ブロックが無いファイルは 16 件のみ |
| 3 | §2 固定ステップソルバー | ソルバー指定 138 件中 44 件が固定ステップ |
| 4 | §3 ベクトル信号 | Mux 69 件 / Demux 104 件 |
| 5 | §4 ゼロクロッシング | Saturate/Switch の精度。P2 |
| 6 | §6〜§8 | Subsystem 残り・SLX 残り・配布 |

## 1. モデル変数（#94）

### 1.1 データ（骨格で実装済み）
- `Model::variables: BTreeMap<String, String>`（名前 → 数値式）。空なら JSON に出力しない。
- 変数名：`is_valid_variable_name`。`[A-Za-z][A-Za-z0-9_]*` を `.` でつないだもの（`az.Umax` は **フラットなキー**。構造体型は持たない）。予約名 `RESERVED_NAMES`（`pi`, `inf`, `Inf`, `eps`, `NaN`, `nan`, `true`, `false`）は不可。
- JSON 形式：変数・固定ステップのどちらかを使うモデルは `format_version: 2` で書く（`Model::required_format_version`）。それ以外は v1 のまま（既存ゴールデンを変えないため）。読込は 1 と 2 を受け付ける。
- `sim_core::workspace::Workspace`（評価済み変数）、`sim_core::expr::parse_value_in(src, &ws)`、`sim_ir::lower::lower_block_in(.., &ws)`、診断コード `E_VARIABLE`。
- 骨格の `Workspace::from_definitions` は **空でない定義を `"model variables are not supported yet"` で拒否** する（未実装のまま誤った値で計算しないため）。

### 1.2 式言語の拡張（`sim_core::expr`）
| 追加 | 仕様 |
|---|---|
| 変数参照 | 識別子は `ws` を最優先で引く（骨格で実装済み）。予約名は変数にできないので組込み定数と衝突しない |
| 構造体フィールド | 字句解析で `Ident '.' Ident`（`.` の直後が英字）を1つの識別子 `a.b` にまとめる。`.*` `./` `.^` は演算子 |
| 要素ごと演算子 | `.*` `./` `.^` はスカラー同士では `*` `/` `^` と同じ。行列が絡むと従来通り `matrix arithmetic is not supported` |
| 転置 | 後置 `'`：行列値の転置。スカラーはそのまま |
| 関数（引数はスカラー、戻りはスカラー） | 1 引数：`sin cos tan asin acos atan sinh cosh tanh exp log log10 log2 sqrt abs sign floor ceil round fix`。2 引数：`atan2 mod rem min max hypot power` |
| 関数のエラー | 未知の関数 `unknown function 'foo'`、引数の数 `function 'atan2' expects 2 arguments`、行列引数 `function 'sin' expects scalar arguments`、変数の後ろに `(` → `indexing is not supported` |
| 依存の列挙 | `pub fn identifiers(src: &str) -> Result<BTreeSet<String>, ExprError>`：関数名（直後が `(`）と組込み定数を除いた識別子の集合 |

`mod(a,b)` = MATLAB の mod（`a - floor(a/b)*b`、b=0 なら a）。`rem(a,b)` = `a - fix(a/b)*b`（b=0 なら a）。`round` は 0.5 を 0 から遠ざける方向（MATLAB と同じ。Rust の `f64::round` と同じ）。NaN になる結果は従来通りエラー。

### 1.3 定義の評価（`Workspace::from_definitions`）
1. 名前が不正なものを `invalid variable name` で報告。
2. 各定義の `identifiers()` を求め、定義内の名前への依存グラフを作る（定義に無い名前は依存とみなさない＝評価時に unknown identifier になる）。
3. Kahn 法（名前の辞書順で決定的）で評価順を決め、順に `parse_value_in`。
4. 循環に入った変数はすべて `cyclic definition (a -> b -> a)`（循環の最小名から辿る）。循環に依存する変数は `depends on an invalid variable 'a'`。
5. 評価エラーは `VariableError{name, message: ExprError の表示}`。エラーは名前順。1つでもエラーがあれば `Err`。

### 1.4 評価を使う箇所
| 箇所 | 変更 |
|---|---|
| `sim_core::validate::validate_model` | 先頭で `Workspace::from_definitions`。失敗時は各エラーを `E_VARIABLE`（Location::default）として出し、以降のパラメータ検証は空の ws で行う |
| `sim_core::library::validate_param` | `validate_param_in(kind, value, &ws)` を追加、`validate_param` は空 ws で呼ぶ |
| サンプル時間 | `parse_sample_time_value_in(src, &ws)` を追加（`Ts` などの変数を許す） |
| `sim_ir::compile` | 骨格で `lower_block_in` に ws を渡す配線済み |
| GUI パラメータダイアログ | 入力検証に文書のモデルの ws を使う |

### 1.5 生成 Python
#94 の旧方針「パラメータ定数は変数を参照する式として出す」は **取りやめ**（式を Python に翻訳する必要があり、読みやすさの利点より不一致の危険が大きい）。代わりに：
- `SimIr::variables: Vec<IrVariable { name: String, expr: String, value: Value }>`（名前順）を追加。
- 「Simulation settings」節の直後に「Model variables」節を出す（変数が無ければ節ごと出さない＝既存ゴールデン不変）：
```python
# ===========================================================================
# Model variables (evaluated by simpylink; informational, parameters below already use these values)
# ===========================================================================
MODEL_VARIABLES = {
    "Kp": 2.0,  # Kp = 2
    "az.Umax": 1.5,  # az.Umax = 3/2
}
```
行列値は `[[1.0, 2.0], [3.0, 4.0]]`（リストのリスト）。

### 1.6 GUI
- メニュー MODELING → 「Model Variables…」（ツールストリップ MODELING タブにボタン）。表（Name / Expression / Value or error）、追加・削除・編集、OK/Cancel/Apply。
- 変更は `Command::SetVariables { variables: BTreeMap<String,String> }`（全置換。逆コマンドは旧マップ）。Undo の同一性判定に `model.variables` を含める。
- 変数の評価エラーは Diagnostics パネルに `E_VARIABLE` として出る（compile 経由）。

### 1.7 外部定義の取り込み（.m の部分集合）
- 対象：1 行ずつ、`%` 以降はコメント、`;` または `,` で区切られた複数文、各文が `NAME = EXPR`（NAME は §1.1 の名前）。それ以外の文（`if`、関数呼び出しだけの行、`load` など）は **読み飛ばして行番号を報告**。
- `pub fn parse_m_assignments(src: &str) -> MImport { assignments: Vec<(String, String)>, skipped: Vec<(usize /*1-based line*/, String)> }`（`sim_core::workspace`）。同じ名前の再代入は後勝ち。式は文字列のまま保存（評価は §1.3）。
- GUI：「Model Variables…」ダイアログに「Import from .m…」ボタン。
- SLX：`blockDiagram.json`（R2026b）／`blockdiagram.xml` の `PreLoadFcn`・`InitFcn` が **単一の識別子**（スクリプト名）なら、SLX と同じフォルダの `<名前>.m` を探し、あれば取り込んで `ImportReport.info` に記録。無ければ `warnings` に「InitFcn script 'X.m' not found next to the model; variables may be undefined」。`modelWorkspace.mxarray`（MAT 形式）は対象外（`warnings` に記録）。

## 2. 固定ステップソルバー（#99）

### 2.1 データ（骨格で実装済み）
- `SolverKind::{FixedDiscrete, Ode1, Ode2, Ode3, Ode4, Ode5}`、`is_fixed_step()`、`simulink_name()`、`SimConfig::fixed_step: Option<f64>`（None = auto）、`IrConfig::fixed_step`。
- `solver_method()`：`"discrete" | "ode1" … "ode5"`。
- 骨格の compile は固定ステップを `E_UNSUPPORTED_FEATURE`（`solver '…' is not implemented yet`）で拒否する。

### 2.2 刻み幅の決定（compile）
- `fixed_step = Some(h)`：`h > 0` かつ `h <= stop - start`、でなければ `E_INVALID_CONFIG`。
- `None`（auto）：離散レートがあれば最小周期 `h = min(period)`、無ければ `h = (stop - start) / 50`（Simulink の既定の考え方。未検証）。
- すべての離散レートの period と offset は h の整数倍（相対許容 1e-9）でなければ `E_SAMPLE_TIME`（`"sample time [Ts, off] of 'X' is not a multiple of the fixed step h"`）。
- `FixedDiscrete` で連続状態が 1 つ以上 → `E_INVALID_CONFIG`（`"the discrete solver cannot integrate continuous states; choose ode1..ode5"`）。
- 決まった h を `IrConfig::fixed_step = Some(h)` に入れる（auto でも具体値）。

### 2.3 実行（固定ランタイムの改訂）
- 生成ファイルの設定節に `FIXED_STEP = <h>`（可変ステップは `FIXED_STEP = None`）を **常に** 出す。→ 全ゴールデン（`tests/fixtures/expected/*.py`）に1行増える（設計変更による更新であり、期待値の根拠は本節）。
- `major_time_points()`：`FIXED_STEP` が None でなければ、`{ round(START + k*h, 12) }` の点を `"step"` 種別として追加（出力時刻とは別。出力時刻・ブレークポイント・離散ヒットはこれまで通り）。
- `simulate()` の積分部分：`SOLVER in FIXED_METHODS`（`("ode1","ode2","ode3","ode4","ode5")`）なら `solve_ivp` の代わりに `x = fixed_step(f, t, t_next, x, SOLVER)`：区間長 `H = t_next - t` を1ステップとして Butcher 表で進める（主要時刻の間隔は h 以下なので、1 区間 = 1 ステップ）。`"discrete"` は積分しない。
- Butcher 表（係数はランタイムに定数として持つ）：

| method | c | A（下三角、行ごと） | b |
|---|---|---|---|
| ode1 | [0] | – | [1] |
| ode2（Heun） | [0, 1] | [[1]] | [1/2, 1/2] |
| ode3（Bogacki–Shampine 3 次） | [0, 1/2, 3/4, 1] | [[1/2],[0, 3/4],[2/9, 1/3, 4/9]] | [2/9, 1/3, 4/9, 0] |
| ode4（古典 RK4） | [0, 1/2, 1/2, 1] | [[1/2],[0,1/2],[0,0,1]] | [1/6, 1/3, 1/3, 1/6] |
| ode5（Dormand–Prince 5 次） | [0, 1/5, 3/10, 4/5, 8/9, 1, 1] | 標準 DP 係数（表 2.3a） | [35/384, 0, 500/1113, 125/192, −2187/6784, 11/84, 0] |

表 2.3a（ode5 の A）：
```
[1/5]
[3/40, 9/40]
[44/45, −56/15, 32/9]
[19372/6561, −25360/2187, 64448/6561, −212/729]
[9017/3168, −355/33, 46732/5247, 49/176, −5103/18656]
[35/384, 0, 500/1113, 125/192, −2187/6784, 11/84]
```
- 既知の差（互換表記「部分」）：Simulink の固定ステップでは出力は刻み時刻にしか存在しない。simpylink は OUTPUT_STEP の時刻とブレークポイントにも主要時刻を置くので、OUTPUT_STEP が h の倍数でない場合や Step の時刻が刻みにない場合は値がずれうる。

### 2.4 SLX・GUI
- SLX 読込：`FixedStepDiscrete`→FixedDiscrete、`ode1`〜`ode5`→同名、`FixedStepAuto`→Ode3（info「FixedStepAuto mapped to ode3」）、`ode8`/`ode14x`→Ode5（warning）。`FixedStep` が `auto` → None、数値 → Some、変数名 → §1 の ws で評価（失敗は warning で None）。
- SLX 書出：`SolverName` に `simulink_name()`、`FixedStep` に h（None なら `auto`）。
- Model Settings（Ctrl+E）：ソルバーが固定ステップのとき「Fixed-step size」欄を有効化（空 = auto）。

## 3. ベクトル信号・第1段（#92）

### 3.1 範囲
- **1 次元ベクトル**（幅 n ≥ 1）のみ。行列信号、バス、フレーム、可変サイズは対象外（`E_UNSUPPORTED_FEATURE`）。
- 骨格：`IrSignal::width`（現状すべて 1）。

### 3.2 幅の伝播（`sim_ir::width`、新規）
```rust
pub fn propagate_widths(model: &Model, conn: &Connectivity, lowered: &BTreeMap<BlockId, Lowered>)
    -> Result<BTreeMap<PortRef /* output port */, usize>, Vec<Diagnostic>>;
```
- 初期値：すべて未定。ソースから順に、各ブロックの規則（表 3.2）を入力幅が揃ったものから適用し、変化が無くなるまで反復（最大ブロック数+1 回）。最後まで未定（入力未接続・遅延ループ内だけで決まる）は 1。
- 不一致は `E_DIMENSION`（新コード）：`"'Sum' input 2 has width 3, expected 1 or 2"` の形式。

表 3.2（u_i の幅を w_i、出力幅を w）：
| ブロック | 規則 |
|---|---|
| Constant | `Value` の要素数（1×N / N×1 → N、M×N（M,N>1）は `E_UNSUPPORTED_FEATURE`） |
| Step, Sin, Clock, Ground, PulseGenerator, Ramp | 1（ベクトルのパラメータは第2段） |
| Gain | Gain の要素数 k。w = max(k, w1)。k と w1 はどちらかが 1 か等しい（行列ゲインは第2段） |
| Sum, Product, Bias, Abs, Saturate, Switch, MinMax(多入力), Relational, Logic, Math, Trig, Sqrt, Signum, DeadZone | 要素ごと：w = max(w_i)、各 w_i は 1 または w（スカラー拡張） |
| Sum/Product/MinMax で入力が1つ | 総和・総積・最小/最大を取り w = 1（Simulink の 1 入力時の挙動） |
| Integrator, UnitDelay, DiscreteIntegrator, ZeroOrderHold, Memory | w = max(w1, 初期値の要素数)。状態数 = w |
| Mux | `Inputs`（数値 n）。w = Σ w_i |
| Demux | `Outputs`：スカラー p なら w1 が p で割り切れ、各出力 w1/p。ベクトル [a b c] なら Σ = w1 で各出力 a,b,c |
| TransferFcn, StateSpace, PIDController, DiscreteTransferFcn | w1 = 1 のみ（それ以外は `E_UNSUPPORTED_FEATURE`） |
| Scope, Outport, Terminator, Display, ToWorkspace, Goto/From | 入力幅をそのまま通す（シンクは制約なし） |

### 3.3 生成 Python
- 幅 1 の信号はこれまで通り Python の float（**既存ゴールデン不変**）。
- 幅 > 1 は `np.ndarray`（shape `(n,)`, dtype float）。スカラー拡張は NumPy のブロードキャストに任せる。
- 式の置換規則：`abs(u)`→`np.abs(u)`、`min(max(u, L), U)`→`np.clip(u, L, U)`、Switch の三項演算子→`np.where(cond, a, b)`、`math.sin`→`np.sin` など、**入力または出力のどれかが幅 > 1 のときだけ** NumPy 版を出す。
- Mux：`np.concatenate([np.atleast_1d(u1), np.atleast_1d(u2), …])`。Demux：`u[a:b]`、幅 1 の出力は `float(u[a])`。
- 状態：連続・離散状態の範囲長 = 幅。Integrator は `x[a:b]`、`dx[a:b] = u`（スカラー入力なら全要素に代入される）。
- ログ：幅 n の入力は n チャネル。名前は `"<既存の名前>(k)"`（k は 1 始まり、MATLAB の添字に合わせる）、`results.meta.json` のチャネルに `"element": k` を追加（幅 1 は `element` を出さない＝既存互換）。

## 4. ゼロクロッシング・第1段（#81）

- 対象：連続サンプル時間に解決されたブロックのうち、`Saturate`（u−U, u−L）、`Switch`（判定式の左辺−右辺）、`Abs`（u）、`Signum`（u）、`RelationalOperator`（u1−u2）、`MinMax`（2 入力：u1−u2）、`DeadZone`（u−U, u−L）。
- `SimConfig::zero_crossing: bool`（既定 true、format v2 の項目に追加。false のとき JSON に出す）。Model Settings にチェックボックス。
- IR：`SimIr::zero_crossings: Vec<IrZeroCrossing { block_sid: BlockId, index: u32 }>`、生成 Python に `NUM_ZERO_CROSSINGS` と `def zero_crossing_signals(t, x, d, t_seg)`（`compute_outputs` を呼び、各 ZC 量のリストを返す）。0 個なら関数は出さず `NUM_ZERO_CROSSINGS = 0` のみ（ランタイム改訂でゴールデンに1行増える）。
- ランタイム：`NUM_ZERO_CROSSINGS > 0` かつ可変ステップのとき、`solve_ivp(..., events=[g_0, …])`（各 `terminal = True`）。イベントが起きたら、その時刻・状態で区間を打ち切り、そこを新しい主要時刻（出力なし）として処理を続ける（`compute_outputs` → 次区間）。同一時刻での連続イベントは 1e-12 進めて再開（無限ループ防止、1 区間あたり最大 1000 回で `SimulationError`）。
- モード固定（Simulink の minor step でのモード保持）は **第2段**。第1段は「不連続の時刻を正確に打つ」まで。固定ステップでは使わない（Simulink と同じ）。

## 5. ブロック拡充・スカラー（#95）

方針：
- 1 ブロック（または同種の数個）= 1 Issue。#80 と同じ構成（BlockSpec / IrBlockKind / lower / 生成式 / fixture / E2E）。
- BlockType 名・パラメータ名・既定値は Simulink の名前（AGENTS.md）。SLX は既定値と同じパラメータを省略するので、**既定値は Simulink の公開ブロックリファレンスと一致させる**（出典 URL を Issue に記載。R2026b 実機では未確認）。
- データ型は double のみ。論理出力は 1.0 / 0.0。`OutDataTypeStr` 等の型パラメータは読み込むが無視し、既定値以外なら `W_DATA_TYPE_IGNORED`（新コード、Warning）。
- ゼロクロッシングは §4 で後付け（それまで互換表記「部分」）。
- 数式：旧実装 `backend/simpylink_backend/blocks.py`（コミット 3d77194）を参照してよい。

| Issue | ブロック（BlockType） |
|---|---|
| #150 | Terminator, Ground |
| #151 | Goto, From |
| B-ZOH | ZeroOrderHold |
| B-TRIG | Trigonometry, Signum |
| B-MATH | Math, Sqrt |
| B-MINMAX | MinMax, DeadZone |
| B-LOGIC | RelationalOperator, Logic |
| B-PULSE | DiscretePulseGenerator（Pulse Generator 時間ベース）, Ramp |
| B-SINKS | Display, ToWorkspace, DataTypeConversion |
| B-LOOKUP | Lookup_n-D（1-D のみ） |
| B-DISCRETE | Memory, Delay, DiscreteTransferFcn |

離散ブロックの「サンプルヒット時に入力をそのまま出す」挙動（ZeroOrderHold 等）は、`compute_outputs` がヒット情報を持たないため、**モデル固有部に** ヘルパー `_sample_hit(t, period, offset)`（許容 `_time_tolerance()`）を、使うブロックがあるときだけ出力して判定する（ランタイムは変えない）。

## 6. Subsystem の残り（#91）

- 中核（Ctrl+G、階層移動、ブレッドクラム、平坦化）は実装済み。
- Atomic Subsystem（SLX の `TreatAsAtomicUnit` = `on`）：**仮想として平坦化** し、`W_ATOMIC_AS_VIRTUAL`（新コード、Warning、「実行順が Simulink と異なる場合がある」）を出す。代数ループが Atomic の境界で切れる Simulink の挙動は再現しない。
- 階層を跨ぐ Undo：`Command` はすべて `path` を持つので、サブシステム内の編集 → 親へ移動 → Undo が正しく戻ることをテストで固定する。
- Enabled/Triggered/Function-Call Subsystem（`EnablePort`/`TriggerPort` を含む）：`E_UNSUPPORTED_FEATURE`。

## 7. SLX の残り（#86, #87, #88）

### 7.1 折れ点（#86）
- ポート位置の計算（`sim_gui::canvas::geometry::block_ports_at` のうち egui に依存しない部分）を `sim_core::geom::port_positions(block: &Block) -> PortPositions { inputs: Vec<Point>, outputs: Vec<Point> }` に移し、`sim_gui` はそれを呼ぶ（依存方向を守るため）。
- 読込：`Points` は「直前の点からの相対オフセット (dx, dy) の列」、起点はソースの出力ポート位置。`Branch` の起点は親の最後の点（`Points` が無ければ親の起点）。絶対座標に変換して `Line.points` / `Branch.points` に入れ、`Branch.tap` は親の折れ線上で起点に最も近い位置（segment, t）。
- 書出（新規・差分の線再生成時）：逆変換。

### 7.2 新規作成（#87 (b)）
- 新規 SLX は、公開コーパスで最も多い **R2026a 以前の XML 構成**（`blockdiagram.xml` + `systems/system_root.xml` + `configSet0.xml` + `configSetInfo.xml` + `[Content_Types].xml` + `_rels/.rels` + `metadata/coreProperties.xml`）で書く。R2026b の JSON 構成は観察が 18 件のみで必須パートが確定できないため使わない。
- `coreProperties.xml` の `cp:version` は書かない（偽のリリース表記を避ける）。Simulink が開けるかは **未検証**（#90）。

### 7.3 損失レポート（#88）
`ImportReport` に分類を追加：`preserved: Vec<String>`（simpylink では編集できないが書出で元のまま残る要素：未対応ブロック、マスク、Stateflow パートなど）と既存の `dropped`（書出しても失われる要素：物理接続、トリガ接続）。差分書出（#148）後に正しくなる。

## 8. 配布（#100）

- タグ `v*` の push で GitHub Actions が Windows / Linux / macOS（x86_64、macOS は arm64 も）で `cargo build --release -p sim_app` を行い、`simpylink-<ver>-<target>.zip`（実行ファイル、README.md、LICENSE、THIRD_PARTY_LICENSES）を Release に添付。
- Python は同梱しない（利用者が Python 3.10+ / NumPy / SciPy を用意。`simpylink check-python` で確認）。理由：生成コードが「普通の Python」であることが製品の要点で、同梱すると容量と保守が大きい。
- コード署名・インストーラーは v1.0 後。

## 9. 今回は設計しないもの（理由）

| Issue | 理由 |
|---|---|
| #93 マルチレート | §3 と §5 の離散ブロックが先。現状は単一レートのみで、公開 SLX でも多くは単一レート |
| #96 代数ループ求解 | §3 の後。現状は検出のみ |
| #98 編集操作の拡充 | R2026b の実機挙動の確認が必要（ユーザーは Simulink を持たない）。公式の操作一覧から確認できる項目だけを別途切り出す |
| #102 性能 / #104 v1.0 | 機能が揃ってから |
| #97, #101, #90 | Simulink 実機が必要 |
