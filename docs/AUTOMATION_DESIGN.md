# 新しいリポジトリの自動化の設計

## 目的

Git管理されているローカルのプロジェクトについて、AIへの1つの依頼で、GitContextへの登録、プロファイルの適用、commit、GitHubへの公開、ブランチの作成、push、Pull Requestの作成までを、人の確認なしに進められるようにする。

例: 「GitContextのMCPを使って、このプロジェクトをGitHubに上げてPRまで作って」

`git init`はGitContextの範囲外とする（AIクライアントがGitHubに接続せずに実行できるため）。

## 現状の課題

自動承認（[MCP_DESIGN.md](MCP_DESIGN.md)の「リポジトリごとの自動承認」）は、登録済みのリポジトリに設定するもので、新しく登録したリポジトリはすべて無効から始まる。そのため、AIが登録したばかりのリポジトリでは、公開・push・PRが毎回人の確認になる。

また、まだGitHubに上げていないリポジトリにはoriginがないため、`suggest_profile`（originの持ち主とプロファイルのGitHubユーザー名の照合）では、どのプロファイルを使うかが決まらない。

## 全体の考え方

- **プロファイル（GitHubユーザーごとの設定のまとまり）に、新しく登録したリポジトリの自動承認の既定値と、その既定値を使ってよいフォルダを持たせる。**
- **フォルダの規則で、使うプロファイルも決める。**
- **規則で1つに決まり、AIの指定もそれと一致する場合だけ、確認なしで適用し、既定値を引き継ぐ。** それ以外は、プロファイルの割り当てを人が確認する。
- 既定値と規則を変更できるのはGUIだけ。AIからは変更できない。

## プロファイルの設定

プロファイルの`autoApprove`に、新しく登録したリポジトリの既定値を追加する。

| 項目（JSON） | 内容 | 既定 |
|---|---|---|
| `newRepositoryFolders` | 既定値を使ってよいフォルダの一覧（絶対パス） | 空 |
| `newRepository.pushWorkBranch` | 作業ブランチへのpush | false |
| `newRepository.pushDefaultBranch` | 既定ブランチへのpush | false |
| `newRepository.createPullRequest` | Pull Requestの作成 | false |
| `newRepository.mergePullRequest` | Pull Requestのmerge | false |
| `newRepository.publishRepository` | GitHubへの公開 | false |
| `newRepository.publishVisibility` | 公開を自動にする範囲（`private`／`any`） | `private` |

- フォルダが空のプロファイルは、規則にも既定値の引き継ぎにも使わない。
- 古い状態ファイル（項目なし）は既定値で読み込む。

## リポジトリの設定

- リポジトリの`autoApprove`に`publishVisibility`（`private`／`any`、既定`private`）を追加する。`publishRepository`がtrueでも、プレビューの公開範囲が`public`で`publishVisibility`が`private`なら、自動にせず通常の確認に戻す。
- 既定値を引き継いだリポジトリには`autoApproveSource`（プロファイルID、日時）を記録し、GUIの概要タブに「プロファイル『…』の既定値から設定（日時）」と表示する。GUIで自動承認を変更したら`autoApproveSource`は消す。

## フォルダの規則

- リポジトリのパスが、プロファイルの`newRepositoryFolders`のいずれかの中にあるとき、そのプロファイルが規則に当てはまる。
- 比較は正規化したパスの構成要素ごとに行い、Windowsでは大文字小文字を区別しない（`C:\Work`は`C:\Work2`に当てはまらない）。
- 複数のプロファイルが当てはまる場合は、最も深いフォルダを設定したプロファイルを選ぶ。最も深いフォルダが複数のプロファイルで同じ深さなら、規則では決まらない（あいまい）。
- 規則のプロファイルとoriginの持ち主から決まるプロファイルが食い違う場合も、規則では決まらない扱いにする。

## 割り当ての流れ（MCP）

「新しく登録したリポジトリ」とは、プロファイルが一度も割り当てられていない（`profileId`と`lastAppliedAt`がない）リポジトリを指す。既に割り当て済みのリポジトリの割り当て変更は、これまでどおり（確認なし、既定値の引き継ぎなし）とする。

### `suggest_profile`

結果に次を追加する。

- `ruleProfile`: フォルダの規則で決まるプロファイル（なければnull）
- `originCandidates`: originの持ち主に一致するプロファイル（従来の`candidates`）
- `needsConfirmation`: 規則で1つに決まらない場合true
- `reason`: 決まらない理由（規則なし、あいまい、originと食い違い）

ツールの説明に「`needsConfirmation`がtrueのときは、どのプロファイルを使うかを利用者に確認してから`preview_assignment`を呼ぶこと」と書く。

### `preview_assignment`

