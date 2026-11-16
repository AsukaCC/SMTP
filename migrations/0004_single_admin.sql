-- Retire ordinary accounts and their cascading sessions/reset tokens only.
DELETE FROM accounts WHERE role <> 'admin';
DELETE FROM account_tokens WHERE purpose = 'verify';
DROP TABLE registration_settings;

ALTER TABLE accounts ADD CONSTRAINT accounts_admin_only CHECK (role = 'admin');
ALTER TABLE account_tokens ADD CONSTRAINT account_tokens_reset_only CHECK (purpose = 'reset');
