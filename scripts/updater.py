#!/usr/bin/env python3
"""Host-side updater for a single managed BunMail installation (Python 3.11+)."""
import argparse
from datetime import datetime, timezone
import fcntl
import hashlib
from http.server import BaseHTTPRequestHandler
import json
import os
from pathlib import Path
import re
import shutil
import socketserver
import subprocess
import tarfile
import tempfile
import threading
import time
import urllib.parse
import urllib.request
import uuid

from deploy import Deployment, FILES, read_package, version, write_json

ACTIVE = {'queued', 'downloading', 'validating', 'backing_up', 'installing', 'rolling_back'}
MAX_ARCHIVE = 2 * 1024**3
MAX_UNPACKED = 4 * 1024**3


class UpdateError(ValueError):
    pass


def now():
    return datetime.now(timezone.utc).isoformat()


def repository(root):
    values = dict(line.split('=', 1) for line in (root / '.env').read_text().splitlines()
                  if line and not line.startswith('#') and '=' in line)
    repo = values.get('UPDATE_REPOSITORY', 'AsukaCC/SMTP')
    if not re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', repo) or any(p in ('.', '..') for p in repo.split('/')):
        raise UpdateError('repository_disabled')
    return repo


class ReleaseRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, message, headers, newurl):
        target = urllib.parse.urlsplit(newurl)
        if (urllib.parse.urlsplit(request.full_url).hostname == 'api.github.com'
                or target.scheme != 'https' or target.port not in (None, 443)
                or target.username or target.password
                or target.hostname not in ('github.com', 'release-assets.githubusercontent.com', 'objects.githubusercontent.com')):
            raise UpdateError('download_failed')
        return super().redirect_request(request, fp, code, message, headers, newurl)


def open_url(url):
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), ReleaseRedirect())
    return opener.open(urllib.request.Request(url, headers={
        'User-Agent': 'BunMail-Updater', 'Accept': 'application/vnd.github+json'}), timeout=30)


def fetch_release(repo):
    with open_url(f'https://api.github.com/repos/{repo}/releases/latest') as response:
        body = response.read(512 * 1024 + 1)
    if len(body) > 512 * 1024:
        raise UpdateError('release_invalid')
    return json.loads(body)


def select_asset(release, repo, number, platform):
    tag = release.get('tag_name', '')
    if tag not in (number, 'v' + number) or release.get('draft', True) or release.get('prerelease', True):
        raise UpdateError('release_changed')
    if platform not in ('linux/amd64', 'linux/arm64'):
        raise UpdateError('platform_mismatch')
    name = f"bunmail-rs-{number}-{platform.replace('/', '-')}.tar.gz"
    assets = [asset for asset in release.get('assets', []) if asset.get('name') == name]
    if len(assets) != 1:
        raise UpdateError('package_missing')
    asset = assets[0]
    expected = f'https://github.com/{repo}/releases/download/{tag}/{name}'
    if (asset.get('browser_download_url') != expected
            or not re.fullmatch(r'sha256:[a-f0-9]{64}', asset.get('digest') or '')
            or not isinstance(asset.get('size'), int) or not 0 < asset['size'] <= MAX_ARCHIVE
            or asset.get('state') != 'uploaded'):
        raise UpdateError('package_invalid')
    return asset


def download(asset, destination):
    checksum = hashlib.sha256()
    total = 0
    deadline = time.monotonic() + 30 * 60
    with open_url(asset['browser_download_url']) as response, destination.open('xb') as stream:
        while chunk := response.read(1024 * 1024):
            total += len(chunk)
            if total > asset['size'] or total > MAX_ARCHIVE or time.monotonic() > deadline:
                raise UpdateError('package_invalid')
            checksum.update(chunk)
            stream.write(chunk)
    if total != asset['size'] or 'sha256:' + checksum.hexdigest() != asset['digest']:
        raise UpdateError('checksum_mismatch')


def unpack(archive, destination):
    expected = set((*FILES, 'image.tar.gz', 'release.json'))
    seen, total = set(), 0
    destination.mkdir()
    with tarfile.open(archive, 'r|gz') as stream:
        for member in stream:
            total += member.size
            limit = MAX_UNPACKED if member.name == 'image.tar.gz' else 2 * 1024**2
            if (member.name not in expected or member.name in seen or not member.isfile()
                    or member.size < 0 or member.size > limit or total > MAX_UNPACKED):
                raise UpdateError('archive_invalid')
            seen.add(member.name)
            with stream.extractfile(member) as source, (destination / member.name).open('xb') as target:
                shutil.copyfileobj(source, target)
    if seen != expected:
        raise UpdateError('archive_invalid')


