# GitContext

GitContextは、学校用・趣味用など複数のGitHub作業identityを、リポジトリ単位で割り当てるTauriデスクトップアプリです。

「アカウントを切り替える」のではなく、各repositoryに `Personal` / `School` などのProfileを一度割り当てます。Gitのauthor、SSH identity、GitHub CLI設定への参照を同じProfileとして扱います。

## MVPでできること

- Profileの作成・編集
- ローカルGit repositoryの追加
- repositoryごとのProfile割当
- 適用前の差分レビュー
- `.git/config` へのrepository-local設定反映
- 既存の `~/.ssh` と `gh` 設定を参照（秘密情報は保存しない）

設計と安全境界は [docs/MVP_DESIGN.md](docs/MVP_DESIGN.md) を参照してください。

## 必要環境（Windows）

- Node.js
- Rust stable MSVC toolchain
- Microsoft C++ Build Tools
- Microsoft Edge WebView2
- Git
- GitHub CLI（gh連携を使う場合のみ）

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

## 設定反映の原則

- Global/System Git configは変更しない
- 任意shell commandを実行しない
- SSH秘密鍵やGitHub tokenをアプリに保存しない
- 適用前に変更予定を表示する
- 複数設定の途中失敗時は元のローカル値へ戻す
