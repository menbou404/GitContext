# Changelog

このプロジェクトの主な変更を記録します。バージョン番号は[Semantic Versioning](https://semver.org/)を基本とします。

## [Unreleased]

### Added

- Profile-aware Fetch、ahead/behind表示、安全なFast-forward Pull、Pushをまとめたリポジトリ同期フロー
- ProfileのGitHub一覧またはSSH URLからcloneし、同じIdentityを自動適用するフロー
- 適用済みProfileのSSH鍵で現在ブランチを安全に通常pushするフロー
- 現在ブランチと変更ファイルを確認し、commitのみ／commitしてpushを選べるフロー
- 作業ブランチ作成、commit、push、Pull Request作成を安全に案内する再開可能なフロー
- 開いているPull Request、CI、競合、レビュー状態の確認と、先頭commitを固定した安全なmergeフロー
- 状態ファイルの自動バックアップ（保存・データ移行の前に最新20世代を保持）
- AIエージェント向けMCPサーバー`gitcontext-mcp`（開発中・読み取り専用）：Profileとリポジトリの一覧、作業フォルダからのリポジトリ検索、状態とIdentity一致の確認、Profile候補の提案、各種プレビュー、Pull Request一覧。SSH鍵はファイル名だけ、gh設定ディレクトリは返さない
- 開発者向け：個人情報・秘密情報のcommitを防ぐGit hook（ローカル専用の禁止語リスト対応）と、CIでのプライバシー・秘密情報スキャン

### Changed

- 開発版（debugビルド）のデータをインストーラー版と別のフォルダ（`app.gitcontext.dev`）に保存し、開発版では保存先を画面に表示
- 状態ファイルの排他制御をプロセス間で有効なファイルロックに変更

### Fixed

- Profileから外したGitHubユーザー名・gh設定ディレクトリ・SSH鍵の設定が、再適用後もリポジトリのローカル設定に残る問題（適用前のプレビューに「削除」と表示し、GitContextが書いた`core.sshCommand`だけを削除）

### Planned

- 適用履歴とワンクリックrollback
- GitContext経由のGitHub操作
- リポジトリrootの一括scan

## [0.1.0-beta.1] - 2026-08-29

### Added

- 任意名のRepository Identity / Profile
- Profile別GitHub CLIブラウザ認証
- 認証中のワンタイムコード表示と認証ページ自動起動
- repository-localなGit author / SSH設定の差分レビューと適用
- GitHubリポジトリ作成、`origin`設定、初回push
- 日本語・英語UI
- Windows向けCIとDraft Releaseビルド

### Security

- GitHub token、SSH秘密鍵、ワンタイムコードをアプリ状態へ保存しない設計
- 固定引数だけをRustバックエンドからGit / GitHub CLIへ渡す検証境界