- `profileId`を省略できるようにする。省略時は規則のプロファイルを使う。規則で決まらない場合は、プレビューは作るが「人の確認が必要」とする。
- 結果に`decision`（`automatic`／`needsConfirmation`）、`inheritDefaults`（既定値を引き継ぐか）、理由を含める。
- `automatic`になる条件: 新しく登録したリポジトリで、規則で1つに決まり、AIの指定がないか規則と同じプロファイル。このとき`inheritDefaults`はtrue。

### `apply_profile`

- `automatic`: 確認なしで適用し、既定値を引き継ぐ。
- `needsConfirmation`: 人の確認を求めてから適用する。確認の方法は、remote操作と同じ順序（elicitation、対応しないクライアントでGitContextが常駐していればGitContextの確認ウィンドウ）。どちらも使えない場合は、適用せず、理由と「利用者にプロファイルを確認するか、GitContextで割り当ててください」という案内を返す。`--trust-client-approval`でも、この確認は省略しない。
- 確認の内容:
  - リポジトリ名、パス
  - プロファイルの選択（一覧から選ぶ。AIが指定したプロファイル、なければoriginから決まるプロファイルを初期値にする）
  - 「このプロファイルの自動承認の既定値も適用する」（リポジトリが選んだプロファイルのフォルダの中にあれば初期値オン、外なら初期値オフ）
  - 承認／拒否
- elicitationではフォームの`enum`でプロファイルを選ばせる（MRTRでも同じ形）。人では不可能な速さ（1秒未満）の応答は拒否する。
- 人が選んだプロファイルが、プレビューのプロファイルと違う場合は、そのプロファイルで照合し直してから適用する。

### 監査ログ

- `apply_profile`の記録に、確認の方法（なし／`elicitation`／`gui`）と、既定値を引き継いだかどうかを示す固定のsummaryを使う。

## 画面

### プロファイル編集

```text
AIによる操作の自動承認
  [ ] このプロファイルでのclone

新しく登録したリポジトリの既定値
  自動化を許可するフォルダ
    C:\Users\…\WebProjects            [削除]
    [フォルダを追加]
  [ ] 作業ブランチへのpush
  [ ] 既定ブランチへのpush       確認なしにmainなどの既定ブランチが変わります
  [ ] Pull Requestの作成
  [ ] Pull Requestのmerge        確認なしにPull Requestがmergeされます
  [ ] GitHubへの公開
        (●) privateのときだけ
        ( ) publicも含む          確認なしに公開リポジトリが作られます
```

- 即時に保存する。危険な項目には、既存と同じく危険の色で影響を常に表示する。
- フォルダはフォルダ選択ダイアログで追加する。ユーザーのホームの外や、ドライブのルートは追加できない。

### リポジトリの概要

- 既存の5項目に、公開の「privateのときだけ／publicも含む」を追加する。
- 既定値から設定された場合は、その旨とプロファイル名、日時を表示する。

### 確認ウィンドウ

GitContextの確認ウィンドウ（[BACKGROUND_DESIGN.md](BACKGROUND_DESIGN.md)）に、プロファイルの割り当ての確認を追加する。上記の「確認の内容」と同じ項目を表示する。

## 安全性

- 既定値とフォルダはGUIからだけ変更できる。MCPに変更するツールは用意しない。
- 既定値を確認なしで引き継ぐのは、フォルダの規則で1つに決まり、AIの指定と一致する場合だけ。AIがフォルダの外のリポジトリや、別のプロファイルを指定した場合は、人の確認なしに既定値は引き継がれない。リポジトリ内の文章（プロンプトインジェクション）でプロファイルを誘導されても、規則と食い違えば人の確認になる。
- 公開の既定は`private`のときだけ自動。publicにする場合は、明示して選ぶ必要がある。
- 既定値を引き継いだ後も、各操作でのプレビューIDの照合、指紋の再照合、mergeの先頭commitの照合は従来どおり行う。

## 段階

| 段階 | 内容 |
|---|---|
| A-1（実装済み） | プロファイルの既定値とフォルダ、リポジトリの`publishVisibility`と`autoApproveSource`、フォルダの規則、`suggest_profile`と`preview_assignment`の拡張、`automatic`の場合の既定値の引き継ぎ、GUIの設定画面。`needsConfirmation`では適用を拒否して理由と案内を返す |
| A-2 | `needsConfirmation`の場合の人の確認（elicitationのプロファイル選択、GitContextの確認ウィンドウ） |

各段階を1つのPRとする。A-1の時点では、`needsConfirmation`の場合は適用せず、理由と案内を返す。

A-1では、規則でプロファイルが決まらず`profileId`も省略されたプレビューは、`needsConfirmation`と理由を返すが、適用可能なプレビューIDは発行しない。プロファイルを明示したプレビューではIDを発行するが、A-2まで適用時に拒否する。
