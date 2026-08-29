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
- repository-localな`.git/config`だけを更新
- GitHubリポジトリ作成、`origin`設定、初回push
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
4. ローカルGitリポジトリを追加
5. Profileを選び、変更内容を確認して適用
6. 未公開リポジトリは「GitHubに公開」から作成・push

## 安全境界

- Global/System Git configは変更しない
- フロントエンドから任意shell commandを受け取らない
- SSH秘密鍵、GitHub token、ワンタイムコードを状態ファイルへ保存しない
- SSH鍵は`~/.ssh`内の既存秘密鍵だけを参照する
- Git設定は適用前にキー単位で差分を表示する
- GitHub公開は既存`origin`や未コミット変更がある場合に拒否する
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
