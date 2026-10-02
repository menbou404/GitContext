import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { basename, isAbsolute, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const IMAGE_EXTENSIONS = new Set(['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'ico', 'icns']);
const EMAIL_PATTERN = /[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}/gi;

export function parseDenylist(source) {
  return source.split(/\r?\n/).map((line) => line.trim())
    .filter((line) => line && !line.startsWith('#'))
    .map((value, index) => ({ index: index + 1, value }));
}

export function findEmails(text) {
  return [...text.matchAll(EMAIL_PATTERN)]
    .map((match) => match[0])
    .filter((email) => !IMAGE_EXTENSIONS.has(email.slice(email.lastIndexOf('.') + 1).toLowerCase()));
}

export function isAllowedEmail(email, config) {
  const lower = email.toLowerCase();
  if (!lower.includes('@')) return false;
  if (config.allowedEmails.some((allowed) => allowed.toLowerCase() === lower)) return true;
  const domain = lower.slice(lower.lastIndexOf('@') + 1);
  return config.allowedEmailDomains.some((allowed) => {
    const pattern = allowed.toLowerCase();
    return pattern.startsWith('*.')
      ? domain.endsWith(pattern.slice(1)) && domain.length > pattern.length - 1
      : domain === pattern;
  });
}

export function isForbiddenPath(filePath) {
  const name = basename(filePath.replaceAll('\\', '/')).toLowerCase();
  return name === 'state.json' || name.startsWith('state.json.') || name === 'state.lock'
    || /^state-.*\.json$/.test(name) || /\.(pem|key|p12|pfx)$/.test(name)
    || name === 'id_rsa' || name.startsWith('id_ed25519')
    || (name !== '.env.example' && (name === '.env' || name.startsWith('.env.')));
}

export function maskEmail(email) {
  const at = email.indexOf('@');
  if (at < 0) return '***';
  return `${email[0]}***${email.slice(at)}`;
}

export function scanText(text, location, config, denylist = [], emailSeverity = 'error') {
  const findings = [];
  const lower = text.toLowerCase();
  for (const entry of denylist) {
    if (lower.includes(entry.value.toLowerCase())) {
      findings.push({ kind: 'denylist', location, description: `denylist entry #${entry.index}`, severity: 'error' });
    }
  }
  for (const email of findEmails(text)) {
    if (!isAllowedEmail(email, config)) {
      findings.push({ kind: 'email', location, description: `許可されていないメール: ${maskEmail(email)}`, severity: emailSeverity });
    }
  }
  return findings;
}

export function parseUnifiedDiffAddedLines(diff) {
  const additions = [];
  let file = null;
  let newLine = 0;
  let inHunk = false;
  for (const line of diff.split('\n')) {
    const current = line.endsWith('\r') ? line.slice(0, -1) : line;
    if (current.startsWith('diff --git ')) {
      file = null;
      inHunk = false;
    } else if (!inHunk && current.startsWith('+++ ')) {
      const raw = current.slice(4);
      file = raw === '/dev/null' ? null : raw.startsWith('b/') ? raw.slice(2) : raw;
    } else if (current.startsWith('@@ ')) {
      const match = /^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(current);
      if (match) {
        newLine = Number(match[1]);
        inHunk = true;
      }
    } else if (inHunk && file !== null && current.startsWith('+')) {
      additions.push({ file, line: newLine, text: current.slice(1) });
      newLine += 1;
    } else if (inHunk && file !== null && current.startsWith(' ')) {
      newLine += 1;
    }
  }
  return additions;
}

export function matchesGlob(filePath, glob) {
  const path = filePath.replaceAll('\\', '/');
  const pattern = glob.replaceAll('\\', '/');
  let regex = '^';
  for (let i = 0; i < pattern.length; i += 1) {
    const char = pattern[i];
    if (char === '*' && pattern[i + 1] === '*' && pattern[i + 2] === '/') {
      regex += '(?:.*/)?';
      i += 2;
    } else if (char === '*' && pattern[i + 1] === '*') {
      regex += '.*';
      i += 1;
    } else if (char === '*') regex += '[^/]*';
    else if (char === '?') regex += '[^/]';
    else regex += char.replace(/[|\\{}()[\]^$+?.]/g, '\\$&');
  }
  return new RegExp(`${regex}$`).test(path);
}

function git(args, options = {}) {
  return execFileSync('git', ['-c', 'core.quotePath=false', ...args], {
    encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'], ...options,
  });
}

function nulPaths(output) {
  return output.split('\0').filter(Boolean);
}

function readDenylist() {
  const selected = process.env.GITCONTEXT_DENYLIST;
  const gitPath = selected ?? git(['rev-parse', '--git-path', 'info/personal-denylist']).trim();
  const file = isAbsolute(gitPath) ? gitPath : resolve(gitPath);
  if (!existsSync(file)) {
    process.stderr.write('denylistが無いので固有語チェックをスキップ\n');
    return [];
  }
  return parseDenylist(readFileSync(file, 'utf8'));
}

function scanPath(file, config, denylist, prefix = '') {
  const location = `${prefix}${file}`;
  const findings = scanText(file, location, config, denylist);
  if (isForbiddenPath(file)) {
    findings.push({ kind: 'path', location, description: '禁止ファイル名', severity: 'error' });
  }
  return findings;
}

function scanLines(text, location, config, denylist, emailSeverity = 'error') {
  return text.split(/\r?\n/).flatMap((line, index) =>
    scanText(line, `${location}:${index + 1}`, config, denylist, emailSeverity));
}

function scanAddedLines(diff, config, denylist, prefix = '') {
  return parseUnifiedDiffAddedLines(diff).flatMap(({ file, line, text }) => {
    if (config.ignoredPaths.some((glob) => matchesGlob(file, glob))) return [];
    return scanText(text, `${prefix}${file}:${line}`, config, denylist);
  });
}

function scanIdent(ident, label, config, denylist, emailSeverity) {
  const match = /^(.*) <([^<>]*)> \d+ [+-]\d{4}\s*$/.exec(ident.trim());
  if (!match) throw new Error('Git identity could not be parsed');
  return [
    ...scanText(match[1], label, config, denylist),
    ...scanIdentityEmail(match[2], label, config, denylist, emailSeverity),
  ];
}

function scanIdentityEmail(email, location, config, denylist, severity) {
  const findings = scanText(email, location, config, denylist).filter((finding) => finding.kind === 'denylist');
  if (!isAllowedEmail(email, config)) {
    findings.push({ kind: 'email', location, description: `許可されていないメール: ${maskEmail(email)}`, severity });
  }
  return findings;
}

function scanCommit(commit, config, denylist) {
  const metadata = git(['log', '-1', '--format=%an%x00%ae%x00%cn%x00%ce%x00%B', commit]);
  const parts = metadata.split('\0');
  if (parts.length < 5) throw new Error('Commit metadata could not be parsed');
  const [authorName, authorEmail, committerName, committerEmail, ...message] = parts;
  const findings = [];
  findings.push(...scanText(authorName, `${commit} author`, config, denylist));
  findings.push(...scanIdentityEmail(authorEmail, `${commit} author`, config, denylist, 'warning'));
  findings.push(...scanText(committerName, `${commit} committer`, config, denylist));
  findings.push(...scanIdentityEmail(committerEmail, `${commit} committer`, config, denylist, 'warning'));
  findings.push(...scanLines(message.join('\0'), `${commit} message`, config, denylist));
  const paths = nulPaths(git(['diff-tree', '--root', '--first-parent', '-m', '--no-commit-id', '--name-only', '-r', '-z', '--diff-filter=ACMR', commit]));
  for (const file of paths) findings.push(...scanPath(file, config, denylist, `${commit} `));
  const diff = git(['show', '--format=', '--root', '--first-parent', '-m', '--no-color', '--no-ext-diff', '-U0', '--diff-filter=ACMR', commit]);
  findings.push(...scanAddedLines(diff, config, denylist, `${commit} `));
  return findings;
}

function parseArgs(argv) {
  let mode = null;
  let value = null;
  let noDenylist = false;
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--no-denylist') noDenylist = true;
    else if (['--staged', '--tree', '--message', '--range'].includes(arg)) {
      if (mode) throw new Error('Specify exactly one mode');
      mode = arg;
      if (arg === '--message' || arg === '--range') {
        value = argv[++i];
        if (!value) throw new Error('Missing mode argument');
      }
    } else throw new Error('Unknown argument');
  }
  if (!mode) throw new Error('Specify one mode');
  return { mode, value, noDenylist };
}

function safeOutput(value, denylist) {
  let safe = value.replaceAll('\r', ' ').replaceAll('\n', ' ');
  for (const entry of [...denylist].sort((a, b) => b.value.length - a.value.length)) {
    safe = safe.replaceAll(new RegExp(entry.value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'gi'), `[denylist entry #${entry.index}]`);
  }
  return safe.replace(EMAIL_PATTERN, (email) => maskEmail(email));
}

export function runCli(argv = process.argv.slice(2)) {
  let denylist = [];
  try {
    const { mode, value, noDenylist } = parseArgs(argv);
    const config = JSON.parse(readFileSync(resolve('.personal-data-guard.json'), 'utf8'));
    if (!noDenylist) denylist = readDenylist();
    const findings = [];
    if (mode === '--staged') {
      const paths = nulPaths(git(['diff', '--cached', '--name-only', '-z', '--diff-filter=ACMR']));
      for (const file of paths) findings.push(...scanPath(file, config, denylist));
      const diff = git(['diff', '--cached', '--no-color', '--no-ext-diff', '-U0', '--diff-filter=ACMR']);
      findings.push(...scanAddedLines(diff, config, denylist));
      findings.push(...scanIdent(git(['var', 'GIT_AUTHOR_IDENT']), 'author', config, denylist, 'error'));
      findings.push(...scanIdent(git(['var', 'GIT_COMMITTER_IDENT']), 'committer', config, denylist, 'error'));
    } else if (mode === '--message') {
      findings.push(...scanLines(readFileSync(value, 'utf8'), value, config, denylist));
    } else if (mode === '--range') {
      const commits = git(['rev-list', '--reverse', value]).trim().split('\n').filter(Boolean);
      for (const commit of commits) findings.push(...scanCommit(commit, config, denylist));
    } else {
      for (const file of nulPaths(git(['ls-files', '-z']))) {
        findings.push(...scanPath(file, config, denylist));
        if (config.ignoredPaths.some((glob) => matchesGlob(file, glob))) continue;
        const content = readFileSync(file);
        if (!content.includes(0)) findings.push(...scanLines(content.toString('utf8'), file, config, denylist));
      }
    }
    for (const finding of findings) {
      const kind = finding.severity === 'warning' ? `警告:${finding.kind}` : finding.kind;
      process.stdout.write(`${kind} | ${safeOutput(finding.location, denylist)} | ${safeOutput(finding.description, denylist)}\n`);
    }
    const errors = findings.filter((finding) => finding.severity === 'error').length;
    const warnings = findings.length - errors;
    process.stdout.write(`違反 ${errors} 件、警告 ${warnings} 件\n`);
    if (errors) process.stdout.write('意図的な例外は .personal-data-guard.json に許可を追加するか、denylistを見直してください。\n');
    return errors ? 1 : 0;
  } catch (error) {
    process.stderr.write('personal-data-guard: 検査を完了できませんでした。引数、設定ファイル、Git リポジトリを確認してください。\n');
    process.stderr.write(`personal-data-guard: ${safeOutput(String(error?.message ?? error), denylist)}\n`);
    return 1;
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  process.exitCode = runCli();
}
