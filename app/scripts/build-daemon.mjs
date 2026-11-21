// Builds the daemon and places it where Tauri expects a sidecar binary:
// src-tauri/binaries/cohors-daemon-<target triple>[.exe].
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const release = process.argv.includes('--release');
const appDir = join(dirname(fileURLToPath(import.meta.url)), '..');
const daemonDir = join(appDir, '..', 'daemon');
const extension = process.platform === 'win32' ? '.exe' : '';

const cargoArgs = ['build', '--manifest-path', join(daemonDir, 'Cargo.toml')];
if (release) cargoArgs.push('--release');
execFileSync('cargo', cargoArgs, { stdio: 'inherit' });

const triple = execFileSync('rustc', ['--print', 'host-tuple'], { encoding: 'utf8' }).trim();
const built = join(daemonDir, 'target', release ? 'release' : 'debug', `cohors-daemon${extension}`);
const sidecarDir = join(appDir, 'src-tauri', 'binaries');
mkdirSync(sidecarDir, { recursive: true });
copyFileSync(built, join(sidecarDir, `cohors-daemon-${triple}${extension}`));
