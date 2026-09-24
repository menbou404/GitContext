# Changelog

このプロジェクトの主な変更を記録します。バージョン番号は[Semantic Versioning](https://semver.org/)を基本とします。

## [Unreleased]

### Added

- ProfileのGitHub一覧またはSSH URLからcloneし、同じIdentityを自動適用するフロー
- 適用済みProfileのSSH鍵で現在ブランチを安全に通常pushするフロー
- 現在ブランチと変更ファイルを確認し、commitのみ／commitしてpushを選べるフロー
- 作業ブランチ作成、commit、push、Pull Request作成を安全に案内する再開可能なフロー

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
