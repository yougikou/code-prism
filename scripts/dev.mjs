import { createHash } from 'node:crypto';
import { createReadStream, existsSync } from 'node:fs';
import { cp, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const root = resolve(scriptDir, '..');
const runtimeDir = join(root, '.codeprism-dev');
const releaseDir = join(root, '.codeprism-release');
const webDir = join(root, 'web');
const lockPath = join(webDir, 'package-lock.json');
const dependencyStamp = join(runtimeDir, 'web-dependencies.sha256');
const configPath = join(runtimeDir, 'codeprism.yaml');
const binaryPath = join(root, 'target', 'debug', process.platform === 'win32' ? 'codeprism.exe' : 'codeprism');

function run(command, args, options = {}) {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(command, args, {
      cwd: options.cwd ?? root,
      env: options.env ?? process.env,
      stdio: 'inherit',
      shell: process.platform === 'win32',
      ...options,
    });
    child.once('error', reject);
    child.once('exit', (code, signal) => {
      if (code === 0) resolvePromise();
      else reject(new Error(`${command} exited with ${signal ?? `code ${code}`}`));
    });
  });
}

async function sha256(path) {
  const hash = createHash('sha256');
  await new Promise((resolvePromise, reject) => {
    const stream = createReadStream(path);
    stream.on('data', chunk => hash.update(chunk));
    stream.once('error', reject);
    stream.once('end', resolvePromise);
  });
  return hash.digest('hex');
}

async function ensureWebDependencies() {
  await mkdir(runtimeDir, { recursive: true });
  const lockHash = await sha256(lockPath);
  let previousHash = '';
  try {
    previousHash = (await readFile(dependencyStamp, 'utf8')).trim();
  } catch {
    // First development run, or a removed runtime directory.
  }
  if (previousHash === lockHash && existsSync(join(webDir, 'node_modules'))) return;

  console.log('Installing locked web dependencies (npm ci)...');
  await run('npm', ['ci'], { cwd: webDir });
  await writeFile(dependencyStamp, `${lockHash}\n`, 'utf8');
}

async function ensureDevelopmentWorkspace() {
  await mkdir(runtimeDir, { recursive: true });
  if (existsSync(configPath)) return;

  console.log(`Creating isolated development configuration: ${configPath}`);
  await run(binaryPath, ['--config', configPath, 'init'], { cwd: runtimeDir });
}

