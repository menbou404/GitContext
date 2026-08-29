# GitContext MVP設計

## プロダクトの中心概念

GitHubアカウントの「現在値」を切り替えるアプリではなく、ローカルの各リポジトリに `Repository Identity / Profile` を割り当てるアプリとする。

ユーザーが行う判断は次の1つだけ。

```text
このリポジトリ → Personal / School / 任意のProfile
```

Profileは次の参照と公開設定をまとめる。

- Git author: `user.name`, `user.email`
- GitHub username（表示・整合性確認用）
- 既存のSSH秘密鍵ファイルへのパス
- 既存のGitHub CLI設定ディレクトリへのパス (`GH_CONFIG_DIR`)

秘密鍵の内容、GitHub token、パスワードはデータモデルに持たない。

## MVP画面

### Repository dashboard

- 左サイドバー: Profile一覧、準備状態、割当リポジトリ数
- 上部: Git / gh / SSH のローカル環境検出結果
- 中央: リポジトリ検索、Profile、適用状態
- 右Inspector: 選択中リポジトリのProfile選択、現在のidentity、適用ボタン

### Profile editor

- Profile名と色
- Git author name / email
- GitHub username（任意）
- `~/.ssh` 内の既存秘密鍵（任意）
- 既存のgh設定ディレクトリ（任意）
- 「参照だけを保存する」ことを常時表示

### Apply review

書き込みの前にキー単位で現在値と予定値を表示する。ユーザーが明示的に確認した場合だけ適用する。

## データモデル

```text
AppData
├─ version
├─ profiles[]
│  ├─ id, label, accent
│  ├─ gitName, gitEmail
│  ├─ githubUsername?
│  ├─ sshKeyPath?
│  └─ ghConfigDir?
└─ repositories[]
   ├─ id, name, canonical path
   ├─ remoteUrl?, branch?
   ├─ profileId?
   └─ lastAppliedAt?
```

アプリ自身の状態はTauriのapp config directoryに `state.json` として保存する。保存時は一時ファイルとバックアップを使い、書き込み途中の破損から復旧できるようにする。

## 安全な設定反映

1. 選択パスをcanonicalizeし、`git rev-parse --show-toplevel` と一致するリポジトリrootだけを受け付ける。
2. フロントエンドから任意のシェル文字列は受け付けない。Rust側で固定した `git` 引数だけを `std::process::Command` に渡す。
3. `git config --local` のみを使用する。Global/System設定には書き込まない。
4. 適用対象は `user.name`, `user.email`, `core.sshCommand` と `gitcontext.*` の追跡メタデータだけに限定する。
5. SSHコマンドはアプリ側で `ssh -i "<canonical path>" -o IdentitiesOnly=yes` を生成する。
6. SSH鍵は `~/.ssh` 内の通常ファイルだけを許可し、`.pub` は拒否する。
7. 複数キーの適用途中でGitが失敗した場合、同じ処理内で元のローカル値へロールバックする。
8. Tauri capabilityはメインwindowのcore標準機能とfile dialogだけに限定する。shell pluginや広いfilesystem権限は付けない。

## GitHub CLIの扱い

`gh auth switch` は同じhostのグローバルなactive accountを書き換えるため、MVPでは自動実行しない。Profileには既存の `GH_CONFIG_DIR` だけを参照として持ち、プレビュー時に次の形で認証状態を確認する。

```text
GH_CONFIG_DIR=<profile directory> gh auth status --active --hostname github.com
```

tokenを表示する `--show-token` は使わない。今後GitContext内から `gh` 操作を追加するときも、リポジトリのworking directoryとProfileの `GH_CONFIG_DIR` をプロセス環境へ渡す方式に統一する。一般の外部terminal全体を暗黙に書き換えることはしない。

## MVPの範囲

実装済み:

- Profileの作成・編集・永続化
- Git repositoryの選択、検証、登録、削除
- Profile割当の差分プレビュー
- repository-local Git identity / SSH commandのtransactionalな適用
- Git / gh / SSHの環境検出
- Profile別gh設定の非秘密な認証状態確認
- ブラウザ用interactive previewとフロントエンドテスト

次の候補:

- 適用履歴とワンクリックrollback
- GitContext経由の `gh repo`, `gh pr`, `gh issue` 操作
- リポジトリrootの一括scan
- remote ownerとProfileのGitHub usernameが異なる場合の警告
- Profile export/import（参照情報のみ）
