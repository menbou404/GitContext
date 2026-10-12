# AIクライアントごとの公開範囲の設計

## 目的

1. **公開する範囲（Tier）をGitContextで決める。** これまでは、AIクライアントの登録（起動の引数`--max-tier`）で範囲が決まり、範囲を変えるにはクライアントの設定を書き換えてMCPサーバーを起動し直す必要があった。GitContextのGUIで範囲を変えるだけで、つなぎ直さずに反映されるようにする。
2. **AIが自分で範囲を広げられないようにする。** シェルを使えるAIは、クライアントの設定ファイル（`~/.claude.json`、`~/.codex/config.toml`）を書き換えて、範囲を広げたり`--trust-client-approval`を付けたりできる。GUIで許可した範囲を、引数で超えられないようにする。

## 前提と限界

- AIクライアントとGitContextは、同じWindowsユーザーとして動く。同じユーザーとして動くプログラムによるファイルの書き換えを、GitContextが完全に防ぐことはできない（暗号化しても、同じユーザーなら鍵を読める）。
- そのため、守りを次の2層に分ける。
  1. **AIクライアント側の権限**（実際に最も効く）: Claude Codeの権限ルール、Codexのサンドボックスで、AIのシェルが設定ファイルや`gh`・`git push`を扱えないようにする。
  2. **GitContext側の上限と検出**（この設計）: 引数で範囲を広げても上限を超えないようにし、GUI以外による設定の変更を検出して止める。
- AIがシェルで`git push`や`gh`を直接実行することは、GitContextでは防げない（[BACKGROUND_DESIGN.md](BACKGROUND_DESIGN.md)の「安全性」と同じ）。

## 範囲の決め方

MCPサーバーが使う範囲は、次の小さい方とする。

- **上限A（起動の引数）:** `--max-tier`。省略時は`read`（従来どおり）。
- **上限B（GitContextの設定）:** クライアントごとの「公開する範囲」。

`--trust-client-approval`も同じ考え方で、引数に付いていて、かつGitContextの設定で許可したクライアントだけ有効にする。

### クライアントの識別

- `clientInfo.name`（モダンプロトコルは各リクエストの`_meta`、レガシーは`initialize`）で識別する。`claude-code`、`codex-mcp-client`など。
- Claude CodeのCLIとCodeタブは同じ`claude-code`のため、CodeタブはCLAUDE_CODE_DESKTOP_APP_VERSIONで補助的に区別する（既存の判定）。設定の単位は「Claude Code（CLI）」「Claude Code（Codeタブ）」「Codex」「その他」とする。
- `clientInfo.name`はクライアント自身の申告で、AIが書き換えるものではない。ただし、AIが別のクライアント名を名乗るプログラムからMCPサーバーを起動することはできるため、「その他」の既定は`read`とする。

### 設定の保存

状態ファイルの`settings.aiClients`に保存する。

| 項目 | 内容 | 既定 |
|---|---|---|
| `claudeCode.maxTier` | Claude Code（CLI）の範囲（`read`／`local`／`remote`） | `read` |
| `claudeCodeDesktop.maxTier` | Claude Code（Codeタブ）の範囲 | `read` |
| `codex.maxTier` | Codex（CLI／Desktop）の範囲 | `read` |
| `other.maxTier` | それ以外のクライアント | `read` |
| `*.allowTrustClientApproval` | `--trust-client-approval`を有効にしてよいか | false |

- 移行: 既存の登録で`--max-tier`を付けている利用者が、更新しただけで範囲が狭まらないよう、この項目がない古い状態ファイルを初めて読み込んだときは、AI連携画面で検出できる登録の引数から初期値を決める（GUIで1回だけ行う。MCPサーバーは行わない）。検出できない場合は`read`。

### ツールの一覧と反映

- `tools/list`は、そのリクエストのクライアントの範囲に含まれるツールだけを返す。範囲の外のツールを呼び出した場合は、従来どおり拒否する。
- MCPサーバーは状態ファイルの更新を数秒ごとに確認し、クライアントの範囲が変わったら`notifications/tools/list_changed`を送る。クライアントはツールの一覧を取り直す（Claude Codeは対応）。つなぎ直しは不要。
- サーバーの起動時の`initialize`／`server/discover`で、ツールの一覧が変わりうることを宣言する（`tools.listChanged: true`）。

