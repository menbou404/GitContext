# Contributing to GitContext

IssueやPull Requestを歓迎します。大きな変更は実装前にIssueで目的と安全境界を相談してください。

## Development setup

```powershell
npm ci
npm run hooks:install
npm test
cargo test --manifest-path src-tauri\Cargo.toml --workspace
npm run tauri dev
```

## 個人情報の混入防止

公開リポジトリのため、個人情報や秘密情報がcommit・pushされないよう、ローカルのGit hookとCIで検査します。

### ローカルのGit hook

`npm run hooks:install`で、このリポジトリの`core.hooksPath`を`scripts/git-hooks`に設定します。以後、次のタイミングで`scripts/personal-data-guard.mjs`が実行されます。

| hook | 検査対象 |
|---|---|
| `pre-commit` | ステージした変更の追加行、ファイル名、commitの作成者とcommitterのメール |
| `commit-msg` | commitメッセージ |
| `pre-push` | pushするcommitの差分、ファイル名、メッセージ、作成者とcommitter |

常に有効な検査:

- 許可されていないメールアドレス（許可するドメインとアドレスは`.personal-data-guard.json`で管理）
- 状態ファイル（`state.json`など）、秘密鍵、`.env`などの禁止ファイル名

### ローカル専用の禁止語リスト

自分の個人名、私用メールアドレス、Windowsのユーザー名など、リポジトリに含めたくない語は`.git/info/personal-denylist`に1行1語で書きます。このファイルは`.git`の中にあるため、commitもpushもされません。大文字小文字を区別しない部分一致で検査し、一致した語そのものは出力せず、何番目の項目に一致したかだけを表示します。

### CI

`Privacy and secret scan`ジョブで、追跡中のファイルとPull Requestのcommitを同じルールで検査し（禁止語リストは使いません）、[gitleaks](https://github.com/gitleaks/gitleaks)で履歴全体の秘密情報を検査します。外部コントリビューターのcommit作成者のメールは、失敗ではなく警告として扱います。

意図的に例外が必要な場合は、`.personal-data-guard.json`に許可を追加し、理由をPull Requestに書いてください。

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
