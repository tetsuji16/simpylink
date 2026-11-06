# simpylink

**Simulink R2026b 互換を目指すオープンソースのブロック線図シミュレーター**（Rust + egui GUI / 生成 Python で計算）

An open-source block-diagram simulator that aims to follow Simulink R2026b editing behaviour. You draw a model with Simulink-like mouse and keyboard operations; simpylink generates a plain, readable, standalone Python script (NumPy + SciPy) behind the scenes, runs it in a separate process when you press **Run**, and plots the result in a Scope window.

> **Status: v0.2 — re-design phase.** The previous v0.1 prototype (Python HTTP backend, commit `3d77194`) is being replaced. The whole plan is written as GitHub Issues; implementation starts from [#31](https://github.com/tetsuji16/simpylink/issues/31). Nothing below is implemented yet unless the corresponding issue is closed.

## 何を作るか

```text
GUI操作 ─(Command)─▶ Model（JSON 保存） ─compile─▶ Simulation IR ─generate─▶ model.py
model.py ─(python 子プロセス)─▶ results.csv + results.meta.json ─load─▶ Scope 表示
```

- **GUI**：Rust + egui/eframe。ブロック線図キャンバスは独自実装。操作は Simulink R2026b の公式ショートカット表に合わせる（[#21](https://github.com/tetsuji16/simpylink/issues/21)）。
- **生成コード**：Python 標準ライブラリ + NumPy + SciPy のみ。巨大な実行エンジンは持たず、モデルごとに `compute_outputs` / `compute_derivatives` / `update_discrete_states` を持つ通常の Python になる（[#20](https://github.com/tetsuji16/simpylink/issues/20)）。GUI なしで `python model.py --out-dir results` で動く。
- **SLX**：Simulink R2026b 以降の SLX を段階的に読み書きする。内部形式は未公開のため、実機で保存したファイルの調査で確定させ、未対応要素は保持して損失を警告する。元の SLX は上書きしない（[#22](https://github.com/tetsuji16/simpylink/issues/22)）。
- **常駐サーバーなし**のローカルデスクトップアプリ（Windows / Linux / macOS）。

## 設計（Design contracts）

| Issue | 内容 |
|---|---|
| [#16](https://github.com/tetsuji16/simpylink/issues/16) | EPIC：全体計画・マイルストーン・ラベル定義 |
| [#17](https://github.com/tetsuji16/simpylink/issues/17) | Cargo Workspace・クレート責務・依存方向・公開 API・CLI |
| [#18](https://github.com/tetsuji16/simpylink/issues/18) | モデルデータ構造・JSON 形式 v1・Undo/Redo コマンド・診断 |
| [#19](https://github.com/tetsuji16/simpylink/issues/19) | シミュレーション意味論と Simulation IR（連続・離散・実行順・代数ループ） |
| [#20](https://github.com/tetsuji16/simpylink/issues/20) | 生成 Python 形式・Runner プロトコル・結果ファイル |
| [#21](https://github.com/tetsuji16/simpylink/issues/21) | キャンバス座標系・操作状態機械・Simulink R2026b 操作対応表 |
| [#22](https://github.com/tetsuji16/simpylink/issues/22) | SLX 互換戦略 |

予定しているリポジトリ構成：

```text
crates/sim_core        モデル・幾何・式・ブロック登録・検証・編集コマンド
crates/sim_ir          Model → Simulation IR
crates/sim_codegen_py  IR → Python
crates/sim_runner      Python 子プロセス実行・結果読込
crates/sim_slx         SLX 読み書き
crates/sim_gui         egui アプリ
crates/sim_app         バイナリ `simpylink`（GUI + CLI）
python/tests           生成コードの pytest
tests/fixtures         共有モデル JSON・ゴールデン Python
```

## ロードマップ

| マイルストーン | 親 Issue | 内容 |
|---|---|---|
| M0 Project Foundation | [#23](https://github.com/tetsuji16/simpylink/issues/23) | Workspace・基本型・JSON・式パーサ・CI・AGENTS.md |
| M1 Diagram Editor | [#24](https://github.com/tetsuji16/simpylink/issues/24) | キャンバス・選択・配線・分岐・ズーム・Undo・パラメータ・保存 |
| M2 Python Code Generator | [#25](https://github.com/tetsuji16/simpylink/issues/25) | 検証・IR・Python 生成（Constant / Step / Gain / Sum） |
| M3 Executable MVP | [#26](https://github.com/tetsuji16/simpylink/issues/26) | Run/Stop・Scope・E2E（Step → Gain → Scope） |
| M4 Continuous / Discrete | [#27](https://github.com/tetsuji16/simpylink/issues/27) | Integrator・Transfer Fcn・State-Space・Unit Delay・PID・ソルバー設定 |
| M5 SLX Compatibility | [#28](https://github.com/tetsuji16/simpylink/issues/28) | OPC/XML・読込・書込・未対応要素の保持・R2026b 往復検証 |
| M6 Compatibility Expansion | [#29](https://github.com/tetsuji16/simpylink/issues/29) | Subsystem・ベクトル・マルチレート・変数・ブロック拡充 |
| M7 Stable Release | [#30](https://github.com/tetsuji16/simpylink/issues/30) | 3 OS 配布・数値検証・ライセンス監査・v1.0 |

M1 と M2 は並列に進められます。

## Simulink 互換性の表記

互換性は **操作 / 数値 / SLX** を別々に評価し、「確認済み / 部分 / 未対応 / 未検証」の4段階で書きます。Simulink R2026b 実機で確認していないものを「互換」とは書きません。現時点ではすべて **未検証** です。

## 開発の進め方

- 実装は GitHub Issue 単位。ラベル `status:ready` の Issue だけが着手可能です。各 Issue に、変更ファイル・API・実装手順・テスト入力と期待値・テスト失敗時の修正手順が書かれています。
- 実装前に `AGENTS.md`（[#41](https://github.com/tetsuji16/simpylink/issues/41) で作成）と、Issue が参照する設計 Issue を読んでください。
- 1 Issue = 1 Pull Request（本文に `Closes #<番号>`）。

## 必要なもの（M0 完了後）

- Rust stable（1.95 以上）
- Python 3.10 以上と `numpy>=1.26`, `scipy>=1.11`（`python -m pip install -r python/requirements.txt`）

## License

MIT — see [LICENSE](LICENSE). Simulink and MATLAB are trademarks of The MathWorks, Inc.; simpylink is not affiliated with or endorsed by MathWorks.
