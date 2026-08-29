# Security Policy

## Supported versions

GitContextはβ段階です。セキュリティ修正は原則として最新のプレリリースまたは最新の安定版に提供します。

| Version | Supported |
| --- | --- |
| Latest release | Yes |
| Older releases | No |

## Reporting a vulnerability

セキュリティ上の問題は公開Issueへ詳細を書かず、GitHubの[Private vulnerability reporting](https://github.com/menbou404/GitContext/security/advisories/new)から報告してください。

報告には、影響範囲、再現手順、想定される攻撃経路を含めてください。GitHub token、SSH秘密鍵、個人情報などの実データは添付しないでください。

受領後7日以内の初回応答を目標とし、内容を確認して修正方針と公開時期を調整します。

## Security boundaries

- GitContextはGitHub tokenやSSH秘密鍵の内容を保存しません。
- ProfileはGitHub CLI設定ディレクトリとSSH鍵ファイルへのパスだけを保持します。
- Git設定の反映対象はrepository-local configに限定します。
- 外部コマンドはRust側で固定・検証した引数だけを使用します。
