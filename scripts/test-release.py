#!/usr/bin/env python3
"""在临时目录和一次性 Docker 卷验证发布流程；不接触现有实例。"""
import argparse
import gzip
import json
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import uuid
from deploy import Deployment, digest, read_package, write_json

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('package', type=Path)
args = parser.parse_args()
source = args.package.resolve()
base = read_package(source)
tags = []


def run(*command, **kwargs):
    return subprocess.run(command, check=True, **kwargs)


with tempfile.TemporaryDirectory(prefix='bunmail-release-test-') as temporary:
    root = Path(temporary)
    instance = root / 'instance'
    run('python3', str(source / 'setup.py'), '--development', '--directory', str(instance))
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        port = listener.getsockname()[1]
    config = instance / '.env'
    config.write_text(config.read_text().replace('HTTP_PORT=3000', f'HTTP_PORT={port}').replace('APP_URL=http://localhost:3000', f'APP_URL=http://localhost:{port}'))
    app = Deployment(instance, timeout=45)
    try:
        base_tag = f'bunmail-release-test:base-{uuid.uuid4().hex}'
        run('docker', 'tag', base['image'], base_tag)
        tags.append(base_tag)
        build_context = root / 'empty-build-context'
        build_context.mkdir()
        app.change('install', source)
        print('PASS install', flush=True)
        try:
            app.change('install', source)
            raise AssertionError('duplicate install accepted')
        except ValueError as error:
            assert '已安装' in str(error)
        app.compose(base, 'exec', '-T', 'db', 'psql', '-U', 'bunmail', '-d', 'bunmail', '-c', "INSERT INTO templates(id,name,subject,html,text) VALUES ('11111111-1111-1111-1111-111111111111','release fixture','test','','persisted')", stdout=subprocess.DEVNULL)

        def fixture(number, fail=False):
            directory = root / number
            shutil.copytree(source, directory)
            tag = f'bunmail-release-test:{uuid.uuid4().hex}'
            tags.append(tag)
            dockerfile = f"FROM {base_tag}\nLABEL org.opencontainers.image.version={number}\n"
            if fail:
                dockerfile += 'CMD ["/bin/false"]\n'
            run('docker', 'build', '-t', tag, '-f', '-', str(build_context), input=dockerfile, text=True, stdout=subprocess.DEVNULL)
            info = json.loads(run('docker', 'image', 'inspect', tag, capture_output=True, text=True).stdout)[0]
            with gzip.open(directory / 'image.tar.gz', 'wb') as stream:
                process = subprocess.Popen(['docker', 'save', tag], stdout=subprocess.PIPE)
                shutil.copyfileobj(process.stdout, stream)
                process.stdout.close()
                assert process.wait() == 0
            manifest = {**base, 'version': number, 'image': info['Id']}
            manifest['files'] = {name: digest(directory / name) for name in base['files']}
            write_json(directory / 'release.json', manifest)
            return directory

        good = fixture('0.1.1')
        broken = fixture('0.1.2', fail=True)
        newer = fixture('0.1.3')
        app.change('update', good)
        assert app.state()['current']['version'] == '0.1.1'
        app.change('rollback')
        assert app.state()['current']['version'] == base['version']
        app.change('update', good)
        print('PASS update and rollback', flush=True)
        try:
            app.change('update', broken)
            raise AssertionError('broken image accepted')
        except ValueError as error:
            assert '已自动恢复' in str(error), str(error)
        assert app.state()['current']['version'] == '0.1.1'
        print('PASS automatic recovery', flush=True)
        app.change('update', newer)
        state = app.state()
        assert sorted(p.name for p in (app.meta / 'releases').iterdir()) == ['0.1.1', '0.1.3']
        assert not list(app.meta.rglob('image.tar.gz'))
        current = state['current']
        output = app.compose(current, 'exec', '-T', 'db', 'psql', '-U', 'bunmail', '-d', 'bunmail', '-Atc', "SELECT text FROM templates WHERE name='release fixture'", capture_output=True, text=True).stdout.strip()
        assert output == 'persisted'
        dump = sorted((app.meta / 'backups').glob('*.dump'))[-1]
        app.compose(current, 'exec', '-T', 'db', 'createdb', '-U', 'bunmail', 'restore_fixture')
        with dump.open('rb') as stream:
            app.compose(current, 'exec', '-T', 'db', 'pg_restore', '-U', 'bunmail', '-d', 'restore_fixture', '--exit-on-error', stdin=stream)
        output = app.compose(current, 'exec', '-T', 'db', 'psql', '-U', 'bunmail', '-d', 'restore_fixture', '-Atc', "SELECT text FROM templates WHERE name='release fixture'", capture_output=True, text=True).stdout.strip()
        assert output == 'persisted'
        print('PASS two-release retention, persisted data, backup restore', flush=True)
    finally:
        state = app.state() or {}
        current = state.get('current') or (state.get('pending') or {}).get('target')
        if current:
            app.compose(current, 'down', '--volumes', '--remove-orphans')
        for tag in tags:
            subprocess.run(['docker', 'image', 'rm', tag], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
