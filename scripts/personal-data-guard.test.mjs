import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import {
  findEmails, isAllowedEmail, isForbiddenPath, maskEmail, matchesGlob,
  parseDenylist, parseUnifiedDiffAddedLines, scanText,
} from './personal-data-guard.mjs';

const config = {
  allowedEmailDomains: ['example.com', 'example.org', 'example.net', 'example.ac.jp', '*.example', 'users.noreply.github.com'],
  allowedEmails: ['git@github.com', 'noreply@anthropic.com', 'noreply@github.com'],
  ignoredPaths: ['package-lock.json', '**/*.png'],
};
const unapprovedEmail = 'learner' + '@' + 'school.invalid';

describe('personal data guard helpers', () => {
  it('parses comments, blanks, and trimmed entries and matches without case sensitivity', () => {
    const entries = parseDenylist('  # comment\r\n\r\n  FICTIONAL-CODE  \r\n');
    expect(entries).toEqual([{ index: 1, value: 'FICTIONAL-CODE' }]);
    expect(scanText('fictional-code', 'sample.txt:2', config, entries)).toEqual([{
      kind: 'denylist', location: 'sample.txt:2', description: 'denylist entry #1', severity: 'error',
    }]);
    expect(JSON.stringify(scanText('fictional-code', 'sample.txt:2', config, entries))).not.toContain('FICTIONAL-CODE');
  });

  it('allows configured email addresses and ignores image file names', () => {
    expect(isAllowedEmail('Learner@EXAMPLE.COM', config)).toBe(true);
    expect(isAllowedEmail('sample@sub.example', config)).toBe(true);
    expect(isAllowedEmail('git@github.com', config)).toBe(true);
    expect(isAllowedEmail('example.com', config)).toBe(false);
    expect(isAllowedEmail(unapprovedEmail, config)).toBe(false);
    expect(findEmails('icon@2x.png photo@2x.jpeg ' + unapprovedEmail)).toEqual([unapprovedEmail]);
    expect(scanText(unapprovedEmail, 'sample.txt:1', config)).toHaveLength(1);
  });

  it('masks email local parts', () => {
    expect(maskEmail(unapprovedEmail)).toBe('l***' + '@school.invalid');
  });

  it('rejects sensitive basenames but allows an example environment file', () => {
    for (const path of ['nested/state.json', 'state.json.old', 'state.lock', 'state-backup.json',
      'nested/key.pem', 'key.key', 'key.p12', 'key.pfx', 'id_rsa', 'id_ed25519.pub', '.env', '.env.local']) {
      expect(isForbiddenPath(path), path).toBe(true);
    }
    expect(isForbiddenPath('.env.example')).toBe(false);
    expect(isForbiddenPath('nested/safe.txt')).toBe(false);
  });

  it('extracts added lines and their new-file line numbers from unified diff hunks', () => {
    const diff = [
      'diff --git a/日本語.txt b/日本語.txt',
      '--- a/日本語.txt',
      '+++ b/日本語.txt',
      '@@ -3,1 +3,2 @@',
      '-old',
      '+first',
      '+second',
      '@@ -9,0 +10,2 @@',
      '+third',
      '+++ fourth',
      '',
    ].join('\n');
    expect(parseUnifiedDiffAddedLines(diff)).toEqual([
      { file: '日本語.txt', line: 3, text: 'first' },
      { file: '日本語.txt', line: 4, text: 'second' },
      { file: '日本語.txt', line: 10, text: 'third' },
      { file: '日本語.txt', line: 11, text: '++ fourth' },
    ]);
  });

  it('matches ignored globs in root and nested paths', () => {
    expect(matchesGlob('icon.png', '**/*.png')).toBe(true);
    expect(matchesGlob('src/assets/icon.png', '**/*.png')).toBe(true);
    expect(matchesGlob('src/assets/icon.svg', '**/*.png')).toBe(false);
  });
});

const temporaryDirectories = [];
afterEach(() => {
  for (const directory of temporaryDirectories.splice(0)) rmSync(directory, { recursive: true, force: true });
});

describe('staged Git scan', () => {
  it('finds a forbidden path, an unapproved email, and a denylist entry without revealing the entry', () => {
    const directory = mkdtempSync(join(tmpdir(), 'gitcontext-guard-'));
    temporaryDirectories.push(directory);
    const environment = {
      ...process.env,
      GIT_AUTHOR_NAME: 'Fictional Author',
      GIT_AUTHOR_EMAIL: 'author@example.com',
      GIT_COMMITTER_NAME: 'Fictional Committer',
      GIT_COMMITTER_EMAIL: 'committer@example.com',
      GIT_CONFIG_NOSYSTEM: '1',
      GIT_CONFIG_GLOBAL: join(directory, 'missing-global-config'),
      GITCONTEXT_DENYLIST: join(directory, 'personal-denylist'),
    };
    const git = (...args) => execFileSync('git', [
      '-c', 'user.name=Fictional Author', '-c', 'user.email=author@example.com', ...args,
    ], { cwd: directory, env: environment, stdio: ['pipe', 'pipe', 'pipe'] });
    git('init', '-q');
    writeFileSync(join(directory, '.personal-data-guard.json'), JSON.stringify(config));
    writeFileSync(join(directory, 'personal-denylist'), 'FICTIONAL-SECRET\n');
    writeFileSync(join(directory, 'state.json'), unapprovedEmail + '\nFICTIONAL-SECRET\n');
    git('add', '--', 'state.json');
    const result = spawnSync(process.execPath, [resolve('scripts/personal-data-guard.mjs'), '--staged'], {
      cwd: directory, env: environment, encoding: 'utf8',
    });
    expect(result.status).toBe(1);
    expect(result.stdout).toContain('禁止ファイル名');
    expect(result.stdout).toContain('l***' + '@school.invalid');
    expect(result.stdout).toContain('denylist entry #1');
    expect(`${result.stdout}${result.stderr}`).not.toContain('FICTIONAL-SECRET');
  });
});
