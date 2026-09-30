# GitContext MCPサーバー設計

## 目的

Claude CodeなどのAIエージェントが、GitContextに登録したRepository Identity / Profileを通してGit・GitHubを操作できるようにする。

AIエージェントは通常、グローバルなGit author、既定のSSH鍵、グローバルな`gh`認証をそのまま使う。複数のGitHubアカウントを使い分ける環境では、別アカウントでのcommitやpushが起きやすい。GitContextのMCPサーバーは「リポジトリに割り当てたProfile以外では操作できない」経路をAIへ提供し、アカウント分離をAI操作にも適用する。

## 前提

- GUI版の安全境界（[MVP_DESIGN.md](MVP_DESIGN.md)）をMCP経由でも維持する。MCPは新しい権限を増やすのではなく、既存の検証済み操作を別の入口から呼べるようにするだけとする。
- GUIでは「プレビューを人が確認してから実行する」ことで安全性を保っている。MCPでは人が画面を見ない可能性があるため、確認の代わりになる仕組みを設計に組み込む。
- 対象OSはGUI版と同じくWindowsとする。

## 非目標

- Profileの作成・編集・削除、GitHub CLIのブラウザ認証はMCPから提供しない。アカウントの登録と認証は人がGUIで行う。
- 任意のgit／ghコマンドを実行する汎用ツールは提供しない。
- token、SSH秘密鍵の内容、ワンタイムコードをMCPの結果に含めない。
- リモートからの接続（HTTP transport）は提供しない。ローカルのstdio接続だけとする。

## 全体構成

```text
src-tauri/                     (workspace)
├─ core/   gitcontext-core     … Tauri非依存の業務ロジック・検証・状態保存
├─ app/    git-context (Tauri) … GUI。coreを呼ぶ薄いcommand層
└─ mcp/    gitcontext-mcp      … MCPサーバー（stdio）。coreを呼ぶ
```

- 現在の[commands.rs](../src-tauri/src/commands.rs)には、検証・状態更新・Git/gh呼び出しとTauri固有処理（`AppHandle`、`State`、event送信、`spawn_blocking`）が混在している。業務ロジックを`gitcontext-core`へ移し、Tauri commandとMCP toolはどちらもcoreの同じ関数を呼ぶ。
- [git_ops.rs](../src-tauri/src/git_ops.rs)と[models.rs](../src-tauri/src/models.rs)はTauriに依存していないため、そのままcoreへ移す。
- MCPサーバーは公式Rust SDK `rmcp`（`server`、`transport-io`、`macros`）で実装する。
- GUIとMCPサーバーは別プロセスとして動き、同じ`state.json`を共有する。

## 状態ファイルの共有

### 保存先の解決

[storage.rs](../src-tauri/src/storage.rs)は現在`AppHandle`から`app_config_dir()`を得ている。coreでは`AppHandle`に依存せず、次の順で解決する。

1. 呼び出し側が明示したディレクトリ（テスト用）
2. `%APPDATA%\app.gitcontext.desktop`（Tauriの`identifier`と同じ場所）

GUIとMCPサーバーが同じ場所を指すことをテストで確認する。

### プロセス間の排他

現在の`AppGate`（`Mutex`）は同一プロセス内でしか効かない。GUIとMCPサーバーが同時に書き込むと更新が失われるため、状態ディレクトリに`state.lock`を置き、読み込みから保存までをOSのファイルロックで保護する。

- ロックは標準ライブラリの`File::lock`（Rust 1.89以降）を使う。
- ロック取得は最大数秒で打ち切り、「GitContextの別の操作が実行中です」と返す。
- 長時間かかるGit/gh操作の間はロックを持たない。現在のcommandと同じく、状態の読み取り → ロック解放 → 外部コマンド実行 → 再ロックして保存、の順に行う。

### GUIへの反映

MCPサーバーが状態を変えた後、起動中のGUIは古い状態を表示し続ける。GUIはウィンドウのフォーカス時と操作の直前に`state.json`を再読み込みする。

## アカウント分離の保証

- **すべてのツールは`repositoryId`を受け取り、Profileは割り当てから決める。** push、commit、PR作成、mergeなどでは、AIが`profileId`を指定できない。Profileを選べるのは`preview_assignment`と`apply_profile`だけとする。
- 実行時には、GUIと同じ検証（`ensure_applied_assignment`、repository-localの`user.name`／`user.email`／`core.sshCommand`／`gitcontext.profileId`の一致、`connected_gh_directory`によるGitHubユーザー名の一致）を必ず通す。
- ghは常にProfile専用の`GH_CONFIG_DIR`で実行し、`GH_TOKEN`などのtoken環境変数を子プロセスから除く（既存の`gh_command`）。グローバルな`gh`認証は使わない。
- MCPサーバー自身の環境変数（`GH_TOKEN`など）は子プロセスへ渡さない。

## ツールの段階分け

