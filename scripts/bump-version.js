#!/usr/bin/env node
/**
 * 版本号自增脚本
 * 用法: node scripts/bump-version.js [patch|minor|major]
 * 自动同步 package.json 和 src-tauri/tauri.conf.json
 */
const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..');
const bump = process.argv[2] || 'patch';

if (!['patch', 'minor', 'major'].includes(bump)) {
  console.error('用法: node scripts/bump-version.js [patch|minor|major]');
  process.exit(1);
}

function bumpVersion(v) {
  const [major, minor, patch] = v.split('.').map(Number);
  if (bump === 'major') return `${major + 1}.0.0`;
  if (bump === 'minor') return `${major}.${minor + 1}.0`;
  return `${major}.${minor}.${patch + 1}`;
}

// package.json
const pkgPath = path.join(root, 'package.json');
const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));
const oldV = pkg.version;
const newV = bumpVersion(oldV);
pkg.version = newV;
fs.writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n', 'utf8');

// tauri.conf.json
const tauriPath = path.join(root, 'src-tauri', 'tauri.conf.json');
const tauri = JSON.parse(fs.readFileSync(tauriPath, 'utf8'));
tauri.version = newV;
fs.writeFileSync(tauriPath, JSON.stringify(tauri, null, 2) + '\n', 'utf8');

console.log(`版本自增: ${oldV} → ${newV} (${bump})`);
console.log('已同步: package.json, src-tauri/tauri.conf.json');
