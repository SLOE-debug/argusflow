import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const workspaceRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const manifestPath = path.join(workspaceRoot, 'scripts', 'refactor-layout.json');
const tsconfigPath = path.join(workspaceRoot, 'tsconfig.app.json');
const applyChanges = process.argv.includes('--apply');
const dryRun = process.argv.includes('--dry-run') || !applyChanges;

if (applyChanges && process.argv.includes('--dry-run')) {
  throw new Error('Choose either --dry-run or --apply, not both.');
}

const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
const moves = manifest.moves.map(({ from, to }) => ({
  from: resolveWorkspacePath(from),
  to: resolveWorkspacePath(to),
}));
const sourceToTarget = new Map(moves.map(({ from, to }) => [pathKey(from), to]));

const pendingMoves = validateManifest(moves);
readTsConfig();
const sourceFiles = collectProjectSourceFiles();
const editsByFile = new Map();
let importEditCount = 0;

for (const sourceFile of sourceFiles) {
  const originalPath = path.resolve(sourceFile);
  const relocatedPath = relocatedPathFor(originalPath);
  const edits = collectSpecifierEdits(originalPath, relocatedPath);
  if (edits.length > 0) {
    editsByFile.set(originalPath, edits);
    importEditCount += edits.length;
  }
}

console.log(`${applyChanges ? 'Applying' : 'Dry-run'} ${pendingMoves.length} pending file moves (${moves.length} manifest entries).`);
for (const { from, to } of pendingMoves) {
  console.log(`  ${displayPath(from)} -> ${displayPath(to)}`);
}
console.log(`Planned ${importEditCount} relative specifier updates.`);

if (dryRun) process.exit(0);

for (const { from, to } of pendingMoves) {
  fs.mkdirSync(path.dirname(to), { recursive: true });
  fs.renameSync(from, to);
}

for (const sourceFile of sourceFiles) {
  const originalPath = path.resolve(sourceFile);
  const targetPath = relocatedPathFor(originalPath);
  const edits = editsByFile.get(originalPath) ?? [];
  const originalTextPath = fs.existsSync(originalPath) ? originalPath : targetPath;
  const originalText = fs.readFileSync(originalTextPath, 'utf8');
  const updatedText = applyTextEdits(originalText, edits);
  if (updatedText !== originalText) fs.writeFileSync(targetPath, updatedText, 'utf8');
}

validateProjectAfterMove();
console.log('Layout migration completed and relative references were validated.');

function resolveWorkspacePath(relativePath) {
  return path.resolve(workspaceRoot, relativePath);
}

function pathKey(filePath) {
  const normalized = path.normalize(path.resolve(filePath));
  return process.platform === 'win32' ? normalized.toLowerCase() : normalized;
}

function displayPath(filePath) {
  return path.relative(workspaceRoot, filePath).split(path.sep).join('/');
}

function validateManifest(entries) {
  const sources = new Set();
  const targets = new Set();
  const pendingEntries = [];
  for (const { from, to } of entries) {
    const sourceKey = pathKey(from);
    const targetKey = pathKey(to);
    if (sources.has(sourceKey)) throw new Error(`Duplicate move source: ${displayPath(from)}`);
    if (targets.has(targetKey)) throw new Error(`Duplicate move target: ${displayPath(to)}`);
    const sourceExists = fs.existsSync(from);
    const targetExists = fs.existsSync(to);
    if (!sourceExists && !targetExists) {
      throw new Error(`Move source and target do not exist: ${displayPath(from)} -> ${displayPath(to)}`);
    }
    if (sourceExists && targetExists) {
      throw new Error(`Move source and target both exist: ${displayPath(from)} -> ${displayPath(to)}`);
    }
    if (sourceExists) pendingEntries.push({ from, to });
    sources.add(sourceKey);
    targets.add(targetKey);
  }
  return pendingEntries;
}

/**
 * 读取项目配置作为迁移前置检查。TypeScript 7 的 npm 入口只暴露原生/LSP API，
 * 因而这里不引入额外解析器；下面的词法扫描只处理真实语法 token，并不会用
 * 正则在注释或字符串内容中猜测 import。
 */
function readTsConfig() {
  const config = JSON.parse(fs.readFileSync(tsconfigPath, 'utf8'));
  if (!config.compilerOptions || !Array.isArray(config.include)) {
    throw new Error(`Invalid TypeScript project config: ${displayPath(tsconfigPath)}`);
  }
  return config;
}

