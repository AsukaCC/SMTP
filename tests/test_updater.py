"""Isolated updater fixtures; no GitHub writes, Docker changes or real deployments."""
import copy
import hashlib
import io
import json
import os
from pathlib import Path
import sys
import socket
import socketserver
import tarfile
import tempfile
import threading
import unittest
import urllib.request
import uuid
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parents[1] / 'scripts'))
import updater
from deploy import FILES, build_repository, write_json


class FakeDeployment:
    def __init__(self, root):
        self.meta = root / '.bunmail'
        self.meta.mkdir()
        self.data = {'current': {'version': '0.1.0', 'schema': 'a' * 64, 'platform': 'linux/amd64'}, 'pending': None}
        self.events = []

    def state(self):
        return copy.deepcopy(self.data)

    def change(self, action, source):
        self.events.append(action)
        self.data['current'] = updater.read_package(source)


class UpdaterTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / '.env').write_text('UPDATE_REPOSITORY=AsukaCC/SMTP\n')
        self.deployment = FakeDeployment(self.root)
        self.agent = updater.Updater(self.root, self.deployment)
        self.request = {'version': '0.1.1', 'repository': 'AsukaCC/SMTP', 'request_id': str(uuid.uuid4())}

    def package(self, schema='a' * 64, platform='linux/amd64'):
        directory = self.root / 'source'
        directory.mkdir()
        files = {}
        for name in (*FILES, 'image.tar.gz'):
            data = b'fixture contents'
            (directory / name).write_bytes(data)
            files[name] = hashlib.sha256(data).hexdigest()
        write_json(directory / 'release.json', {'format': 1, 'version': '0.1.1', 'schema': schema,
                   'platform': platform, 'image': 'sha256:' + 'b' * 64, 'files': files})
        archive = self.root / 'package.tar.gz'
        with tarfile.open(archive, 'w:gz') as stream:
            for path in directory.iterdir():
                stream.add(path, arcname=path.name)
        data = archive.read_bytes()
        name = 'bunmail-rs-0.1.1-linux-amd64.tar.gz'
        asset = {'name': name, 'state': 'uploaded', 'size': len(data),
                 'digest': 'sha256:' + hashlib.sha256(data).hexdigest(),
                 'browser_download_url': f'https://github.com/AsukaCC/SMTP/releases/download/v0.1.1/{name}'}
        release = {'tag_name': 'v0.1.1', 'draft': False, 'prerelease': False, 'assets': [asset]}
        return data, release

    def run_install(self, data, release):
        self.agent.job = {**self.request, 'status': 'queued', 'error': None, 'updated_at': updater.now()}
        with patch.object(updater, 'fetch_release', return_value=release), \
                patch.object(updater, 'open_url', side_effect=lambda _: io.BytesIO(data)):
            self.agent.install('0.1.1', self.deployment.state()['current'])

    def test_valid_package_installs_and_persists_result(self):
        self.run_install(*self.package())
        self.assertEqual(self.deployment.events, ['update'])
        self.assertEqual(self.agent.job['status'], 'succeeded')
        self.assertEqual(updater.Updater(self.root, self.deployment).status()['current_version'], '0.1.1')
        self.assertFalse(list(self.deployment.meta.glob('update-*/*/image.tar.gz')))

    def test_build_source_from_origin_and_actions(self):
        (self.root / '.git').mkdir()
        with patch.dict(os.environ, {}, clear=True), patch('deploy.shutil.which', return_value='git'), patch('deploy.subprocess.run') as git:
            for origin in ('https://github.com/team/fork.git', 'git@github.com:team/fork.git', 'ssh://git@github.com/team/fork'):
                git.return_value.stdout = origin
                self.assertEqual(build_repository(self.root), 'team/fork')
            git.return_value.stdout = 'https://github.com.example.org/team/fork.git'
            self.assertEqual(build_repository(self.root), 'AsukaCC/SMTP')
            os.environ['GITHUB_REPOSITORY'] = 'publisher/other'
            self.assertEqual(build_repository(self.root), 'publisher/other')
            os.environ['GITHUB_REPOSITORY'] = '../other'
            with self.assertRaises(ValueError):
                build_repository(self.root)

    def test_installed_source_follows_current_release_and_respects_override(self):
        (self.root / '.env').write_text('')
        self.assertEqual(self.agent.status()['repository'], 'AsukaCC/SMTP')
        self.deployment.data['current']['repository'] = 'team/fork'
        self.assertEqual(self.agent.status()['repository'], 'team/fork')
        with self.assertRaisesRegex(updater.UpdateError, 'repository_mismatch'):
            self.agent.submit(self.request)
        (self.root / '.env').write_text('UPDATE_REPOSITORY=custom/mail\n')
        self.assertEqual(self.agent.status()['repository'], 'custom/mail')
        (self.root / '.env').write_text('UPDATE_REPOSITORY=\n')
        with self.assertRaisesRegex(updater.UpdateError, 'repository_disabled'):
            self.agent.submit(self.request)

    def test_schema_change_never_reaches_deployment(self):
        self.run_install(*self.package(schema='c' * 64))
        self.assertEqual(self.agent.job['error'], 'schema_mismatch')
        self.assertEqual(self.deployment.events, [])

    def test_wrong_architecture_never_reaches_deployment(self):
        self.run_install(*self.package(platform='linux/arm64'))
        self.assertEqual(self.agent.job['error'], 'platform_mismatch')
        self.assertEqual(self.deployment.events, [])

    def test_checksum_failure_never_reaches_deployment(self):
        data, release = self.package()
        release['assets'][0]['digest'] = 'sha256:' + '0' * 64
        self.run_install(data, release)
        self.assertEqual(self.agent.job['error'], 'checksum_mismatch')
        self.assertEqual(self.deployment.events, [])

    def test_manifest_file_hash_is_checked(self):
        data, release = self.package()
        with patch.object(updater, 'read_package', side_effect=ValueError('invalid file hash')):
            self.run_install(data, release)
        self.assertEqual(self.agent.job['status'], 'failed')
        self.assertEqual(self.deployment.events, [])

    def test_source_archives_missing_digest_and_redirected_urls_are_rejected(self):
        _, valid = self.package()
        for changes in [{'name': 'Source code.tar.gz'}, {'digest': None}, {'size': updater.MAX_ARCHIVE + 1},
                        {'browser_download_url': 'https://example.com/package.tar.gz'}, {'state': 'new'}]:
            release = copy.deepcopy(valid)
            release['assets'][0].update(changes)
            with self.assertRaises(updater.UpdateError):
                updater.select_asset(release, 'AsukaCC/SMTP', '0.1.1', 'linux/amd64')
        valid['assets'] *= 2
        with self.assertRaises(updater.UpdateError):
            updater.select_asset(valid, 'AsukaCC/SMTP', '0.1.1', 'linux/amd64')

    def test_archive_traversal_links_and_duplicates_are_rejected(self):
        for index, (name, kind, duplicate) in enumerate([
            ('../escape', tarfile.REGTYPE, False), ('/escape', tarfile.REGTYPE, False),
            ('deploy.py', tarfile.SYMTYPE, False), ('deploy.py', tarfile.LNKTYPE, False),
            ('deploy.py', tarfile.REGTYPE, True),
        ]):
            archive = self.root / f'bad-{index}.tar.gz'
            with tarfile.open(archive, 'w:gz') as stream:
                member = tarfile.TarInfo(name)
                member.type = kind
                member.linkname = '/etc/passwd' if kind != tarfile.REGTYPE else ''
                stream.addfile(member)
                if duplicate:
                    stream.addfile(member)
            with self.assertRaises(updater.UpdateError):
                updater.unpack(archive, self.root / f'unpack-{index}')
        self.assertFalse((self.root / 'escape').exists())

    def test_duplicate_request_does_not_start_second_worker(self):
        with patch.object(updater.threading, 'Thread') as thread:
            first = self.agent.submit(self.request)
            second = self.agent.submit(self.request)
            self.assertEqual(first, second)
            self.assertEqual(thread.call_count, 1)
            with self.assertRaisesRegex(updater.UpdateError, 'update_busy'):
                self.agent.submit({**self.request, 'request_id': str(uuid.uuid4())})
        restarted = updater.Updater(self.root, self.deployment)
        self.assertEqual(restarted.job['status'], 'interrupted')

    def test_repository_and_version_are_bound_to_installation(self):
        for change in [{'repository': 'attacker/repo'}, {'version': '0.1.0'}, {'version': '../bad'}, {'path': '/tmp'}]:
            with self.assertRaises(ValueError):
                self.agent.submit({**self.request, **change})
        self.assertIsNone(self.agent.job)

    def test_restart_after_successful_switch_recovers_completed_job(self):
        self.agent.job = {**self.request, 'status': 'installing'}
        write_json(self.agent.path, self.agent.job)
        self.deployment.data['current']['version'] = '0.1.1'
        self.assertEqual(updater.Updater(self.root, self.deployment).job['status'], 'succeeded')

    def test_rollback_and_unresolved_failure_are_distinguished(self):
        data, release = self.package()
        def rollback(*args):
            self.agent.progress('rolling_back')
            raise ValueError('automatically restored')
        with patch.object(self.deployment, 'change', side_effect=rollback):
            self.run_install(data, release)
        self.assertEqual(self.agent.job['status'], 'rolled_back')
        def unresolved(*args):
            self.agent.progress('rolling_back')
            self.deployment.data['pending'] = {'target': {'version': '0.1.1'}}
            raise ValueError('both images failed')
        with patch.object(self.deployment, 'change', side_effect=unresolved):
            self.run_install(data, release)
        self.assertEqual(self.agent.job['status'], 'failed')
        self.assertEqual(self.agent.status()['status'], 'blocked')

    def test_asset_redirect_cannot_reach_private_or_untrusted_hosts(self):
        handler = updater.ReleaseRedirect()
        request = urllib.request.Request('https://github.com/AsukaCC/SMTP/releases/download/v0.1.1/package')
        for url in ['http://127.0.0.1/file', 'https://example.com/file', 'https://github.com:444/file', 'https://user@github.com/file']:
            with self.assertRaises(updater.UpdateError):
                handler.redirect_request(request, None, 302, 'Found', {}, url)

    def test_control_socket_serves_status_and_accepts_idempotent_job(self):
        # macOS Unix socket paths have a short length limit.
        with tempfile.TemporaryDirectory(dir='/tmp') as directory:
            path = str(Path(directory) / 'control.sock')
            with socketserver.UnixStreamServer(path, updater.Handler) as server:
                server.updater = self.agent
                def request(method, endpoint, body=b''):
                    thread = threading.Thread(target=server.handle_request)
                    thread.start()
                    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
                        client.settimeout(5)
                        client.connect(path)
                        client.sendall(f'{method} {endpoint} HTTP/1.0\r\nContent-Length: {len(body)}\r\n\r\n'.encode()+body)
                        result = b''
                        while chunk := client.recv(4096):
                            result += chunk
                    thread.join(timeout=5)
                    headers, payload = result.split(b'\r\n\r\n', 1)
                    return headers, json.loads(payload)
                headers, status = request('GET', '/status')
                self.assertIn(b'200 OK', headers)
                self.assertEqual(status['platform'], 'linux/amd64')
                self.agent.job = {**self.request, 'status': 'downloading', 'error': None, 'updated_at': updater.now()}
                headers, job = request('POST', '/install', json.dumps(self.request).encode())
                self.assertIn(b'202 Accepted', headers)
                self.assertEqual(job['status'], 'downloading')
                headers, _ = request('POST', '/arbitrary-command', b'{}')
                self.assertIn(b'404 Not Found', headers)


if __name__ == '__main__':
    unittest.main()
