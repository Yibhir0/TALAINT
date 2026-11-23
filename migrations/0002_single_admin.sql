CREATE UNIQUE INDEX IF NOT EXISTS idx_auth_tokens_single_admin
ON auth_tokens ((1))
WHERE kind = 'admin';