## 登録

- AI連携画面からの登録では、起動の引数を上限として`--max-tier remote`（GitHub操作まで）を付ける。実際の範囲はGitContextの設定で決まる。
- `--trust-client-approval`は、AI連携画面で警告に同意した場合だけ付ける（従来どおり）。付けても、GitContextの設定で許可しない限り有効にならない。
- 既存の登録（引数なし、または`--max-tier`付き）はそのまま動く。上限Aが`read`の登録は、AI連携画面で「範囲を広げるには登録の更新が必要」と表示する。

## AI連携画面

クライアントの行に次を表示する。

- 公開する範囲（読み取りのみ／ローカルの変更まで／GitHub操作まで）の選択。変更は即時に保存し、つないでいるクライアントへ反映される。「GitHub操作まで」には危険の色の注記を出す。
- 確認画面を出せないクライアントでの「クライアント自身の確認に任せる」（`allowTrustClientApproval`）。警告と同意を求める（従来の警告文）。
- 登録の引数と設定の関係:
  - 登録の上限が設定より狭い場合: 「登録の更新が必要」と表示し、更新の操作を出す（従来の「範囲の変更」と同じ流れ）。
  - 登録の引数が、GitContextの設定にない`--trust-client-approval`を含む場合や、AI連携画面以外で変更されたとみられる場合: 「要確認」と表示する。

## GUI以外による変更の検出

GitContextが起動している間、次の変更を検出して知らせる。

1. **状態ファイルの安全に関わる設定**（`settings.aiClients`、リポジトリとプロファイルの`autoApprove`、`newRepositoryFolders`）が、GUIの操作でもMCPの正規の処理（既定値の引き継ぎ。監査ログに記録される）でもなく変わった場合:
   - GUIは直前に自分が保存した内容を覚えておき、変わった項目を元に戻す。
   - Windowsの通知と履歴画面に「GitContext以外で安全に関わる設定が変更されたため、元に戻しました」と記録する。
2. **AIクライアントの登録**（`~/.claude.json`、`~/.codex/config.toml`、Claude Desktopの設定）でGitContextの登録の引数が変わった場合: AI連携画面に「要確認」を表示し、通知する。登録はクライアント側のファイルのため、自動では元に戻さない。

GitContextが起動していない間の変更は、次の起動時に、前回保存した内容と比べて検出する（前回の内容は状態ファイルとは別のファイルに保存する。同じユーザーのプログラムなら書き換えられるため、完全ではない）。

## AIクライアント側の権限ルール（推奨）

AI連携画面に、AIクライアント側で設定ファイルの変更を防ぐためのルールを案内する。

- Claude Code（`~/.claude/settings.json`の`permissions`）: 次を`deny`または`ask`にする。
  - `Edit`: `~/.claude.json`、`~/.gitcontext/**`、`~/.gitcontext-dev/**`、`~/.codex/config.toml`
  - `Bash`: `claude mcp add *`、`claude mcp remove *`、`gh *`、`git push *`
- Codex: サンドボックスを`workspace-write`にし、作業フォルダの外へ書き込ませない。
- 画面では、追加するルールの内容を表示し、利用者が同意した場合だけ書き込む（設定ファイルをバックアップしてから、既存のルールを保ったまま追加する）。

## 段階

| 段階 | 内容 |
|---|---|
| C-1 | `settings.aiClients`、範囲の決め方（上限A・Bの小さい方）、クライアントの識別、`tools/list`の絞り込みと`tools/list_changed`、`--trust-client-approval`の許可、AI連携画面での範囲の選択、登録の上限を`remote`にする変更、移行 |
| C-2 | GUI以外による変更の検出（状態ファイルの安全に関わる設定の差し戻し、登録の引数の変化、起動時の比較） |
| C-3 | AIクライアント側の権限ルールの案内と書き込み |

各段階を1つのPRとする。
