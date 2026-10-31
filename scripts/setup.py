#!/usr/bin/env python3
"""生成本地配置，不覆盖已有配置或打印凭据。"""
import argparse
import base64
import hashlib
import os
from pathlib import Path
import secrets

parser = argparse.ArgumentParser()
parser.add_argument('--development', action='store_true')
parser.add_argument('--directory', type=Path, default=Path.cwd())
args = parser.parse_args()
root = args.directory.resolve()
root.mkdir(parents=True, exist_ok=True)
db_password = secrets.token_hex(24)
environment = {
    'APP_ENV': 'development' if args.development else 'production',
    'APP_URL': 'http://localhost:3000' if args.development else 'https://mail-admin.your-domain.com',
    'HTTP_PORT': '3000',
    'HTTP_BIND': '127.0.0.1:3000',
    'COMPOSE_PROJECT_NAME': 'bunmail-' + hashlib.sha256(str(root).encode()).hexdigest()[:12],
    'POSTGRES_PASSWORD': db_password,
    'DATABASE_URL': f'postgres://bunmail:{db_password}@127.0.0.1:15432/bunmail',
    'DKIM_ENCRYPTION_KEY': base64.b64encode(secrets.token_bytes(32)).decode(),
    'OWNER_EMAIL': 'admin@bunmail.test' if args.development else 'admin@your-domain.com',
    'BOOTSTRAP_PASSWORD': secrets.token_urlsafe(24),
    'MAIL_HOSTNAME': 'mail.bunmail.test' if args.development else 'mail.your-domain.com',
    'PUBLIC_IP': '',
    'DELIVERY_ENABLED': 'false',
    'SMTP_BIND': '',
    'SMTP_TLS_CERT': '',
    'SMTP_TLS_KEY': '',
}
try:
    descriptor = os.open(root / '.env', os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
except FileExistsError:
    parser.exit(1, '当前目录已有 .env，未覆盖配置。\n')
with os.fdopen(descriptor, 'w') as stream:
    stream.write(''.join(f'{key}={value}\n' for key, value in environment.items()))
print(f'配置已生成：{root / ".env"}；凭据未输出。')
