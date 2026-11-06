# AGENTS.md — simpylink 実装規約

このリポジトリは GitHub Issue 単位で実装します。作業前に必ず次を読んでください。
1. 担当 Issue の全文（特にセクション D, G, H, K, N）
2. Issue の Dependencies に書かれた設計 Issue（docs/architecture/ に写しがあります）
3. このファイル

## 絶対ルール
- Issue のセクション D に書かれたファイル以外を変更しない。
- 公開 API（pub な型・関数・フィールド）の名前や型を設計 Issue から変えない。変える必要があれば実装を止めて Issue にコメントする。
- テストの期待値を根拠なく変えない。テストを削除しない。`#[ignore]` / `pytest.skip` を付けない。
- エラーを握りつぶさない（`let _ = result;`、空の `catch`、`unwrap_or_default()` で失敗を隠さない）。
- ライブラリコードで `unwrap()` / `expect()` / `panic!` を使わない（テストコードと、Issue で明示的に許可された箇所を除く）。
- HashMap / HashSet の反復順に出力を依存させない。決定的な出力が必要な所は BTreeMap / BTreeSet / ソート済み Vec を使う。
- 依存クレートの追加・バージョン変更は Issue に書かれている場合のみ。
- ブロック型名・パラメータ名は Simulink の名前（`Gain`, `Sum`, `Inputs`, `SampleTime` …）を使う。

## コマンド
    cargo fmt --all
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    python -m pip install -r python/requirements.txt
    python -m pytest python/tests -q

## クレートの依存方向（違反禁止）
    sim_core            : 依存なし
    sim_ir              : sim_core
    sim_codegen_py      : sim_ir, sim_core
    sim_runner          : なし
    sim_slx             : sim_core
    sim_gui             : sim_core, sim_ir, sim_codegen_py, sim_runner, sim_slx
    sim_app             : すべて

## コーディング規約
- Rust edition 2021、rustfmt 既定設定。
- pub 項目には `///` で1行以上の説明を書く。
- エラー型は thiserror。エラーメッセージは英語、小文字始まり。
- テストは同じファイルの `#[cfg(test)] mod tests` か、クレートの `tests/` に置く。
- GUI ロジックは `sim_gui::canvas::controller` など egui 非依存のモジュールでテストする。

## 完了報告（PR 本文）に必ず書くこと
- `Closes #<Issue番号>`
- 変更したファイル一覧
- 実行したコマンドと結果（pass 数）
- 手動確認した内容（GUI の場合）
- 残っている制限・未対応事項

## Simulink 互換性の表記
「確認済み / 部分 / 未対応 / 未検証」の4段階。Simulink R2026b 実機で確認していないものを「互換」「Simulink と同じ」と書かない。
