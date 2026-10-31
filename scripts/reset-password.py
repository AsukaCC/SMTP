#!/usr/bin/env python3
"""服务器管理员密码恢复，密码通过标准输入传递。"""
import argparse
import getpass
from pathlib import Path
import subprocess
from deploy import Deployment

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--directory', type=Path, default=Path.cwd())
args = parser.parse_args()
password = getpass.getpass('新密码（15 至 256 字节）：')
if not 15 <= len(password.encode()) <= 256:
    parser.exit(1, '密码长度不符合要求。\n')
if password != getpass.getpass('重复新密码：'):
    parser.exit(1, '两次密码不一致。\n')
root = args.directory.resolve()
try:
    if (root / '.bunmail/state.json').exists():
        deployment = Deployment(root)
        with deployment.lock():
            state = deployment.state()
            if state.get('pending') or not state.get('current'):
                parser.exit(1, '请先完成待恢复的部署。\n')
            deployment.compose(state['current'], 'exec', '-T', 'app', 'bunmail-rs', '--reset-password', input=password.encode())
    else:
        subprocess.run(['docker', 'compose', '--project-directory', str(root), '-f', str(root / 'docker-compose.yml'), 'exec', '-T', 'app', 'bunmail-rs', '--reset-password'], input=password.encode(), check=True)
except (ValueError, OSError, subprocess.CalledProcessError):
    parser.exit(1, '密码重置失败，请检查服务和数据库状态。\n')