async function startDevelopment() {
  await ensureWebDependencies();
  await run('cargo', ['build', '--locked'], { cwd: root });
  await ensureDevelopmentWorkspace();

  const config = await readFile(configPath, 'utf8');
  const databaseLine = config.match(/^database_url:\s*["']?([^\s"']+)/m)?.[1];
  if (!databaseLine || databaseLine !== 'sqlite:codeprism.db') {
    throw new Error(
      `Development database must be sqlite:codeprism.db in ${runtimeDir}. Update the isolated config to continue.`,
    );
  }

  const vite = join(webDir, 'node_modules', 'vite', 'bin', 'vite.js');
  if (!existsSync(vite)) {
    throw new Error('Vite was not found after npm ci. Check web/package-lock.json and try again.');
  }

  const env = {
    ...process.env,
    CODEPRISM_ANALYZERS_DIR: join(root, 'custom_analyzers'),
  };
  const children = [];
  let stopping = false;

  const stopChildren = async signal => {
    if (stopping) return;
    stopping = true;
    for (const child of children) {
      if (child.exitCode !== null || child.signalCode !== null) continue;
      if (process.platform === 'win32') {
        await new Promise(resolvePromise => {
          const killer = spawn('taskkill', ['/pid', String(child.pid), '/t', '/f'], { stdio: 'ignore' });
          killer.once('exit', resolvePromise);
          killer.once('error', resolvePromise);
        });
      } else {
        try { process.kill(-child.pid, signal); } catch { child.kill(signal); }
      }
    }
  };

  const spawnService = (command, args, cwd) => {
    const child = spawn(command, args, {
      cwd,
      env,
      stdio: 'inherit',
      shell: process.platform === 'win32',
      detached: process.platform !== 'win32',
    });
    children.push(child);
    child.once('error', error => {
      console.error(`Failed to start ${command}: ${error.message}`);
      void stopChildren('SIGTERM').finally(() => { process.exitCode = 1; });
    });
    child.once('exit', (code, signal) => {
      if (!stopping && (code !== 0 || signal)) {
        console.error(`${command} stopped (${signal ?? `code ${code}`}).`);
        void stopChildren('SIGTERM').finally(() => { process.exitCode = code ?? 1; });
      }
    });
    return child;
  };

  process.once('SIGINT', () => { void stopChildren('SIGINT'); });
  process.once('SIGTERM', () => { void stopChildren('SIGTERM'); });

  console.log(`Development data: ${runtimeDir}`);
  console.log('Dashboard: http://127.0.0.1:5173');
  spawnService(binaryPath, ['--config', configPath, 'serve'], runtimeDir);
  spawnService(process.execPath, [vite, '--host', '127.0.0.1', '--strictPort'], webDir);

  await new Promise(resolvePromise => {
    const poll = setInterval(() => {
      if (stopping || children.every(child => child.exitCode !== null || child.signalCode !== null)) {
        clearInterval(poll);
        resolvePromise();
      }
    }, 200);
  });
  await stopChildren('SIGTERM');
}

async function installWeb() {
  await run('npm', ['ci'], { cwd: webDir });
}

async function ensureBuiltAssets() {
  const indexPath = join(webDir, 'dist', 'index.html');
  if (!existsSync(indexPath)) {
    throw new Error(`Frontend assets are missing or empty: ${indexPath}`);
  }
  const html = await readFile(indexPath, 'utf8');
  if (html.trim().length === 0) throw new Error(`Frontend assets are missing or empty: ${indexPath}`);
  for (const quote of ['"', "'"]) {
    for (const tail of html.split(`${quote}/assets/`).slice(1)) {
      const asset = tail.split(quote)[0];
      if (!existsSync(join(webDir, 'dist', 'assets', asset))) throw new Error(`Frontend entry point references missing asset: ${asset}`);
    }
  }
}

async function cargoBuild({ release = false, target } = {}) {
  const args = ['build', '--locked'];
  if (release) args.push('--release');
  if (target) args.push('--target', target);
  await run('cargo', args, {
    cwd: root,
    env: { ...process.env, CODEPRISM_SKIP_WEB_BUILD: '1' },
  });
}

function executablePath(target, release = true) {
  const targetDir = target ? join(root, 'target', target) : join(root, 'target');
  return join(targetDir, release ? 'release' : 'debug', process.platform === 'win32' ? 'codeprism.exe' : 'codeprism');
}

async function runChecks() {
  await verifyVersion();
  await installWeb();
  await run('npm', ['run', 'lint'], { cwd: webDir });
  await run('npm', ['test'], { cwd: webDir });
  await run('npm', ['run', 'build'], { cwd: webDir });
  await ensureBuiltAssets();
  await run('cargo', ['fmt', '--all', '--', '--check'], { cwd: root });
  await run('cargo', ['clippy', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings'], { cwd: root, env: { ...process.env, CODEPRISM_SKIP_WEB_BUILD: '1' } });
  await run('cargo', ['test', '--workspace', '--locked'], { cwd: root, env: { ...process.env, CODEPRISM_SKIP_WEB_BUILD: '1' } });
  await run('cargo', ['run', '--locked', '--', 'test-analyzers'], { cwd: root, env: { ...process.env, CODEPRISM_ANALYZERS_DIR: join(root, 'custom_analyzers'), CODEPRISM_SKIP_WEB_BUILD: '1' } });
}

async function verifyPayload(payloadDir, binary, executionCwd = payloadDir) {
  const required = [
    'custom_analyzers/camel_java_production_metrics.py',
    'custom_analyzers/camel_java_test_metrics.py',
    'custom_analyzers/camel_java_production_project_metrics.py',
    'custom_analyzers/camel_java_test_project_metrics.py',
    'custom_analyzers/camel_support/__init__.py',
    'custom_analyzers/camel_support/file_metrics.py',
    'custom_analyzers/camel_support/model.py',
  ];
  const missing = required.filter(path => !existsSync(join(payloadDir, path)));
  if (missing.length) throw new Error(`Release package missing files:\n${missing.map(path => `  - ${path}`).join('\n')}`);
  const inspect = async dir => {
    for (const entry of await readdir(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name);
      if (entry.name === '__pycache__' || entry.name.endsWith('.pyc') || entry.name.endsWith('.pyo')) throw new Error(`Release package contains Python bytecode: ${path}`);
      if (entry.isDirectory()) await inspect(path);
    }
  };
  await inspect(join(payloadDir, 'custom_analyzers'));
  const python = process.platform === 'win32' ? 'python' : 'python3';
  for (const name of required.filter(path => path.endsWith('.py') && !path.includes('/camel_support/'))) {
    await run(python, [join(payloadDir, name), 'test'], { cwd: payloadDir, env: { ...process.env, PYTHONDONTWRITEBYTECODE: '1' } });
  }
  const reportedVersion = await runCapture(binary, ['--version']);
  const expectedVersion = JSON.parse(await readFile(join(webDir, 'package.json'), 'utf8')).version;
  if (!reportedVersion.includes(expectedVersion)) throw new Error(`Binary version '${reportedVersion}' does not match package version ${expectedVersion}`);
  const config = join(payloadDir, 'codeprism.release-test.yaml');
  await run(binary, ['init-config', config], { cwd: executionCwd });
  await run(binary, ['--config', config, 'check-config'], { cwd: executionCwd });
  const packagedEnv = { ...process.env };
  delete packagedEnv.CODEPRISM_ANALYZERS_DIR;
  delete packagedEnv.CODEPRISM_CONFIG_DIR;
  await run(binary, ['test-analyzers'], { cwd: executionCwd, env: packagedEnv });
  await rm(config, { force: true });
}

async function packageBuild({ target, prebuilt = false } = {}) {
  await verifyVersion();
  if (!prebuilt) {
    await installWeb();
    await run('npm', ['run', 'build'], { cwd: webDir });
  }
  await ensureBuiltAssets();
  if (!prebuilt) await cargoBuild({ release: true, target });
  const binary = executablePath(target);
  if (!existsSync(binary) || (await readFile(binary)).length === 0) throw new Error(`Release binary missing or empty: ${binary}`);
  const actualTarget = target ?? (await runCapture('rustc', ['-vV'])).match(/^host: (.+)$/m)?.[1];
  if (!actualTarget) throw new Error('Unable to determine Rust target triple');
  const stage = join(releaseDir, actualTarget);
  await rm(stage, { recursive: true, force: true });
  await mkdir(stage, { recursive: true });
  const windowsTarget = actualTarget.includes('windows');
  const binaryName = windowsTarget ? 'codeprism.exe' : 'codeprism';
  await cp(binary, join(stage, binaryName));
  await cp(join(root, 'custom_analyzers'), join(stage, 'custom_analyzers'), {
    recursive: true,
    filter: source => !source.includes('__pycache__') && !source.endsWith('.pyc') && !source.endsWith('.pyo'),
  });
  await cp(join(root, 'LICENSE'), join(stage, 'LICENSE'));
  const scriptFiles = windowsTarget
    ? ['codeprism-scan.ps1', 'codeprism-scan-list.ps1', 'codeprism-repo-list.ps1', 'codeprism-repo-clone.ps1']
    : ['codeprism-scan', 'codeprism-scan-list', 'codeprism-repo-list', 'codeprism-repo-clone'];
  await mkdir(join(stage, 'scripts', 'lib'), { recursive: true });
  for (const file of scriptFiles) await cp(join(root, 'scripts', file), join(stage, 'scripts', file));
  if (!windowsTarget) await cp(join(root, 'scripts', 'lib', 'codeprism-common.sh'), join(stage, 'scripts', 'lib', 'codeprism-common.sh'));
  await cp(join(root, 'scripts', 'README.md'), join(stage, 'scripts', 'README.md'));
  const version = JSON.parse(await readFile(join(webDir, 'package.json'), 'utf8')).version;
  const commit = await runCapture('git', ['rev-parse', 'HEAD']);
  const rustc = await runCapture('rustc', ['--version']);
  const npm = await runCapture(process.platform === 'win32' ? 'npm.cmd' : 'npm', ['--version']);
  await writeFile(join(stage, 'build-info.json'), JSON.stringify({ version, commit, target: actualTarget, node: process.version, npm, rustc }, null, 2) + '\n');
  if ((process.platform === 'win32') === windowsTarget) await verifyPayload(stage, join(stage, binaryName));
  const archiveName = `codeprism-${actualTarget}.${windowsTarget ? 'zip' : 'tar.gz'}`;
  const archive = join(releaseDir, archiveName);
  if (windowsTarget) {
    if (process.platform !== 'win32') throw new Error('Windows packages must be built on Windows to validate the target binary');
    await run('powershell', ['-NoProfile', '-Command', 'Compress-Archive -Path $args[0] -DestinationPath $args[1] -Force', `${stage}\\*`, archive], { cwd: root });
  } else {
    await run('tar', ['-czf', archive, '-C', stage, '.'], { cwd: root });
  }
  const extracted = await mkdtemp(join(tmpdir(), 'codeprism-package-'));
  try {
    await run('tar', ['-xf', archive, '-C', extracted], { cwd: root });
    const extractedBinary = join(extracted, binaryName);
    if (!existsSync(extractedBinary)) throw new Error('Release archive extraction did not contain the executable');
    await verifyPayload(extracted, extractedBinary, root);
  } finally {
    await rm(extracted, { recursive: true, force: true });
  }
  const digest = createHash('sha256').update(await readFile(archive)).digest('hex');
  await writeFile(join(releaseDir, 'SHA256SUMS'), `${digest}  ${archiveName}\n`);
  console.log(`Package ready: ${archive}`);
}

async function updateVersion(version) {
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version ?? '')) throw new Error('Version must use x.y.z or x.y.z-prerelease format');
  const rootPackagePath = join(root, 'package.json');
  const rootPackage = JSON.parse(await readFile(rootPackagePath, 'utf8'));
  rootPackage.version = version;
  await writeFile(rootPackagePath, `${JSON.stringify(rootPackage, null, 2)}\n`);
  const webPackagePath = join(webDir, 'package.json');
  const webPackage = JSON.parse(await readFile(webPackagePath, 'utf8'));
  webPackage.version = version;
  await writeFile(webPackagePath, `${JSON.stringify(webPackage, null, 2)}\n`);
  const lockPath = join(webDir, 'package-lock.json');
  const lock = JSON.parse(await readFile(lockPath, 'utf8'));
  lock.version = version;
  if (lock.packages?.['']) lock.packages[''].version = version;
  await writeFile(lockPath, `${JSON.stringify(lock, null, 2)}\n`);
  const cargoPath = join(root, 'Cargo.toml');
  const cargo = await readFile(cargoPath, 'utf8');
  const updated = cargo.replace(/(\[workspace\.package\][\s\S]*?^version\s*=\s*")[^"]+("\s*$)/m, `$1${version}$2`);
  if (updated === cargo) throw new Error('Could not find [workspace.package].version in Cargo.toml');
  await writeFile(cargoPath, updated);
  await run('cargo', ['check', '--workspace'], { cwd: root, env: { ...process.env, CODEPRISM_SKIP_WEB_BUILD: '1' } });
  await verifyVersion();
}

async function verifyVersion() {
  const webVersion = JSON.parse(await readFile(join(webDir, 'package.json'), 'utf8')).version;
  const lock = JSON.parse(await readFile(join(webDir, 'package-lock.json'), 'utf8'));
  const rootVersion = JSON.parse(await readFile(join(root, 'package.json'), 'utf8')).version;
  const cargo = await readFile(join(root, 'Cargo.toml'), 'utf8');
  const cargoVersion = cargo.match(/\[workspace\.package\][\s\S]*?^version\s*=\s*"([^"]+)"/m)?.[1];
  const packageLockVersion = lock.packages?.['']?.version ?? lock.version;
  if (!cargoVersion || new Set([cargoVersion, webVersion, packageLockVersion, rootVersion]).size !== 1) {
    throw new Error(`Version mismatch: Cargo=${cargoVersion}, web=${webVersion}, web lock=${packageLockVersion}, root=${rootVersion}`);
  }
  const tag = process.env.CODEPRISM_RELEASE_TAG;
  if (tag && tag.replace(/^v/, '') !== cargoVersion) throw new Error(`Release tag ${tag} does not match project version ${cargoVersion}`);
  console.log(`Version ${cargoVersion} is consistent across Cargo and npm`);
}

async function runCapture(command, args) {
  const child = spawn(command, args, { cwd: root, stdio: ['ignore', 'pipe', 'inherit'] });
  let output = '';
  child.stdout.on('data', chunk => { output += chunk; });
  const code = await new Promise((resolvePromise, reject) => {
    child.once('error', reject);
    child.once('exit', resolvePromise);
  });
  if (code !== 0) throw new Error(`${command} exited with code ${code}`);
  return output.trim();
}

const [command, ...args] = process.argv.slice(2);
const tasks = {
  dev: startDevelopment,
  check: runChecks,
  build: async () => {
    await verifyVersion();
    await installWeb();
    await run('npm', ['run', 'build'], { cwd: webDir });
    await ensureBuiltAssets();
    await cargoBuild({ release: true, target: args[0] === '--target' ? args[1] : undefined });
  },
  package: () => packageBuild({
    target: args.includes('--target') ? args[args.indexOf('--target') + 1] : undefined,
    prebuilt: args.includes('--prebuilt'),
  }),
  version: () => updateVersion(args[0]),
  'version-check': verifyVersion,
  'verify-payload': async () => {
    const payload = resolve(process.cwd(), args[0] ?? '.');
    const binary = join(payload, process.platform === 'win32' ? 'codeprism.exe' : 'codeprism');
    await verifyPayload(payload, binary);
  },
};
if (!tasks[command]) {
  console.error('Usage: npm run <dev|check|build|package>');
  process.exitCode = 2;
} else {
  tasks[command]().catch(error => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
