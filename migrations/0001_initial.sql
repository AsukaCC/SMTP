CREATE TABLE owner (
    id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id),
    email TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE sessions (
    token_hash TEXT PRIMARY KEY,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE api_keys (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    prefix TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at TIMESTAMPTZ
);
CREATE TABLE domains (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    selector TEXT NOT NULL DEFAULT 'bunmail',
    public_key TEXT NOT NULL,
    private_key TEXT NOT NULL,
    checks JSONB NOT NULL DEFAULT '{}',
    verified BOOLEAN NOT NULL DEFAULT FALSE,
    checked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE templates (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    subject TEXT NOT NULL,
    html TEXT NOT NULL DEFAULT '',
    text TEXT NOT NULL DEFAULT '',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE emails (
    id UUID PRIMARY KEY,
    api_key_id UUID REFERENCES api_keys(id),
    domain_id UUID NOT NULL REFERENCES domains(id),
    from_address TEXT NOT NULL,
    to_address TEXT NOT NULL,
    subject TEXT NOT NULL,
    html TEXT NOT NULL DEFAULT '',
    text TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'queued' CHECK (status IN ('queued', 'sending', 'sent', 'failed', 'cancelled')),
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    lease_until TIMESTAMPTZ,
    lease_token UUID,
    last_error TEXT,
    message_id TEXT NOT NULL,
    idempotency_scope TEXT,
    idempotency_key TEXT,
    request_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    sent_at TIMESTAMPTZ,
    UNIQUE (idempotency_scope, idempotency_key)
);
CREATE INDEX emails_queue ON emails (next_attempt_at) WHERE status IN ('queued', 'sending');
CREATE INDEX emails_created ON emails (created_at DESC);
CREATE TABLE delivery_attempts (
    id BIGSERIAL PRIMARY KEY,
    email_id UUID NOT NULL REFERENCES emails(id),
    result TEXT NOT NULL,
    detail TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
