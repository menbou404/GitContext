# GitContext MCPサーバー設計

## 目的

Claude Code、Codex、Claude Desktop、Codex DesktopなどのAIエージェントが、GitContextに登録したRepository Identity / Profileを通してGit・GitHubを操作できるようにする。

AIエージェントは通常、グローバルなGit author、既定のSSH鍵、グローバルな`gh`認証をそのまま使う。複数のGitHubアカウントを使い分ける環境では、別アカウントでのcommitやpushが起きやすい。GitContextのMCPサーバーは「リポジトリに割り当てたProfile以外では操作できない」経路をAIへ提供し、アカウント分離をAI操作にも適用する。

## 前提

- GUI版の安全境界（[MVP_DESIGN.md](MVP_DESIGN.md)）をMCP経由でも維持する。MCPは新しい権限を増やすのではなく、既存の検証済み操作を別の入口から呼べるようにするだけとする。
- GUIでは「プレビューを人が確認してから実行する」ことで安全性を保っている。MCPでは人が画面を見ない可能性があるため、確認の代わりになる仕組みを設計に組み込む。
- 対象OSはGUI版と同じくWindowsとする。
- 特定のクライアントに依存しない。ローカルのstdio接続に対応したMCPクライアント（Claude Code、Codex CLI、Claude Desktop、Codex Desktopなど）であれば同じサーバーを使える。
- GUIの起動を前提にしない。MCPサーバー単体で動作し、安全性もサーバー側の仕組み（Tierの制限、プレビューID、確認要求）で保つ。

## 非目標

- Profileの作成・編集・削除、GitHub CLIのブラウザ認証はMCPから提供しない。アカウントの登録と認証は人がGUIで行う。
- 任意のgit／ghコマンドを実行する汎用ツールは提供しない。
- token、SSH秘密鍵の内容、ワンタイムコードをMCPの結果に含めない。
- リモートからの接続（HTTP transport）は提供しない。ローカルのstdio接続だけとする。

## 全体構成

```text
src-tauri/                          (workspace root + Tauri app package)
├─ src/                             git-context (Tauri) … GUI。coreを呼ぶ薄いcommand層
└─ crates/
   ├─ gitcontext-core/              … Tauri非依存の業務ロジック・検証・状態保存
   └─ gitcontext-mcp/               … MCPサーバー（stdio）。coreを呼ぶ（段階2で追加）
```

- Tauri CLIは`src-tauri/`直下にアプリがある構成を前提にしているため、GUIは移動せず、`src-tauri/Cargo.toml`をworkspaceのルートにする。
- Tauri commandには、検証・状態更新・Git/gh呼び出しとTauri固有処理（`AppHandle`、`State`、event送信、`spawn_blocking`）が混在していた。業務ロジックを`gitcontext-core`へ移し、Tauri commandとMCP toolはどちらもcoreの同じ関数を呼ぶ。
- `git_ops`と`models`はTauriに依存していないため、そのままcoreへ移す。
- workspaceのテストと整形は`cargo test --workspace`、`cargo fmt --all`で行う。
- MCPサーバーは公式Rust SDK `rmcp`（`server`、`transport-io`、`macros`）で実装する。
- GUIとMCPサーバーは別プロセスとして動き、同じ`state.json`を共有する。

## 状態ファイルの共有

### 保存先の解決

coreの`StateStore`は状態ディレクトリを引数で受け取り、`AppHandle`に依存しない。

- GUIとMCPサーバーはcoreの共通関数で`StateStore`を作り、Windowsでは配布版を`%USERPROFILE%\.gitcontext`、開発版を`%USERPROFILE%\.gitcontext-dev`に保存する。
- パッケージアプリから起動した子プロセスではAppDataへの書き込みが別の場所に振り替えられることがある。ホーム配下に置くことでGUIとMCPが同じデータを参照する。旧AppData保存先からは`state.json`が新保存先にない場合だけ自動移行し、旧データは残す。
- テストでは一時ディレクトリを渡す。

### プロセス間の排他