function collectProjectSourceFiles() {
  const sourceRoot = path.join(workspaceRoot, 'src');
  const files = [];
  walk(sourceRoot);
  return files.sort();

  function walk(directory) {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      const entryPath = path.join(directory, entry.name);
      if (entry.isDirectory()) walk(entryPath);
      else if (/\.tsx?$/.test(entry.name)) files.push(entryPath);
    }
  }
}

function relocatedPathFor(filePath) {
  return sourceToTarget.get(pathKey(filePath)) ?? filePath;
}

function collectSpecifierEdits(originalImporterPath, relocatedImporterPath) {
  const text = fs.readFileSync(originalImporterPath, 'utf8');
  const tokens = tokenize(text);
  const edits = [];

  for (let index = 0; index < tokens.length; index += 1) {
    const token = tokens[index];
    if (token.value === 'import') {
      if (tokens[index + 1]?.value === '.') continue;
      if (tokens[index + 1]?.value === '(') {
        addTokenEdit(tokens[index + 2], false);
      } else if (tokens[index + 1]?.value === 'type' && tokens[index + 2]?.value === '(') {
        addTokenEdit(tokens[index + 3], false);
      } else if (tokens[index + 1]?.kind === 'string') {
        addTokenEdit(tokens[index + 1], false);
      } else {
        addSpecifierAfterFrom(index + 1, false);
      }
    } else if (token.value === 'export') {
      addSpecifierAfterFrom(index + 1, false);
    } else if (
      token.value === 'new'
      && tokens[index + 1]?.value === 'URL'
      && tokens[index + 2]?.value === '('
    ) {
      addTokenEdit(tokens[index + 3], true);
    } else if (
      (token.value === 'vi' || token.value === 'jest')
      && tokens[index + 1]?.value === '.'
      && tokens[index + 2]?.value === 'mock'
    ) {
      addTokenEdit(tokens[index + 3], false);
    }
  }

  return edits;

  function addSpecifierAfterFrom(startIndex, preserveExtension) {
    for (let index = startIndex; index < tokens.length; index += 1) {
      if (tokens[index].value === ';') return;
      if (tokens[index].value === 'from') {
        addTokenEdit(tokens[index + 1], preserveExtension);
        return;
      }
    }
  }

  function addTokenEdit(token, preserveExtension) {
    if (!token || token.kind !== 'string' || !token.value.startsWith('.')) return;
    const targetPath = resolveReferencedTarget(
      token.value,
      originalImporterPath,
      preserveExtension,
    );
    if (!targetPath) return;
    const updatedSpecifier = formatRelativeSpecifier(
      relocatedImporterPath,
      targetPath,
      preserveExtension,
    );
    if (updatedSpecifier === token.value) return;
    edits.push({
      start: token.start,
      end: token.end,
      replacement: quoteString(updatedSpecifier, token.quote),
    });
  }
}

/** 将注释、字符串和标识符分开，保证只在 import/export 语法位置取路径。 */
function tokenize(text) {
  const tokens = [];
  let index = 0;
  while (index < text.length) {
    const character = text[index];
    if (/\s/.test(character)) {
      index += 1;
      continue;
    }
    if (character === '/' && text[index + 1] === '/') {
      index = skipLineComment(text, index + 2);
      continue;
    }
    if (character === '/' && text[index + 1] === '*') {
      index = skipBlockComment(text, index + 2);
      continue;
    }
    if (character === "'" || character === '"') {
      const quote = character;
      const start = index;
      index += 1;
      while (index < text.length) {
        if (text[index] === '\\') {
          index += 2;
          continue;
        }
        if (text[index] === quote) {
          index += 1;
          break;
        }
        index += 1;
      }
      const rawValue = text.slice(start + 1, index - 1);
      tokens.push({
        kind: 'string',
        value: rawValue.replaceAll(`\\${quote}`, quote).replaceAll('\\\\', '\\'),
        quote,
        start,
        end: index,
      });
      continue;
    }
    if (/[A-Za-z_$]/.test(character)) {
      const start = index;
      index += 1;
      while (index < text.length && /[A-Za-z0-9_$]/.test(text[index])) index += 1;
      tokens.push({ kind: 'word', value: text.slice(start, index), start, end: index });
      continue;
    }
    tokens.push({ kind: 'punctuation', value: character, start: index, end: index + 1 });
    index += 1;
  }
  return tokens;
}

function skipLineComment(text, index) {
  const lineEnd = text.indexOf('\n', index);
  return lineEnd === -1 ? text.length : lineEnd + 1;
}

function skipBlockComment(text, index) {
  const blockEnd = text.indexOf('*/', index);
  return blockEnd === -1 ? text.length : blockEnd + 2;
}

