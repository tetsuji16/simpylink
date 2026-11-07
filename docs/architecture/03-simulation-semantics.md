> Source: https://github.com/tetsuji16/simpylink/issues/19 (copied on 2026-11-07)

> 計画ID: `DES-03` ／ 親: #16 ／ Blocked by: #18

# [DES-03] 設計契約：シミュレーション意味論と Simulation IR

> **設計契約**。個別ブロックの実装（M2/M4）より先にここで意味論を確定する。
> 本書の期待値は「設計上定義した期待値」または「解析解」であり、**Simulink R2026b 実機での一致は未検証**。実機比較は #101 で行う。

## 1. 用語

| 用語 | 定義 |
|---|---|
| 信号 | 出力ポート1つが出す値。M2〜M5 では **スカラー float64 のみ**（ベクトルは #92） |
| 直達（Direct Feedthrough, DF） | 入力ポートの値が同時刻の出力計算に必要なこと |
| 連続状態 x | ODE ソルバーが積分する状態ベクトル（全ブロック分を連結） |
| 離散状態 d | サンプルヒット時にのみ更新される状態ベクトル |
| 主要時刻 (major time point) | 出力時刻・ブレークポイント・離散サンプルヒット・停止時刻の和集合 |
| 実行順 (sorted order) | 出力計算の順番。DF 依存のトポロジカル順 |

## 2. ブロックの意味論表（M2〜M4 で実装対象）

`u_i` = 入力ポート i の値、`y` = 出力、未接続入力は 0.0。

| block_type | 出力式 | DF | 連続状態 | 離散状態 | サンプル時間（既定） | 担当Issue |
|---|---|---|---|---|---|---|
| Constant | y = Value | – | 0 | 0 | Constant（"inf"） | #56 |
| Step | y = After if t_seg >= Time else Before（**右連続**） | – | 0 | 0 | 連続（"0"） | #56 |
| Gain | y = Gain·u1 | ○ | 0 | 0 | 継承（"-1"） | #56 |
| Sum | y = Σ s_i·u_i（s_i は Inputs の +/-） | ○ | 0 | 0 | 継承 | #56 |
| Scope | 出力なし。入力 i をログ "名前/i" | ○（ログのみ） | 0 | 0 | 継承 | #56 |
| Outport（ルート） | 出力なし。入力をログ "名前" | ○ | 0 | 0 | 継承 | #56 |
| Integrator | y = x；dx/dt = u1；x(0)=InitialCondition | – | 1 | 0 | 連続 | #71 |
| TransferFcn | 可制御正準形 (A,B,C,D)、y = Cx + D·u1 | D≠0 のとき○ | n | 0 | 連続 | #73 |
| StateSpace | y = Cx + D·u1、dx/dt = Ax + B·u1 | D≠0 のとき○ | n | 0 | 連続 | #74 |
| UnitDelay | y = d；ヒット時 d ← u1 | – | 0 | 1 | 継承→解決必須 | #75 |
| DiscreteIntegrator | 前進 Euler：y = d；ヒット時 d ← d + gainval·Ts·u1 | – | 0 | 1 | 離散（"1"） | #76 |
| PIDController | 並列形：y = P·u + xi + D·N·(u − w)；dxi/dt = I·u；dw/dt = N·(u − w) | ○ | 2 | 0 | 連続 | #77 |
| Sin（Sine Wave） | y = Amplitude·sin(Frequency·t + Phase) + Bias | – | 0 | 0 | 連続 | #80 |
| Clock | y = t | – | 0 | 0 | 連続 | #80 |

`t_seg`：区間左端評価時刻（§4.3）。Step のような区分定数入力の不連続点はブレークポイントに登録する。

## 3. コンパイル手順（`sim_ir::compile`）

1. `sim_core::validate::validate_model` を実行。Error があれば `CompileFailure` を返す（Warning は持ち越す）。
2. ルート以外の階層（SubSystem）を含む場合 → `E_UNSUPPORTED_FEATURE`（#91 まで）。ルートの Inport → `E_UNSUPPORTED_FEATURE`。
3. 各ブロックのパラメータを評価して `IrBlockKind` を作る（`E_PARAM_EVAL`、`E_PARAM_DOMAIN`、`E_UNSUPPORTED_BLOCK`）。
4. 接続グラフ：各入力ポート (sid, port) → 駆動元出力ポート (sid, port) の写像を作る（Line::destinations を合併）。
5. 信号割当：出力ポート1つにつき `SignalId` を1つ。sid 昇順・port 昇順で 0 から採番。
6. 実行順：§5。代数ループは `E_ALGEBRAIC_LOOP`。
7. サンプル時間解決：§6。
8. 状態割当：実行順ではなく **sid 昇順** に連続状態・離散状態のオフセットを割り当てる。
9. ブレークポイント収集：Step の Time（start_time < Time <= stop_time のもの）を昇順・重複除去。
10. ログチャネル：Scope（sid 昇順、ポート昇順）→ ルート Outport（Port 番号昇順）の順。