従来の`AppGate`（`Mutex`）は同一プロセス内でしか効かない。GUIとMCPサーバーが同時に書き込むと更新が失われるため、状態ディレクトリに`state.lock`を置き、読み込みから保存までをOSのファイルロックで保護する。

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

| ツール | 内容 | 必須入力 | previewIdを発行するプレビュー |
|---|---|---|---|
| `push` | 現在ブランチを通常push | `previewId` | `preview_push` |
| `create_pull_request` | PR作成（既存PRがあれば再利用） | `previewId`、タイトル、本文、Draft | `preview_pull_request`（作業ブランチ上のとき） |
| `merge_pull_request` | 先頭commitを固定してmerge | `previewId`、merge方法 | `preview_merge`（PR番号を指定） |
| `clone_repository` | ProfileのSSH鍵でclone、登録、適用 | `previewId` | `preview_clone`（Profile、SSH URL、保存先を指定） |
| `publish_repository` | GitHubリポジトリ作成と初回push | `previewId` | `preview_publish`（名前、説明、公開範囲を指定） |

- `preview_pull_request`は、既定ブランチ上なら`create_branch`用、作業ブランチ上なら`create_pull_request`用の`previewId`を発行する。
- `preview_merge`、`preview_clone`、`preview_publish`は`remote`のときだけ公開するプレビューで、実行に必要な入力をプレビュー時に固定する。

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

### 保存先をメモリにする理由

stdio接続のMCPサーバーは、クライアントごとに別プロセスとして起動する。Claude CodeとCodexを同時に使えば、サーバーも2つ動く。

- `previewId`と指紋はサーバープロセスのメモリだけに置き、ファイルには書かない。
- 他のプロセスからは見えないため、別のクライアントが発行したIDを使い回したり、ファイルから盗み見たりできない。
- サーバーの終了とともに消えるため、古いIDが残らない。
- 再起動でIDが無効になった場合は、プレビューを取り直せばよい。

特にcommitは、GUIでは`git add --all`で全変更を対象にしている。MCPでは、プレビュー後にAIが作ったファイル（`.env`など）が紛れ込むのを防ぐため、プレビュー時のファイル一覧と完全に一致する場合だけcommitする。さらに、commitの直前にプレビュー時のファイルだけを明示してステージし、ステージ済みの集合がプレビューと一致しない場合はインデックスを元に戻してcommitしない。

**既知の制限（段階3時点）**: `pull`は実行時にもう一度fetchするため、プレビュー後にリモートへ追加されたcommitも取り込む可能性がある（Fast-forwardに限るため、ローカルの変更が失われることはない）。プレビュー時の追跡先commitまでだけを取り込むよう、今後修正する。

既存のmergeの`expected_head_oid`／`--match-head-commit`は、この仕組みの特殊な場合にあたる。

## 人の確認

GUIでの承認は行わない。MCPサーバー単体で使う人がいるため、確認はMCPの仕組みの中で完結させる。

### サーバーからの確認要求（elicitation）

MCPのelicitationを使い、`remote`の実行前にサーバーからクライアントへ確認を依頼する。クライアントは画面で人に確認し、その結果をサーバーへ返す。

- 確認の文面には、操作の種類、Profile、GitHubユーザー名、リポジトリ、ブランチ、対象（commit ID、PR番号、merge方法、公開範囲など）を表示する。
- 人が承認した場合だけ実行する。拒否、キャンセル、**2分以内に応答がない場合**は、何もせずに理由を返す（応答しないまま止まるクライアントがあるため）。
- AIが確認を代わりに承認できないよう、確認の結果はツールの引数ではなく、elicitationの応答だけで受け取る。
- 人が確認している間に状態が変わることがあるため、承認を受けた後にもう一度指紋を照合してから実行する。

### クライアントの対応状況

2026年10月時点の公開情報では、elicitationへの対応はクライアントによって異なる。

