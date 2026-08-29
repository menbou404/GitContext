# GitContext

GitContextは、学校用・趣味用など複数のGitHub作業identityを、リポジトリ単位で割り当てるTauriデスクトップアプリです。

「アカウントを切り替える」のではなく、各repositoryに自分で名前を付けたProfileを一度割り当てます。`個人用`、`研究室`、`サークル` など、Profileは必要なタイミングでいくつでも追加でき、Gitのauthor、SSH identity、GitHub CLI設定への参照を同じProfileとして扱います。

## MVPでできること

- 任意名のProfileの作成・編集（固定のPersonal / School区分はなし）
- Profile作成画面でGitHub CLIアカウント、Git identity、既存SSH鍵をまとめて紐付け
- ローカルGit repositoryの追加
- repositoryごとのProfile割当
- 適用前の差分レビュー
- `.git/config` へのrepository-local設定反映
- 接続済みProfileを使ったGitHubリポジトリ作成、`origin`設定、初回push
- 既存の `~/.ssh` と `gh` 設定を参照（秘密情報は保存しない）

設計と安全境界は [docs/MVP_DESIGN.md](docs/MVP_DESIGN.md) を参照してください。

## 必要環境（Windows）

- Node.js
- Rust stable MSVC toolchain
- Microsoft C++ Build Tools
- Microsoft Edge WebView2
- Git
- GitHub CLI（gh連携を使う場合のみ）

GitHub CLIがない場合、Profile作成画面に次の導入コマンドを表示します。導入後はGitContextを再起動してください。

```powershell
winget install --id GitHub.cli
```

Profile画面の「GitHubに接続」を押すとGitHub公式のブラウザ認証が始まります。ワンタイムコードは認証中だけ画面に表示され、認証ページも自動で開きます。認証情報はProfile専用の `GH_CONFIG_DIR` にGitHub CLI自身が保存し、GitContextはコードやtokenを状態ファイルへ保存しません。

## 開発

```powershell
npm install
npm run test
npm run tauri dev
```

Rust未導入でも、次のコマンドでinteractive previewをブラウザ表示できます。このモードはGit設定へ書き込みません。

```powershell
npm run dev
```

- 英語版: `http://localhost:1420/`
- 日本語版: `http://localhost:1420/ja.html`

各画面の右上にある言語リンクからも切り替えられます。

## 設定反映の原則

- Global/System Git configは変更しない
- 任意shell commandを実行しない
- SSH秘密鍵やGitHub tokenをアプリに保存しない
- 適用前に変更予定を表示する
- 複数設定の途中失敗時は元のローカル値へ戻す
- GitHub公開はコミット済みで変更のないリポジトリだけを対象にし、公開範囲と実行内容を直前に確認する
