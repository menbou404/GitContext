# Contributing to GitContext

IssueやPull Requestを歓迎します。大きな変更は実装前にIssueで目的と安全境界を相談してください。

## Development setup

```powershell
npm ci
npm test
cargo test --manifest-path src-tauri\Cargo.toml
npm run tauri dev
```

## Workflow

1. 最新の`main`から短命な作業ブランチを作成
2. 変更に対応するテストを追加・更新
3. フロントエンドとRustのテストを実行
4. Pull Requestで変更目的、安全上の影響、確認結果を説明

ブランチ名は`feature/summary`、`fix/summary`など短く具体的にしてください。Codexで作業する場合は`codex/summary`を使用します。

## Pull Request checklist

- `npm test`が成功する
- `npm run build`が成功する
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`が成功する
- `cargo test --manifest-path src-tauri/Cargo.toml`が成功する
- token、秘密鍵、個人情報をコミットしていない
- ユーザー向け変更を`CHANGELOG.md`へ記載した

## Security-sensitive changes

認証、SSH鍵、Git config、外部コマンド、filesystem権限に関わる変更では、入力検証、秘密情報の保存有無、失敗時の復旧方法をPull Requestへ明記してください。