| クライアント | 状況 |
|---|---|
| Claude Code（CLI） | 対応 |
| Claude Code（デスクトップのCodeタブ）、Codex Desktop | 確認フォームが表示されずに閉じられるという報告がある |
| Codex CLI | 対応が進行中。自動で拒否を返す場合があるという報告がある |
| Claude Desktop | 非対応 |
| Claude Cowork | 対応を宣言するが確認依頼を無視し、呼び出しが止まるという報告がある |

版によって変わるため、実際の動作は各クライアントで確認し、この表と下記の一覧を更新する。

### 確認の方法の決め方

既定では、人の確認なしにGitHubが変更されないようにする。そのうえで、確認画面を出せないクライアントでも、利用者が警告を理解して選んだ場合に限り、クライアント自身の確認に任せられるようにする。

| クライアントの状態 | `--trust-client-approval`なし（既定） | `--trust-client-approval`あり |
|---|---|---|
| elicitationに対応し、不安定な一覧にない | elicitationで確認する | elicitationを使わず、クライアントの確認に任せる |
| elicitationに対応していない、または不安定な一覧にある | 実行しない。AI連携画面で設定を見直すよう案内する | クライアントの確認に任せて実行する |

- elicitationの「対応」は、フォーム形式の確認を宣言している場合だけとみなす。URLを開かせる形式だけを宣言しているクライアントは「対応していない」として扱う。
- クライアント名とelicitation対応はリクエストごとに判定する。モダンプロトコルでは各リクエストの`_meta`にある`io.modelcontextprotocol/clientInfo`と`io.modelcontextprotocol/clientCapabilities`を使い、レガシープロトコルでは`initialize`の情報にフォールバックする。監査ログのクライアント名にも同じ情報を使う。
- **不安定な一覧**: 対応を宣言しながら確認フォームを表示しないことが分かっているクライアントを、当該リクエストの`clientInfo.name`で判定する一覧。サーバーには区別がつかない「自動の拒否」を人の拒否と取り違えないためのもので、アプリの更新に合わせて見直す。
- `--trust-client-approval`は、GUIの「AI連携」画面で、確認画面を出せないクライアントに「GitHub操作まで」を選び、警告に同意したときだけ登録時に付ける（[UI_DESIGN.md](UI_DESIGN.md)参照）。画面ができるまでは手で付ける。
- どの方法でも、プレビューIDの照合は必ず行う。
- 監査ログには、確認の方法（`elicitation`／`client`）と結果を記録する。クライアントに任せた場合は、実行結果にもその旨を含める。

### その他

- ツールにはMCPのannotationsを付ける。`read`は`readOnlyHint: true`、`remote`の実行ツールは`destructiveHint: true`と`openWorldHint: true`とする。
- 実行ツールの結果には、実行したこと（Profile、ブランチ、commit ID、PR URLなど）を必ず含める。

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

どのクライアントでも、実行ファイルのパスと起動オプションを登録するだけで使える。`--max-tier`を省略した場合は`read`だけが公開される。以下の例のパスは、インストール先に合わせて読み替える。

### Claude Code

```powershell
claude mcp add gitcontext -- "C:\Program Files\GitContext\gitcontext-mcp.exe" --max-tier remote
```

### Claude Desktop

`%APPDATA%\Claude\claude_desktop_config.json`に追加する。

```json
{
  "mcpServers": {
    "gitcontext": {
      "command": "C:\\Program Files\\GitContext\\gitcontext-mcp.exe",
      "args": ["--max-tier", "remote"]
    }
  }
}
```

### Codex CLI / Codex Desktop

どちらも`~/.codex/config.toml`の設定を使う。

```toml
[mcp_servers.gitcontext]
command = 'C:\Program Files\GitContext\gitcontext-mcp.exe'
args = ["--max-tier", "remote"]
```

### Tierの選び方

- まずは既定の`read`で使い始め、必要になったら`local`、`remote`へ広げることを推奨する。
- CodexのMCPサーバーはCodexのサンドボックスの外で動く。サンドボックスでGit操作を制限していても、`remote`を公開すればMCP経由でpushやmergeができる点に注意する。
- GitContext自体の開発では、[CONTRIBUTING.md](../CONTRIBUTING.md)の役割分担に合わせて、Codexには`read`だけを設定する。
- `remote`を、確認画面を出せないクライアント（上記「クライアントの対応状況」）で使うには`--trust-client-approval`が必要になる。付けた場合、GitHub操作の前の確認はクライアント自身の確認だけになる。クライアント側で自動承認を有効にしていると、確認なしにpushやmergeが実行されるため、GitContextのツールには毎回確認を出す設定にすること。