class Updater:
    def __init__(self, root, deployment=None):
        self.root = root.resolve()
        self.deployment = deployment or Deployment(self.root, progress=self.progress)
        self.repo = repository(self.root)
        self.path = self.deployment.meta / 'update-job.json'
        self.mutex = threading.RLock()
        self.job = json.loads(self.path.read_text()) if self.path.exists() else None
        if self.job and self.job['status'] in ACTIVE:
            state = self.deployment.state() or {}
            complete = not state.get('pending') and (state.get('current') or {}).get('version') == self.job['version']
            self.progress('succeeded' if complete else 'interrupted', None if complete else 'update_interrupted')

    def status(self):
        with self.mutex:
            state = self.deployment.state() or {}
            current = state.get('current') or {}
            return {'status': 'ready' if current and not state.get('pending') else 'blocked',
                    'repository': self.repo, 'platform': current.get('platform'),
                    'current_version': current.get('version'), 'job': self.job}

    def progress(self, status, error=None):
        with self.mutex:
            self.job = {**self.job, 'status': status, 'error': error, 'updated_at': now()}
            write_json(self.path, self.job)

    def submit(self, request):
        if set(request) != {'version', 'repository', 'request_id'}:
            raise UpdateError('invalid_request')
        number = request['version']
        version(number)
        uuid.UUID(request['request_id'])
        if request['repository'] != self.repo:
            raise UpdateError('repository_mismatch')
        with self.mutex:
            if self.job and self.job['request_id'] == request['request_id']:
                if self.job['version'] != number:
                    raise UpdateError('invalid_request')
                return self.job.copy()
            if self.job and self.job['status'] in ACTIVE:
                raise UpdateError('update_busy')
            state = self.deployment.state() or {}
            current = state.get('current')
            if not current or state.get('pending'):
                raise UpdateError('deployment_blocked')
            if version(number) <= version(current['version']):
                raise UpdateError('version_not_newer')
            self.job = {'request_id': request['request_id'], 'version': number,
                        'status': 'queued', 'error': None, 'updated_at': now()}
            write_json(self.path, self.job)
            threading.Thread(target=self.install, args=(number, current.copy()), daemon=False).start()
            return self.job.copy()

    def install(self, number, original):
        try:
            self.progress('downloading')
            asset = select_asset(fetch_release(self.repo), self.repo, number, original['platform'])
            if shutil.disk_usage(self.deployment.meta).free < asset['size'] * 3 + 512 * 1024**2:
                raise UpdateError('insufficient_space')
            with tempfile.TemporaryDirectory(prefix='update-', dir=self.deployment.meta) as temporary:
                directory = Path(temporary)
                archive = directory / 'package.tar.gz'
                download(asset, archive)
                self.progress('validating')
                source = directory / 'package'
                unpack(archive, source)
                candidate = read_package(source)
                if candidate['version'] != number or candidate['platform'] != original['platform']:
                    raise UpdateError('platform_mismatch')
                if candidate['schema'] != original['schema']:
                    raise UpdateError('schema_mismatch')
                # The installed deployment engine is trusted; downloaded Python files are never executed.
                self.deployment.change('update', source)
            self.progress('succeeded')
        except Exception as error:
            state = self.deployment.state() or {}
            restored = (self.job['status'] == 'rolling_back' and not state.get('pending')
                        and (state.get('current') or {}).get('version') == original['version'])
            code = str(error) if isinstance(error, UpdateError) else 'update_failed'
            self.progress('rolled_back' if restored else 'failed', code)


class Handler(BaseHTTPRequestHandler):
    def setup(self):
        self.request.settimeout(5)
        super().setup()

    def log_message(self, *args):
        pass

    def reply(self, status, value):
        body = json.dumps(value).encode()
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path != '/status':
            return self.reply(404, {'error': 'not_found'})
        self.reply(200, self.server.updater.status())

    def do_POST(self):
        if self.path != '/install':
            return self.reply(404, {'error': 'not_found'})
        try:
            size = int(self.headers.get('Content-Length', '0'))
            if not 0 < size <= 4096:
                raise UpdateError('invalid_request')
            request = json.loads(self.rfile.read(size))
            self.reply(202, self.server.updater.submit(request))
        except (ValueError, TypeError, KeyError, AttributeError) as error:
            self.reply(409, {'error': str(error) if isinstance(error, UpdateError) else 'invalid_request'})


def socket_directory(app, gid):
    path = app.meta / 'updater'
    path.mkdir(exist_ok=True, mode=0o750)
    os.chown(path, -1, gid)
    path.chmod(0o750)
    return path


def enable(root, gid):
    app = Deployment(root)
    state = app.state() or {}
    if not state.get('current') or state.get('pending'):
        raise UpdateError('deployment_blocked')
    directory = socket_directory(app, gid)
    service = app.meta / 'updater-service'
    service.mkdir(exist_ok=True, mode=0o700)
    for name in ('updater.py', 'deploy.py'):
        source = Path(__file__).with_name(name)
        if source.resolve() != (service / name).resolve():
            shutil.copy2(source, service / name)
    write_json(directory / 'compose.json', {'services': {'app': {
        'environment': {'UPDATE_SOCKET': '/run/bunmail-updater/control.sock'},
        'volumes': [{'type': 'bind', 'source': str(directory), 'target': '/run/bunmail-updater',
                     'read_only': True, 'bind': {'create_host_path': False}}]}}})
    (root / '.web-updates-enabled').touch(mode=0o600)
    app.change('reconfigure')
    print(f'Updater service: {service / "updater.py"}')


def serve(root, gid):
    app = Deployment(root)
    with (app.meta / 'updater.lock').open('a+') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        directory = socket_directory(app, gid)
        socket = directory / 'control.sock'
        socket.unlink(missing_ok=True)
        updater = Updater(root)
        # A single control connection at a time bounds local socket resource use; jobs run separately.
        with socketserver.UnixStreamServer(str(socket), Handler) as server:
            os.chown(socket, -1, gid)
            socket.chmod(0o660)
            server.updater = updater
            server.serve_forever()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory', type=Path, required=True)
    parser.add_argument('--socket-gid', type=int, default=10001)
    parser.add_argument('command', choices=('enable', 'serve'))
    args = parser.parse_args()
    try:
        {'enable': enable, 'serve': serve}[args.command](args.directory.resolve(), args.socket_gid)
    except (ValueError, OSError, subprocess.CalledProcessError):
        parser.exit(1, 'Updater unavailable; check managed installation, permissions and service configuration.\n')


if __name__ == '__main__':
    main()
