# GitContext

[![CI](https://github.com/menbou404/GitContext/actions/workflows/ci.yml/badge.svg)](https://github.com/menbou404/GitContext/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/menbou404/GitContext?include_prereleases)](https://github.com/menbou404/GitContext/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

GitContextは、複数のGitHub作業Identityをリポジトリ単位で割り当てるWindowsデスクトップアプリです。

アカウントの「現在値」を毎回切り替える代わりに、`個人開発`、`研究室`、`学校`など任意名のProfileを作成します。各ProfileはGitの作成者情報、既存SSH鍵、Profile専用のGitHub CLI設定をまとめて保持し、リポジトリへ一度割り当てて使用します。

> [!IMPORTANT]
> 現在はWindows向けβ版です。公開中のインストーラーはコード署名されていないため、Windows SmartScreenの警告が表示される場合があります。

## 主な機能

- 任意名のProfileを必要なタイミングで追加・編集
- ProfileごとにGitHub CLIのブラウザ認証を分離
- Gitの`user.name` / `user.email`と既存SSH鍵をまとめて管理
- リポジトリごとのProfile割り当てと適用前レビュー
- ProfileのGitHub一覧またはSSH URLから安全にcloneし、自動でIdentityを適用
- repository-localな`.git/config`だけを更新
- GitHubリポジトリ作成、`origin`設定、初回push
- 現在ブランチと変更ファイルを確認し、適用済みProfileで全変更をcommit
- commitのみ、またはcommit後の通常pushを1回の画面から実行
- 適用済みProfileのSSH鍵で現在ブランチを通常push
- 既定ブランチから作業ブランチを作成し、commit・push・Pull Request作成までを案内
- 日本語・英語UI

## インストール

配布物は[GitHub Releases](https://github.com/menbou404/GitContext/releases)からダウンロードできます。

GitHub連携を利用する場合はGitHub CLIも必要です。

```powershell
winget install --id GitHub.cli
```

GitContextは認証tokenやSSH秘密鍵の内容を保存しません。GitHub CLIのProfile専用設定ディレクトリと、`~/.ssh`にある既存鍵のパスだけを参照します。

## 基本的な使い方

1. Profileを作成し、Gitの作成者名とメールアドレスを入力
2. 「GitHubと紐付ける」から対象アカウントを認証
3. 必要に応じて既存SSH秘密鍵を選択
4. GitHubからcloneするか、既存のローカルGitリポジトリを追加
5. Profileを選び、変更内容を確認して適用
6. 未公開リポジトリは「GitHubに公開」から作成・push
7. 現在ブランチと変更ファイルを確認し、「commitのみ」または「commitしてpush」を実行
8. commit済みの変更だけを送る場合は、送信先とブランチを確認して通常push
9. 「PRを作成」から作業ブランチ名、commit、PRタイトルと説明を確認してPull Requestを作成

## 安全境界

- Global/System Git configは変更しない
- フロントエンドから任意shell commandを受け取らない
- SSH秘密鍵、GitHub token、ワンタイムコードを状態ファイルへ保存しない
- SSH鍵は`~/.ssh`内の既存秘密鍵だけを参照する
- Git設定は適用前にキー単位で差分を表示する
- GitHub公開は既存`origin`や未コミット変更がある場合に拒否する
- clone URLは`git@github.com:owner/repository.git`形式だけを受け付け、選択ProfileのSSH鍵を明示する
- commit前に現在ブランチ、Profile、対象ファイルを表示し、コミットメッセージは1行200文字以内に制限する
- 通常pushはGitHub SSH origin、適用済みProfile、現在ブランチを再検証し、force pushを提供しない
- PR作成はProfile専用のGitHub CLI認証を使い、既存PRの重複作成を避け、途中失敗後に再開できる
- 複数Git設定の途中失敗時は元の値へロールバックする

詳細は[docs/MVP_DESIGN.md](docs/MVP_DESIGN.md)を参照してください。脆弱性の報告方法は[SECURITY.md](SECURITY.md)に記載しています。

## 開発

必要環境:

- Windows 11
- Node.js 22 LTS
- Rust stable MSVC toolchain
- Microsoft C++ Build Tools / Windows SDK
- Microsoft Edge WebView2
- Git
- GitHub CLI（GitHub連携を使用する場合）

```powershell
npm ci
npm test
cargo test --manifest-path src-tauri\Cargo.toml
npm run tauri dev
```

ブラウザ用の操作プレビューは実際のGit設定を書き換えません。

```powershell
npm run dev
```

- 英語版: `http://localhost:1420/`
- 日本語版: `http://localhost:1420/ja.html`

## リリースとバージョニング

`main`は次期版の開発を続けるブランチです。GitHub Releaseは`v`で始まるタグから作成し、公開済みタグは移動しません。

- `v0.1.1`: 後方互換なバグ修正
- `v0.2.0`: 新機能や大きな変更
- `v0.x.y-beta.n`: 動作確認用のプレリリース

変更履歴は[CHANGELOG.md](CHANGELOG.md)、開発への参加方法は[CONTRIBUTING.md](CONTRIBUTING.md)を参照してください。

## License

[MIT License](LICENSE)