## 4. 時間進行アルゴリズム（生成 Python の固定ランタイムが実装）

### 4.1 主要時刻
```text
tol = 1e-9 * max(1, |start|, |stop|)
出力時刻   = { round(start + k*output_step, 12) | k=0,1,..., <= stop+tol } ∪ { stop }
ブレーク    = BREAKPOINTS のうち [start-tol, stop+tol] 内
離散ヒット  = 各レート (Ts, off) について { round(off + k*Ts, 12) | k=0,1,... } のうち [start-tol, stop+tol] 内
主要時刻    = 上記の和集合を昇順に並べ、差が tol 以下の点を1点に併合（最初の時刻値を採用）
各点は (t, is_output, hits=ヒットしたレート番号のタプル) を持つ
```
### 4.2 各主要時刻 t_i での処理順（Simulink のメジャーステップに倣う）
1. `y = compute_outputs(t_i, x, d, t_seg=t_i)`（実行順で全出力を計算）
2. `is_output` なら `t_i` と `log_values(y)` を記録
3. `hits` が空でなければ `d = update_discrete_states(t_i, x, d, y, hits)`
4. 次の点があり連続状態数 > 0 なら、`solve_ivp(f, (t_i, t_{i+1}), x)` で積分して `x` を更新。積分中 `d` は固定、`f(t, x) = compute_derivatives(t, x, d, t_seg=t_i)`

### 4.3 区間左端評価規則（t_seg）
区分定数の時間関数（Step）は **`t_seg` で評価** する。主要時刻では `t_seg = t`、積分区間 (t_i, t_{i+1}) 内では `t_seg = t_i`。ブレークポイントが主要時刻に含まれるため、Step の値は各積分区間内で一定となり、ソルバーが不連続をまたがない。SineWave / Clock は **t で評価**（滑らかな関数）。

### 4.4 ソルバー対応（近似マッピング、Simulink と同一アルゴリズムではない）
| SolverKind | SciPy method | 注記 |
|---|---|---|
| auto | "RK45" | Simulink の自動選択とは異なる |
| ode45 | "RK45" | Dormand–Prince（同系統） |
| ode23 | "RK23" | Bogacki–Shampine（同系統） |
| ode15s | "BDF" | 可変次数 BDF（NDF ではない） |
`max_step=None` → `math.inf`。連続状態が 0 個なら `solve_ivp` を呼ばない。

## 5. 実行順（Sorted Order）

- 有向グラフ G：駆動元ブロック A → 被駆動ブロック B の辺を、**B のその入力ポートが DF のときだけ** 張る。
- **Kahn 法**、入次数0のノードを `BinaryHeap<Reverse<sid>>` で sid 最小から取り出す（決定的）。
- 取り出せなかったノードが残れば代数ループ：Tarjan SCC で **サイズ2以上の SCC、または自己ループ** を列挙し、各 SCC を `E_ALGEBRAIC_LOOP`（message: `algebraic loop: Gain(2) -> Sum(3) -> Gain(2)` の形式、SCC 内 sid 昇順で開始し辺をたどる）として報告。代数ループの **求解は M6**（#96）。
- 例：Constant(1) → Sum(2) ← Integrator(3)、Sum(2) → Integrator(3)。Integrator は非DFなので辺は 1→2、3→2 のみ。順序 = [1, 3, 2]。

## 6. サンプル時間

```rust
pub enum SampleTime { Constant, Continuous, Discrete { period: f64, offset: f64 }, }
```
- パラメータ解釈：`"inf"` → Constant、`"0"` → Continuous、正数 `Ts` → Discrete{Ts,0}、`"[Ts, off]"` → Discrete{Ts,off}、`"-1"` → 継承。
- 継承の解決（実行順に1パス + 収束まで繰返し、最大ブロック数回）：入力の駆動元の解決済みサンプル時間集合 S について
  - S が空（全入力未接続）→ Constant
  - Continuous を含む → Continuous
  - Constant のみ → Constant
  - Discrete がちょうど1種類（Constant と混在可）→ その Discrete
  - Discrete が2種類以上 → `E_SAMPLE_TIME`（"multirate is not supported"、#93）
- 状態を持つ離散ブロック（UnitDelay 等）で解決結果が Continuous/Constant → `E_SAMPLE_TIME`（"Unit Delay 'X' needs a discrete sample time; set SampleTime"）。
- 非DFのフィードバック（UnitDelay の出力が自分の入力に戻る）では、未解決のまま循環する。UnitDelay の SampleTime が "-1" のまま循環内でしか決まらない場合も `E_SAMPLE_TIME`。
- 離散レート一覧 `DISCRETE_RATES` は Discrete の (period, offset) を period 昇順→offset 昇順で重複除去（差 1e-12 以下を同一）。

