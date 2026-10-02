import { execFileSync } from 'node:child_process';
import { chmodSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, isAbsolute, resolve } from 'node:path';

const git = (args) => execFileSync('git', args, { encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'] });

try {
  const root = git(['rev-parse', '--show-toplevel']).trim();
  const gitPath = git(['rev-parse', '--git-path', 'info/personal-denylist']).trim();
  const denylistPath = isAbsolute(gitPath) ? gitPath : resolve(root, gitPath);
  if (!existsSync(denylistPath)) {
    mkdirSync(dirname(denylistPath), { recursive: true });
    writeFileSync(denylistPath,
      '# ローカル専用の固有語を1行に1語ずつ記入してください。\n# このファイルの値はリポジトリに追加しないでください。\n',
      { encoding: 'utf8', flag: 'wx' });
  }
  for (const hook of ['pre-commit', 'commit-msg', 'pre-push']) {
    chmodSync(resolve(root, 'scripts/git-hooks', hook), 0o755);
  }
  git(['config', '--local', 'core.hooksPath', 'scripts/git-hooks']);
  process.stdout.write('Git hooks のパスを設定しました。\n');
} catch {
  process.stderr.write('Git hooks の設定に失敗しました。Git リポジトリと書き込み権限を確認してください。\n');
  process.exitCode = 1;
}