## 配布

- `gitcontext-mcp.exe`をGUIのインストーラーに同梱し、GUIと同じバージョンで配布する。
- **インストーラーからAIクライアントへ直接登録しない。** 理由は次のとおり。
  - MSIは管理者権限で動くため、実際に使う利用者とは別のユーザーの設定へ書き込むおそれがある。
  - 登録用の公式CLIがないクライアント（Claude Desktop）や、CLIがPATHにない環境がある。
  - アンインストール時に登録を消せず、存在しない実行ファイルを指す登録が残る。
  - 他のアプリの設定を、差分を見せずに書き換えることになる。
- 代わりに、GUIの「AI連携」画面から、利用者の同意を得て登録する（[UI_DESIGN.md](UI_DESIGN.md)参照）。クライアントの検出、書き込む設定ファイルの差分表示、書き込み前のバックアップ、公開する範囲の選択（既定は読み取りのみ）、解除、登録先が見つからない場合の修復を提供する。
- インストーラーの最後に「起動後にAI連携を設定する」を置き、チェックするとこの画面を開く。
- 開発版のMCPサーバーは`gitcontext-dev`、配布版は`gitcontext`という登録名で区別する。

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
- elicitationについては、承認・拒否・キャンセル・時間切れ・非対応クライアント・不安定な一覧にあるクライアント（`--trust-client-approval`の有無）の各場合と、承認後の指紋の再照合をテストする。
- 実際のクライアント（Claude Code、Codex CLI、Claude Desktop、Codex Desktop）で、接続とツール一覧の取得を手動で確認する。

## 段階的な実装計画

| 段階 | 内容 | 完了条件 | 状況 |
|---|---|---|---|
| 1 | `gitcontext-core`の切り出し、保存先の解決、プロセス間ロック | 動作を変えずに既存テストが通る。GUIの挙動が変わらない | 完了 |
| 2 | `gitcontext-mcp`の`read`ツール | 4つのクライアントから状態とプレビューを取得できる | 実装済み。stdioの結合テストと実データでの動作を確認。各クライアントからの接続確認は未実施 |
| 3 | プレビューID、監査ログ、`local`ツール | 拒否ケースのテストが通る | 実装済み。`pull`の既知の制限あり（上記） |
| 4 | elicitationによる確認と`remote`ツール | GUIと同じ検証を通り、確認の各ケースのテストが通る | 実装済み。実際のクライアントでの確認画面の動作と、不安定な一覧の中身は未確認 |
| 5 | インストーラーへの同梱、GUIの「AI連携」画面、README（クライアント別の設定例） | 配布物から設定できる | 未着手 |

各段階を1つのPRとする。

## 決定事項

- **対象クライアント:** 特定のクライアントに依存せず、Claude Code、Codex CLI、Claude Desktop、Codex Desktopを想定する。
- **GUIでの承認:** 行わない。MCPサーバー単体で動作させ、人の確認はelicitationで行う。
- **`previewId`の保存先:** サーバープロセスのメモリとする。
- **`remote`の範囲:** push、PR作成、merge、clone、GitHub公開をすべて提供する。ただし`--max-tier remote`を明示して起動した場合だけ公開する。
- **確認画面を出せないクライアント:** 既定では`remote`を実行しない。警告に同意したうえで`--trust-client-approval`を付けた場合だけ、クライアント自身の確認に任せる（2026年10月決定。当初案の「既定でクライアントの確認に任せ、`--require-confirmation`で拒否する」から変更）。

## 未決事項

- **不安定な一覧の中身:** 各クライアントの`clientInfo.name`と実際の動作を確認し、一覧と「クライアントの対応状況」の表を更新する。