MCPのツールは危険度で3段階に分ける。サーバー起動時の`--max-tier`で公開する範囲を決め、既定値は`read`とする。

| Tier | 公開される操作 | 起動オプション |
|---|---|---|
| `read` | 状態の確認とプレビュー | `--max-tier read`（既定） |
| `local` | ローカルリポジトリの変更 | `--max-tier local` |
| `remote` | GitHubやリモートへの変更 | `--max-tier remote` |

### Tier `read`

| ツール | 内容 | 対応する既存処理 |
|---|---|---|
| `list_profiles` | Profile一覧（ID、表示名、Git author、GitHubユーザー名、SSH鍵の有無、GitHub接続状態） | `bootstrap`、`inspect_github_profile` |
| `list_repositories` | 登録済みリポジトリと割り当てProfile | `bootstrap` |
| `get_repository_status` | 現在ブランチ、変更ファイル、追跡ブランチ、repository-localのidentityと割り当てProfileの一致状態 | `build_commit_preview`など |
| `find_repository` | ローカルパスから登録済みリポジトリを探す（AIの作業ディレクトリとの対応付け用） | 新規 |
| `suggest_profile` | originのownerとProfileのGitHubユーザー名から、割り当て候補を返す（書き込みはしない） | 新規（CHANGELOGのPlannedにある照合機能） |
| `preview_assignment` | Profile適用の差分 | `preview_assignment` |
| `preview_commit` | commit対象ファイルとメッセージ検証 | `preview_commit` |
| `preview_push` | push先、ブランチ、未コミット変更の有無 | `preview_push` |
| `preview_sync` | Fetch後のahead/behind | `preview_repository_sync` |
| `preview_pull_request` | 既定ブランチ、作業ブランチ作成の要否、既存PR | `preview_pull_request` |
| `list_pull_requests` | 開いているPR、CI、競合、レビュー状態 | `list_pull_requests` |

`preview_sync`はfetchを行うが、リモートを変更せず、Profileの鍵で読み取るだけなので`read`に含める。

### Tier `local`

| ツール | 内容 | 必須入力 |
|---|---|---|
| `add_repository` | 既存のローカルGitリポジトリを登録 | パス |
| `apply_profile` | Profileをrepository-local設定へ適用 | `previewId` |
| `create_branch` | 既定ブランチから作業ブランチを作成 | `previewId`、ブランチ名 |
| `commit` | プレビュー時のファイルだけをcommit | `previewId`、メッセージ |
| `pull` | Fast-forward Pull | `previewId` |

### Tier `remote`

| ツール | 内容 | 必須入力 |
|---|---|---|
| `push` | 現在ブランチを通常push | `previewId` |
| `create_pull_request` | PR作成（既存PRがあれば再利用） | `previewId`、タイトル、本文、Draft |
| `merge_pull_request` | 先頭commitを固定してmerge | `previewId`、merge方法 |
| `clone_repository` | ProfileのSSH鍵でclone、登録、適用 | Profile、SSH URL、保存先 |
| `publish_repository` | GitHubリポジトリ作成と初回push | `previewId`、名前、説明、公開範囲 |

### MCPでは提供しない操作

- Profileの作成・編集・削除
- GitHub CLIのブラウザ認証と認証ページの起動
- リポジトリ登録の削除
- force push、任意refspec、タグのpush、ブランチ削除、管理者権限によるmerge（GUIでも提供していない）

## プレビューIDによる実行の固定

GUIでは人がプレビューを見てから実行するが、MCPではAIがプレビューを省略したり、プレビュー後に状態が変わったりする。そこで、`local`と`remote`の実行ツールは、直前のプレビューで発行した`previewId`がないと動かないようにする。

1. プレビューツールは結果と一緒に`previewId`を返す。
2. サーバーは`previewId`に次の「指紋」を紐付けて、メモリ上に保存する。
   - リポジトリID、Profile ID、操作の種類
   - `HEAD`のcommit ID、現在ブランチ、origin URL
   - 変更ファイル一覧とその状態（commitの場合）
   - PR番号と先頭commit ID（mergeの場合）
   - repository-localのidentity設定値（Profile適用の場合）
3. 実行ツールは指紋を取り直し、保存したものと完全に一致する場合だけ実行する。一致しなければ「状態が変わりました。プレビューからやり直してください」と返す。
4. `previewId`は1回だけ使え、10分で失効する。サーバーを再起動すると、すべて無効になる。

特にcommitは、GUIでは`git add --all`で全変更を対象にしている。MCPでは、プレビュー後にAIが作ったファイル（`.env`など）が紛れ込むのを防ぐため、プレビュー時のファイル一覧と完全に一致する場合だけcommitする。

既存のmergeの`expected_head_oid`／`--match-head-commit`は、この仕組みの特殊な場合にあたる。

## 人の確認

