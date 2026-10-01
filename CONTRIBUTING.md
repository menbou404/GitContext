# Contributing to GitContext

IssueやPull Requestを歓迎します。大きな変更は実装前にIssueで目的と安全境界を相談してください。

## Development setup

```powershell
npm ci
npm test
cargo test --manifest-path src-tauri\Cargo.toml --workspace
npm run tauri dev
```

## Workflow

1. 最新の`main`から短命な作業ブランチを作成
2. 変更に対応するテストを追加・更新
3. フロントエンドとRustのテストを実行
4. Pull Requestで変更目的、安全上の影響、確認結果を説明

ブランチ名は`feature/summary`、`fix/summary`など短く具体的にしてください。Claude Codeが作成するブランチは`claude/summary`を使用します。

## AI-assisted development

Claude CodeとCodexを併用する場合は、次のように役割を分けます。

| 担当 | 役割 |
|---|---|
| Claude Code | 設計、タスク分解、ブランチ作成、commit、push、Pull Request作成・merge、`gh`操作、CHANGELOGとチェックリストの確認 |
| Codex | サンドボックス内でのコード実装・修正、テスト実行、差分レビュー |

1. Claude Codeが設計を固め、最新の`main`から作業ブランチを作成
2. Codexが作業ブランチ上で実装とテストを行う
3. Claude Codeが差分とテスト結果を確認してcommitし、Pull Requestを作成
4. 必要に応じてCodexがPull Requestの差分をレビュー

Codexの`workspace-write`サンドボックスでは`.git`が読み取り専用でネットワークも遮断されるため、Git／GitHub操作はCodexへ任せません。push、merge、ブランチ削除などの外部に影響する操作は、Claude Codeが実行前に確認を取ります。

## Pull Request checklist

- `npm test`が成功する
- `npm run build`が成功する
- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`が成功する
- `cargo test --manifest-path src-tauri/Cargo.toml --workspace`が成功する
- token、秘密鍵、個人情報をコミットしていない
- ユーザー向け変更を`CHANGELOG.md`へ記載した

## Security-sensitive changes

認証、SSH鍵、Git config、外部コマンド、filesystem権限に関わる変更では、入力検証、秘密情報の保存有無、失敗時の復旧方法をPull Requestへ明記してください。