function resolveReferencedTarget(specifier, importerPath, preserveExtension) {
  const resolvedPath = resolveModulePath(specifier, importerPath);
  if (!resolvedPath) return null;
  if (preserveExtension && !sourceToTarget.has(pathKey(resolvedPath))) return null;
  return relocatedPathFor(resolvedPath);
}

function resolveModulePath(specifier, importerPath) {
  const basePath = path.resolve(path.dirname(importerPath), specifier);
  const candidates = [basePath];
  const extension = path.extname(basePath).toLowerCase();
  if (extension === '.js' || extension === '.jsx') {
    candidates.push(
      `${basePath.slice(0, -extension.length)}.ts`,
      `${basePath.slice(0, -extension.length)}.tsx`,
      `${basePath.slice(0, -extension.length)}.d.ts`,
    );
  } else if (!extension) {
    candidates.push(`${basePath}.ts`, `${basePath}.tsx`, `${basePath}.d.ts`, `${basePath}.js`);
  }
  for (const candidate of candidates) {
    if (fs.existsSync(candidate) && fs.statSync(candidate).isFile()) return candidate;
  }
  if (fs.existsSync(basePath) && fs.statSync(basePath).isDirectory()) {
    for (const extensionName of ['.ts', '.tsx', '.d.ts', '.js']) {
      const indexPath = path.join(basePath, `index${extensionName}`);
      if (fs.existsSync(indexPath)) return indexPath;
    }
  }
  return null;
}

function formatRelativeSpecifier(importerPath, targetPath, preserveExtension) {
  let relativePath = path.relative(path.dirname(importerPath), targetPath).split(path.sep).join('/');
  if (!preserveExtension) {
    relativePath = relativePath.replace(/\.(?:d\.)?(?:tsx?|jsx?)$/, '');
    if (relativePath.endsWith('/index')) relativePath = relativePath.slice(0, -'/index'.length);
  }
  if (!relativePath) return '.';
  return relativePath.startsWith('.') ? relativePath : `./${relativePath}`;
}

function quoteString(value, quote) {
  const escaped = value.replaceAll('\\', '\\\\').replaceAll(quote, `\\${quote}`);
  return `${quote}${escaped}${quote}`;
}

function applyTextEdits(text, edits) {
  return edits
    .sort((left, right) => right.start - left.start)
    .reduce((currentText, edit) => (
      `${currentText.slice(0, edit.start)}${edit.replacement}${currentText.slice(edit.end)}`
    ), text);
}

function validateProjectAfterMove() {
  const failures = [];
  for (const sourceFile of collectProjectSourceFiles()) {
    const text = fs.readFileSync(sourceFile, 'utf8');
    const tokens = tokenize(text);
    for (let index = 0; index < tokens.length; index += 1) {
      const token = tokens[index];
      let specifierToken = null;
      let preserveExtension = false;
      if (token.value === 'import') {
        if (tokens[index + 1]?.value === '(') specifierToken = tokens[index + 2];
        else if (tokens[index + 1]?.value === 'type' && tokens[index + 2]?.value === '(') {
          specifierToken = tokens[index + 3];
        } else if (tokens[index + 1]?.kind === 'string') specifierToken = tokens[index + 1];
        else specifierToken = findSpecifierAfterFrom(tokens, index + 1);
      } else if (token.value === 'export') {
        specifierToken = findSpecifierAfterFrom(tokens, index + 1);
      } else if (
        token.value === 'new'
        && tokens[index + 1]?.value === 'URL'
        && tokens[index + 2]?.value === '('
      ) {
        specifierToken = tokens[index + 3];
        preserveExtension = true;
      } else if (
        (token.value === 'vi' || token.value === 'jest')
        && tokens[index + 1]?.value === '.'
        && tokens[index + 2]?.value === 'mock'
      ) {
        specifierToken = tokens[index + 3];
      }
      if (!specifierToken || specifierToken.kind !== 'string' || !specifierToken.value.startsWith('.')) continue;
      const resolvedPath = resolveModulePath(specifierToken.value, sourceFile);
      if (!resolvedPath) failures.push(`${displayPath(sourceFile)} -> ${specifierToken.value}`);
      else if (preserveExtension && !fs.existsSync(resolvedPath)) {
        failures.push(`${displayPath(sourceFile)} -> ${specifierToken.value}`);
      }
    }
  }
  if (failures.length > 0) {
    throw new Error(`Unresolved relative references after migration:\n${failures.join('\n')}`);
  }
}

function findSpecifierAfterFrom(tokens, startIndex) {
  for (let index = startIndex; index < tokens.length; index += 1) {
    if (tokens[index].value === ';') return null;
    if (tokens[index].value === 'from') return tokens[index + 1];
  }
  return null;
}