- ツールにはMCPのannotationsを付ける。`read`は`readOnlyHint: true`、`remote`は`destructiveHint: true`と`openWorldHint: true`とする。
- Claude Codeなどのクライアントは、ツールの呼び出しごとに確認を出せる。`remote`のツールは確認を出す設定を推奨し、READMEに設定例を載せる。
- 実行ツールの結果には、実行したこと（Profile、ブランチ、commit ID、PR URLなど）を必ず含める。
- 将来の拡張として、`remote`の実行時にGUI側で承認ダイアログを出す方式を検討する（未決事項を参照）。

## プロンプトインジェクションと入力検証

- PRのタイトルや本文、commitメッセージ、ファイル名、ブランチ名など、リポジトリやGitHub由来の文字列は、ツールの結果の中で「外部データ」として区別して返す。これらに含まれる指示には従わないよう、ツールの説明文にも明記する。
- 入力の検証は既存の関数（`validate_github_ssh_url`、`validate_pull_request_title`、`validate_pull_request_body`、commitメッセージの1行200文字制限、Git標準のブランチ名検証）を再利用する。
- `clone_repository`の保存先は、ユーザーのhome配下にある既存ディレクトリに限る。既存のパスは上書きしない。
- `add_repository`は、GUIと同じく`git rev-parse --show-toplevel`でリポジトリrootと一致するパスだけを受け付ける。

## 秘密情報

- ツールの結果にtoken、SSH秘密鍵の内容、ワンタイムコードを含めない。
- SSH鍵は「設定の有無」と「ファイル名」だけを返し、フルパスは返さない。
- gh設定ディレクトリのパスは返さない。

## 監査ログ

MCPから実行した`local`と`remote`の操作を、状態ディレクトリの`mcp-audit.jsonl`に1行ずつ記録する。

- 記録する項目: 日時、ツール名、リポジトリID、Profile ID、主要な結果（commit ID、PR番号など）、成否
- 秘密情報とファイルの中身は記録しない。
- CHANGELOGのPlannedにある「適用履歴とワンクリックrollback」の土台として、将来はGUIでも表示する。

## クライアントの設定

### Claude Code

```powershell
claude mcp add gitcontext -- "C:\Program Files\GitContext\gitcontext-mcp.exe" --max-tier remote
```

`remote`のツールには、Claude Codeの権限設定で確認を出すルールを推奨する。

### Codex

CodexのMCPサーバーはサンドボックスの外で動くため、`remote`を公開するとCodexがpushやmergeを実行できてしまう。[CONTRIBUTING.md](../CONTRIBUTING.md)の役割分担に合わせ、Codexには`--max-tier read`だけを設定する。

## 配布

- `gitcontext-mcp.exe`をGUIのインストーラーに同梱し、GUIと同じバージョンで配布する。
- GUIの設定画面にMCPの接続手順と、クライアントごとの設定例を表示する。

## テスト方針

- coreの切り出しでは、既存のRustテスト14件と、フロントエンドのテストが変わらず通ることを確認する。
- 状態ディレクトリを一時ディレクトリへ向け、テスト用のGitリポジトリで各ツールを検証する。
- プレビューIDについては、次の拒否ケースをテストする。
  - 使用済みのID
  - 失効したID
  - 別のリポジトリのID
  - プレビュー後にHEADが変わった場合
  - プレビュー後にファイルが増えた場合
- ロックについては、GUIとMCPの同時書き込みを模したテストで、更新が失われないことを確認する。
- `--max-tier`より上のツールが、一覧に出ず、呼び出しも拒否されることを確認する。

## 段階的な実装計画

| 段階 | 内容 | 完了条件 |
|---|---|---|
| 1 | `gitcontext-core`の切り出し、保存先の解決、プロセス間ロック | 動作を変えずに既存テストが通る。GUIの挙動が変わらない |
| 2 | `gitcontext-mcp`の`read`ツール | Claude Codeから状態とプレビューを取得できる |
| 3 | プレビューID、監査ログ、`local`ツール | 拒否ケースのテストが通る |
| 4 | `remote`ツール | GUIと同じ検証を通り、Codex向けの`read`制限が効く |
| 5 | インストーラーへの同梱、GUIの設定画面、README | 配布物から設定できる |

各段階を1つのPRとする。

## 未決事項

- **対象クライアント:** 現時点ではClaude Codeを主な対象とし、Codexは`read`だけとする案で進める。Claude Desktopへの対応が必要か確認する。
- **`remote`の範囲:** push、PR作成、mergeまでMCPで提供するか。提供しない場合は段階4を省く。
- **GUIでの承認:** `remote`の実行時にGUIで承認させるか。GUIの起動が前提になる代わりに、クライアントの設定に頼らずに人の確認を保証できる。
- **`previewId`の保存:** 保存先をサーバープロセスのメモリにするか。サーバーを再起動すると失効するが、ファイルに保存するより漏れや再利用のリスクが小さい。
