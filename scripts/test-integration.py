#!/usr/bin/env python3
"""复用本地数据库服务，测试内部创建独立数据库；不读取或输出密码。"""
import os
from pathlib import Path
import subprocess

values = dict(line.split('=', 1) for line in Path('.env').read_text().splitlines() if line and not line.startswith('#'))
environment = {**os.environ, 'TEST_DATABASE_URL': values['DATABASE_URL']}
subprocess.run(['cargo', 'test', '--test', 'integration', '--', '--ignored'], env=environment, check=True)