## 7. IR 型（`sim_ir`、確定）

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SignalId(pub u32);

#[derive(Debug, Clone, PartialEq)]
pub struct SimIr {
    pub model_name: String,          // Model.name
    pub config: IrConfig,
    pub blocks: Vec<IrBlock>,        // 実行順（sorted order）
    pub signals: Vec<IrSignal>,      // SignalId 順
    pub num_cont_states: usize,
    pub num_disc_states: usize,
    pub discrete_rates: Vec<(f64, f64)>,
    pub breakpoints: Vec<f64>,
    pub logs: Vec<IrLog>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct IrConfig { pub start_time: f64, pub stop_time: f64, pub output_step: f64, pub solver_method: String, pub rel_tol: f64, pub abs_tol: f64, pub max_step: Option<f64> }
#[derive(Debug, Clone, PartialEq)]
pub struct IrSignal { pub id: SignalId, pub source: PortRef, pub var_name: String }
#[derive(Debug, Clone, PartialEq)]
pub struct IrBlock {
    pub sid: BlockId,
    pub name: String,                 // 元のブロック名（コメント用）
    pub block_type: String,
    pub var_base: String,             // Python 識別子の基底（§8）
    pub kind: IrBlockKind,
    pub inputs: Vec<Option<SignalId>>, // 入力ポート順。None = 未接続（0.0）
    pub outputs: Vec<SignalId>,
    pub sample_time: SampleTime,
    pub cont_state: Option<std::ops::Range<usize>>,
    pub disc_state: Option<std::ops::Range<usize>>,
    pub rate_index: Option<usize>,    // Discrete のとき DISCRETE_RATES の番号
}
#[derive(Debug, Clone, PartialEq)]
pub enum IrBlockKind {
    Constant { value: f64 },
    Step { time: f64, before: f64, after: f64 },
    Gain { gain: f64 },
    Sum { signs: Vec<i8> },           // +1 / -1、入力ポート順
    Scope,
    Outport { port: u32 },
    // M4 で追加（#71 等）：Integrator{x0}, TransferFcn{a,b,c,d}, StateSpace{a,b,c,d,x0}, UnitDelay{x0}, DiscreteIntegrator{gain,x0}, Pid{p,i,d,n}, SineWave{..}, Clock
}
#[derive(Debug, Clone, PartialEq)]
pub struct IrLog { pub name: String, pub block_sid: BlockId, pub port: u32, pub signal: Option<SignalId> }
```
`IrConfig.solver_method` は §4.4 の SciPy 名（"RK45" 等）。

## 8. Python 識別子規則（sim_ir が決定、codegen は使うだけ）

- `var_base` = `sanitize(name) + "_" + sid`。`sanitize`：(1) ASCII 英数字以外を `_` に置換、(2) CamelCase の境界（小文字/数字→大文字）に `_` を挿入、(3) 小文字化、(4) 連続 `_` を1つに、(5) 前後の `_` を除去、(6) 空なら `"block"`、(7) 先頭が数字なら `"b_"` を前置。例："Gain"→`gain_2`、"Unit Delay"→`unit_delay_3`、"TransferFcn1"→`transfer_fcn1_5`、"増幅"→`block_7`、"1abc"→`b_1abc_4`、"x2Y"→`x2_y_9`。
- 出力が1つなら信号変数名 = `var_base`、2つ以上なら `var_base + "_y" + port`。
- パラメータ定数名 = `var_base.to_uppercase() + "_" + 定数サフィックス`。定数サフィックスはブロックごとに Issue で指定（例：Gain → `GAIN`、Step → `TIME`/`BEFORE`/`AFTER`）。
- ログ名：Scope は `"{name}/{port}"`、Outport は `"{name}"`。名前中の `,` `"` 改行は `_` に置換。

## 9. 参照（R2026b ドキュメント、ブロック仕様の出典）
- Step：https://www.mathworks.com/help/simulink/slref/step.html
- Integrator：https://www.mathworks.com/help/simulink/slref/integrator.html
- Transfer Fcn：https://www.mathworks.com/help/simulink/slref/transferfcn.html
- State-Space：https://www.mathworks.com/help/simulink/slref/statespace.html
- Unit Delay：https://www.mathworks.com/help/simulink/slref/unitdelay.html
- Discrete-Time Integrator：https://www.mathworks.com/help/simulink/slref/discretetimeintegrator.html
- PID Controller：https://www.mathworks.com/help/simulink/slref/pidcontroller.html
- Algebraic loops：https://www.mathworks.com/help/simulink/ug/algebraic-loops.html
- Sample time：https://www.mathworks.com/help/simulink/ug/what-is-sample-time.html
- Solvers：https://www.mathworks.com/help/simulink/gui/solver.html

（URL はリリース非依存の現行ヘルプ。R2026b 表示は #101 で再確認する）


---
_Generated by [Claude Code](https://claude.ai/code)_
