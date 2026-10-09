# Changelog

このプロジェクトの主な変更を記録します。バージョン番号は[Semantic Versioning](https://semver.org/)を基本とします。

## [Unreleased]

### Planned

- 適用履歴からのワンクリックrollback
- リポジトリrootの一括scan
- Claude Desktop（チャット）でのMCPの利用
- AI連携画面での、開発版MCPサーバーの実行ファイルの鮮度の表示
- pullを、プレビューで確認した上流のcommitに固定する

## [0.3.0-beta.1] - 2026-10-10

### Added

- GUIを通知領域に常駐させ、左クリックで再表示、右クリックメニューで終了できるようにした。設定でウィンドウを閉じたときの常駐を切り替えられる。
- 配布版でWindowsへのサインイン時に通知領域だけで起動する設定を追加し、二重起動時は既存のウィンドウを前面に表示する。
- B-1: 確認画面を出せないAIクライアントのGitHub操作を、GitContextの専用ウィンドウで確認できるようにした。ユーザー専用パイプと実行ファイルの照合、期限切れ時の拒否に対応。
- AI操作の監査ログを監視してWindows通知を出す設定と、要対応リポジトリの定期確認・通知領域の件数表示・通知設定を追加。

## [0.2.0-beta.1] - 2026-10-07

### Added
- MCP段階5: リリース用インストーラーに `gitcontext-mcp.exe` をサイドカーとして同梱。既存の利用者には、AI連携画面への案内を条件付きで一度だけ表示。
- UI-6: AI連携画面にMCPサーバー情報とクライアントの検出・接続・範囲変更・修復・解除を追加。差分確認、設定ファイルのバックアップ、接続確認、Claude Code公式CLIを使う登録に対応。
- UI-5: MCPとGUIのプロファイル適用を共有の監査ログに記録し、履歴画面に日時・操作・主体・結果・確認方法を表示。リポジトリ・プロファイル・主体で絞り込み可能にした。
- UI-4: 設定画面に環境の再確認、言語切り替え、保存先フォルダ、バックアップ一覧と画面内で確認する復元を追加。
- UI-3: プロファイル一覧を連携状態と割り当て数が分かる表に変更し、画面内の編集とGitHubブラウザ認証を整理。空の状態ではプロファイル作成、GitHub連携、リポジトリ追加を順に案内し、ブラウザプレビューの `?demo=empty` で確認可能にした。
- UI-2: リポジトリ詳細に「概要／変更／同期／Pull Request」のキーボード操作対応タブを追加。commit、push、fetch、Fast-forward Pull、GitHub公開、Pull Request作成・確認・mergeを画面内の確認欄から実行できるようにした。リポジトリ一覧に、ローカルの追跡ブランチを基準とするpush待ちのcommit数を追加。
- GUIに日本語／Englishの切り替えを追加し、選択した言語を状態ファイルに保存。未設定時はブラウザの言語から初期値を決定。
- UI-1: リポジトリ一覧・詳細の概要・プロファイル画面を新しい左メニューと無彩色の画面枠に移行。検索、4状態の絞り込み、画面内の適用確認、問題があるときだけの通知を追加。既存のcommit・push・同期・Pull Request・公開ダイアログは詳細の「操作」から利用可能
- リポジトリのローカル設定とプロファイルのGitHub連携を確認し、適用済み／要再適用／未割り当て／要確認を一括取得するTauriコマンド
- GUIから設定する「AIによる操作の自動承認」。リポジトリごとに作業ブランチへのpush、既定ブランチへのpush、Pull Requestの作成とmerge、GitHubへの公開を個別に設定でき、プロファイルごとにcloneを設定できる。MCPは有効な項目を確認なしで実行する

- Profile-aware Fetch、ahead/behind表示、安全なFast-forward Pull、Pushをまとめたリポジトリ同期フロー
- ProfileのGitHub一覧またはSSH URLからcloneし、同じIdentityを自動適用するフロー
- 適用済みProfileのSSH鍵で現在ブランチを安全に通常pushするフロー
- 現在ブランチと変更ファイルを確認し、commitのみ／commitしてpushを選べるフロー
- 作業ブランチ作成、commit、push、Pull Request作成を安全に案内する再開可能なフロー
- 開いているPull Request、CI、競合、レビュー状態の確認と、先頭commitを固定した安全なmergeフロー
- 状態ファイルの自動バックアップ（保存・データ移行の前に最新20世代を保持）
- AIエージェント向けMCPサーバー`gitcontext-mcp`（開発中・読み取り専用）：Profileとリポジトリの一覧、作業フォルダからのリポジトリ検索、状態とIdentity一致の確認、Profile候補の提案、各種プレビュー、Pull Request一覧。SSH鍵はファイル名だけ、gh設定ディレクトリは返さない
- MCPサーバーの`--max-tier local`：リポジトリ登録、プロファイル適用、ブランチ作成、commit、pull。実行はプレビューIDで固定し（1回限り・10分で失効・実行直前に状態を再照合）、commitはプレビューしたファイルだけを対象にする。変更の操作は監査ログ`mcp-audit.jsonl`に記録
- MCPサーバーの`--max-tier remote`：push、Pull Request作成、merge、clone、GitHub公開（`preview_merge`・`preview_clone`・`preview_publish`を追加）。実行前にelicitationで人の確認を求め、拒否・キャンセル・2分の時間切れでは実行しない。承認後にも状態を再照合する。確認画面を出せないクライアントでは既定で実行せず、`--trust-client-approval`で明示した場合だけクライアントの確認に任せる
- 開発者向け：個人情報・秘密情報のcommitを防ぐGit hook（ローカル専用の禁止語リスト対応）と、CIでのプライバシー・秘密情報スキャン

### Changed

- Windowsのデータ保存先をユーザーのホーム配下に変更し、旧AppData保存先から起動時に自動移行
- 開発版（debugビルド）のデータをインストーラー版と別のフォルダ（Windowsでは`%USERPROFILE%\.gitcontext-dev`）に保存し、開発版では保存先を画面に表示
- 状態ファイルの排他制御をプロセス間で有効なファイルロックに変更

### Fixed

- Codex（`codex-mcp-client`）を確認画面を表示できないクライアントとして扱い、GitHub操作の前に確認依頼を送らず理由を返す。AI連携画面でもCodexを「非対応」と表示
- macOSとLinuxで、ブラウザでGitHubのアクセスを許可した後に「GitHub CLIのログインが完了しませんでした」となり、プロファイルのGitHub連携が保存されない問題。ワンタイムコードを読んだ後もGitHub CLIの出力を最後まで読むようにした
- Tauri WebViewで`window.confirm`が表示されないまま設定や削除が進む問題。自動承認はチェック時に保存し、GitContextからの削除は画面内に「削除する／やめる」の確認を表示
- MCPの即時自動応答を人の拒否・承認と区別して拒否し、監査ログにも記録。Claude CodeのデスクトップCodeタブを補助的に検出して確認画面を出さずに拒否し、`--trust-client-approval`指定時も対応クライアントではGitContextの確認画面を優先
- MCPのクライアント情報とelicitation対応をリクエストごとに判定し、モダンプロトコルの`_meta`とMRTRによる確認に対応。レガシーの`elicitation/create`も維持
- Profileから外したGitHubユーザー名・gh設定ディレクトリ・SSH鍵の設定が、再適用後もリポジトリのローカル設定に残る問題（適用前のプレビューに「削除」と表示し、GitContextが書いた`core.sshCommand`だけを削除）

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
