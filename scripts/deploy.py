#!/usr/bin/env python3
"""BunMail RS 目录级安装、离线发布、更新和回退。Python 3.11+。"""
import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
import gzip
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import tomllib

FILES = ('compose.yml', 'compose.smtp.yml', 'deploy.py', 'setup.py', 'reset-password.py', '.env.example', 'LICENSE', 'THIRD_PARTY_NOTICES.md')

def run(*args, **kwargs):
    return subprocess.run(args, check=True, **kwargs)

def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

def write_json(path, data):
    temporary = path.with_suffix('.tmp')
    with temporary.open('w') as stream:
        json.dump(data, stream, indent=2)
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, path)

def version(value):
    if not re.fullmatch(r'(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)', value):
        raise ValueError('版本号必须为 major.minor.patch')
    return tuple(map(int, value.split('.')))

def package(number, output):
    version(number)
    root = Path(__file__).resolve().parents[1]
    if tomllib.loads((root / 'Cargo.toml').read_text())['package']['version'] != number:
        raise ValueError('版本号必须与 Cargo.toml 一致')
    output.mkdir(parents=True, exist_ok=True)
    destination = output / number
    if destination.exists():
        raise ValueError('此版本安装包已存在')
    image = f'bunmail-rs:{number}'
    if subprocess.run(['docker', 'image', 'inspect', image], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0:
        raise ValueError('此版本镜像已存在，不能覆盖不可变发布版本')
    schema_hash = hashlib.sha256()
    for path in sorted((root / 'migrations').glob('*.sql')):
        schema_hash.update(path.name.encode())
        schema_hash.update(path.read_bytes())
    schema = schema_hash.hexdigest()
    run('docker', 'build', '--label', f'io.bunmail.schema={schema}', '--label', f'org.opencontainers.image.version={number}', '-t', image, str(root))
    info = json.loads(run('docker', 'image', 'inspect', image, capture_output=True, text=True).stdout)[0]
    with tempfile.TemporaryDirectory(prefix='.package-', dir=output) as temporary:
        directory = Path(temporary)
        sources = ('docker-compose.yml', 'docker-compose.smtp.yml', 'scripts/deploy.py', 'scripts/setup.py', 'scripts/reset-password.py', '.env.example', 'LICENSE', 'THIRD_PARTY_NOTICES.md')
        for name, source in zip(FILES, sources):
            shutil.copy2(root / source, directory / name)
        with gzip.open(directory / 'image.tar.gz', 'wb') as stream:
            process = subprocess.Popen(['docker', 'save', image], stdout=subprocess.PIPE)
            try:
                shutil.copyfileobj(process.stdout, stream)
            finally:
                process.stdout.close()
                code = process.wait()
            if code:
                raise ValueError('镜像导出失败')
        manifest = {'format': 1, 'version': number, 'schema': schema, 'image': info['Id'], 'platform': f"{info['Os']}/{info['Architecture']}", 'files': {name: digest(directory / name) for name in (*FILES, 'image.tar.gz')}}
        write_json(directory / 'release.json', manifest)
        os.rename(directory, destination)
    print(f'安装包已生成：{destination}')

def read_package(directory):
    data = json.loads((directory / 'release.json').read_text())
    version(data['version'])
    if data['format'] != 1 or not re.fullmatch(r'sha256:[a-f0-9]{64}', data['image']) or not re.fullmatch(r'[a-f0-9]{64}', data['schema']):
        raise ValueError('无效的发布清单')
    if set(data['files']) != set((*FILES, 'image.tar.gz')):
        raise ValueError('安装包文件列表不正确')
    for name, checksum in data['files'].items():
        if (directory / name).is_symlink() or digest(directory / name) != checksum:
            raise ValueError(f'安装包校验失败：{name}')
    return data

class Deployment:
    def __init__(self, directory, timeout=180):
        self.root = directory.resolve()
        self.meta = self.root / '.bunmail'
        self.meta.mkdir(parents=True, exist_ok=True, mode=0o700)
        self.project = 'bunmail-' + hashlib.sha256(str(self.root).encode()).hexdigest()[:12]
        self.timeout = timeout

    @contextmanager
    def lock(self):
        stream = (self.meta / 'lock').open('a+')
        try:
            try:
                fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                raise ValueError('此目录已有部署操作正在进行') from None
            yield
        finally:
            stream.close()

    def state(self):
        path = self.meta / 'state.json'
        return json.loads(path.read_text()) if path.exists() else None

    def save(self, state):
        write_json(self.meta / 'state.json', state)

    def compose(self, release, *arguments, **kwargs):
        directory = self.meta / 'releases' / release['version']
        command = ['docker', 'compose', '--project-name', self.project, '--project-directory', str(self.root), '--env-file', str(self.root / '.env'), '-f', str(directory / 'compose.yml'), '-f', str(directory / 'image.json')]
        if (self.root / '.smtp-enabled').exists():
            command += ['-f', str(directory / 'compose.smtp.yml')]
        return run(*command, *arguments, **kwargs)

    def prepare(self, source):
        release = read_package(source)
        inspect = subprocess.run(['docker', 'image', 'inspect', release['image']], capture_output=True, text=True)
        if inspect.returncode:
            run('docker', 'load', '-i', str(source / 'image.tar.gz'), stdout=subprocess.DEVNULL)
            inspect = run('docker', 'image', 'inspect', release['image'], capture_output=True, text=True)
        info = json.loads(inspect.stdout)[0]
        labels = info['Config'].get('Labels') or {}
        if labels.get('io.bunmail.schema') != release['schema'] or labels.get('org.opencontainers.image.version') != release['version']:
            raise ValueError('镜像版本或数据库指纹不匹配')
        releases = self.meta / 'releases'
        releases.mkdir(exist_ok=True)
        destination = releases / release['version']
        if destination.exists():
            if json.loads((destination / 'release.json').read_text()) != release:
                raise ValueError('已存在内容不同的同版本文件')
        else:
            with tempfile.TemporaryDirectory(prefix='.prepare-', dir=releases) as temporary:
                staging = Path(temporary)
                for name in FILES:
                    shutil.copy2(source / name, staging / name)
                write_json(staging / 'release.json', release)
                write_json(staging / 'image.json', {'services': {'app': {'image': release['image'], 'pull_policy': 'never'}}})
                os.rename(staging, destination)
        self.compose(release, 'config', '--quiet')
        return release

    def backup(self, release):
        directory = self.meta / 'backups'
        directory.mkdir(exist_ok=True, mode=0o700)
        stamp = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S%fZ')
        path = directory / f'{stamp}.dump'
        partial = path.with_suffix('.partial')
        with partial.open('wb') as stream:
            self.compose(release, 'exec', '-T', 'db', 'pg_dump', '-U', 'bunmail', '-d', 'bunmail', '-Fc', stdout=stream)
            stream.flush()
            os.fsync(stream.fileno())
        os.rename(partial, path)
        secrets = directory / f'{stamp}.env'
        shutil.copyfile(self.root / '.env', secrets)
        secrets.chmod(0o600)
        return str(path)

    def start(self, release, install=False):
        if install:
            self.compose(release, 'pull', 'db')
        args = ['up', '-d', '--no-build', '--pull', 'never', '--wait', '--wait-timeout', str(self.timeout)]
        if not install:
            args += ['--no-deps', 'app']
        self.compose(release, *args)

    def prune(self, state):
        keep = {release['version'] for release in [state.get('current'), state.get('previous')] if release}
        for path in (self.meta / 'releases').iterdir():
            if path.is_dir() and not path.is_symlink() and re.fullmatch(r'\d+\.\d+\.\d+', path.name) and path.name not in keep:
                shutil.rmtree(path)

    def resume(self, state):
        pending = state.get('pending')
        if not pending:
            raise ValueError('没有待恢复的操作')
        target, original = pending['target'], pending.get('original')
        for release in (target, original):
            if release:
                run('docker', 'image', 'inspect', release['image'], stdout=subprocess.DEVNULL)
        try:
            if original:
                self.compose(original, 'stop', 'app')
            if original and not pending.get('backup'):
                pending['backup'] = self.backup(original)
                self.save(state)
            self.start(target, install=original is None)
        except (subprocess.CalledProcessError, OSError):
            if original:
                try:
                    self.start(original)
                except (subprocess.CalledProcessError, OSError):
                    raise ValueError('新旧版本启动均失败，保留 pending 和备份；排查后运行 resume') from None
                state.setdefault('failures', []).append(pending)
                state['pending'] = None
                self.save(state)
                self.prune(state)
                raise ValueError('更新失败，已自动恢复原版本；备份已保留') from None
            raise ValueError('首次启动失败，保留安装状态；排查后运行 resume') from None
        state['previous'], state['current'], state['pending'] = original, target, None
        if pending.get('backup'):
            state.setdefault('backups', []).append(pending['backup'])
        self.save(state)
        self.prune(state)
        print(f"当前版本：{target['version']}")

    def change(self, action, source=None):
        with self.lock():
            if not (self.root / '.env').is_file():
                raise ValueError('请先运行 setup.py，在安装目录生成 .env')
            state = self.state()
            if state and (state.get('directory') != str(self.root) or state.get('project') != self.project):
                raise ValueError('安装目录已移动，须先迁移 Docker 卷与部署状态，不能直接启动新实例')
            if action == 'resume':
                if not state:
                    raise ValueError('尚未安装')
                return self.resume(state)
            if action == 'reconfigure':
                if not state or not state.get('current') or state.get('pending'):
                    raise ValueError('需要完整的已安装状态，请先检查 status / resume')
                self.start(state['current'])
                return
            if action == 'install':
                if state:
                    raise ValueError('此目录已安装或存在未完成的安装，请使用 update 或 resume')
                containers = run('docker', 'ps', '-aq', '--filter', f'label=com.docker.compose.project={self.project}', capture_output=True, text=True).stdout.strip()
                manual = run('docker', 'ps', '-aq', '--filter', f'label=com.docker.compose.project.working_dir={self.root}', capture_output=True, text=True).stdout.strip()
                volumes = run('docker', 'volume', 'ls', '-q', '--filter', f'label=com.docker.compose.project={self.project}', capture_output=True, text=True).stdout.strip()
                if containers or manual or volumes:
                    raise ValueError('检测到此目录对应的容器，不能重复安装')
                state = {'directory': str(self.root), 'project': self.project, 'current': None, 'previous': None}
            elif not state or not state.get('current'):
                raise ValueError('此目录尚未安装')
            if state.get('pending'):
                raise ValueError('存在未完成操作，请先运行 resume')
            current = state.get('current')
            if action == 'rollback':
                target = state.get('previous')
                if not target:
                    raise ValueError('没有可回退版本')
            else:
                candidate = read_package(source)
                if current and version(candidate['version']) <= version(current['version']):
                    raise ValueError('更新版本必须高于当前版本')
                if current and candidate['schema'] != current['schema']:
                    raise ValueError('数据库迁移指纹变化，禁止自动更新/回退；须制定独立的数据迁移方案')
                target = self.prepare(source)
            if current and target['schema'] != current['schema']:
                raise ValueError('数据库版本不兼容，不能回退')
            state['pending'] = {'target': target, 'original': current}
            self.save(state)
            self.resume(state)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory', type=Path, default=Path.cwd())
    parser.add_argument('--timeout', type=int, default=180)
    commands = parser.add_subparsers(dest='command', required=True)
    build = commands.add_parser('package')
    build.add_argument('version')
    build.add_argument('--output', type=Path, default=Path('releases'))
    for action in ('install', 'update'):
        commands.add_parser(action).add_argument('package', type=Path)
    for action in ('rollback', 'resume', 'status', 'reconfigure'):
        commands.add_parser(action)
    args = parser.parse_args()
    try:
        if args.command == 'package':
            return package(args.version, args.output.resolve())
        deployment = Deployment(args.directory, args.timeout)
        if args.command == 'status':
            state = deployment.state() or {}
            print(json.dumps({'directory': str(deployment.root), 'project': deployment.project, 'current': (state.get('current') or {}).get('version'), 'previous': (state.get('previous') or {}).get('version'), 'pending': bool(state.get('pending'))}, ensure_ascii=False, indent=2))
        else:
            deployment.change(args.command, getattr(args, 'package', None))
    except (ValueError, OSError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'部署未完成：{error}\n')

if __name__ == '__main__':
    main()
