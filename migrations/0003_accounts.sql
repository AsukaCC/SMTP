CREATE TABLE accounts (
    id UUID PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('admin', 'user')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX accounts_one_admin ON accounts (role) WHERE role = 'admin';

INSERT INTO accounts (id, email, password_hash, role)
SELECT gen_random_uuid(), lower(email), password_hash, 'admin'
FROM owner;

ALTER TABLE sessions ADD COLUMN account_id UUID REFERENCES accounts(id) ON DELETE CASCADE;
UPDATE sessions
SET account_id = (SELECT id FROM accounts WHERE role = 'admin')
WHERE account_id IS NULL
  AND EXISTS (SELECT 1 FROM accounts WHERE role = 'admin');
DELETE FROM sessions WHERE account_id IS NULL;
ALTER TABLE sessions ALTER COLUMN account_id SET NOT NULL;

CREATE TABLE registration_settings (
    id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id),
    enabled BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE account_tokens (
    token_hash TEXT PRIMARY KEY,
    purpose TEXT NOT NULL CHECK (purpose IN ('verify', 'reset')),
    email TEXT NOT NULL,
    password_hash TEXT,
    account_id UUID REFERENCES accounts(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (
        (purpose = 'verify' AND password_hash IS NOT NULL AND account_id IS NULL)
        OR (purpose = 'reset' AND password_hash IS NULL AND account_id IS NOT NULL)
    )
);
CREATE INDEX account_tokens_email ON account_tokens (purpose, email);
