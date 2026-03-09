# Docker Mock Services

Lightweight mock servers for integration testing.

## Mock GitHub GraphQL API

Simulates GitHub's GraphQL endpoint with canned responses for:
- Token validation (`viewer` query)
- Batched PR fetching (`pullRequests` query)
- Rate limit simulation

### Tokens

| Token | Behavior |
|-------|----------|
| `ghp_test_token_valid` | Returns successful responses |
| `ghp_test_rate_limited` | Returns 403 with rate limit headers |
| Any other value | Returns 401 Unauthorized |

### Usage

```bash
# Start mock server
docker compose up -d

# Verify it's running
curl -s http://localhost:4000/health

# Run integration tests against it
cd src-tauri && cargo test --test integration -- --ignored

# Stop
docker compose down
```

### Endpoints

- `POST /graphql` - GraphQL endpoint (checks Authorization header)
- `GET /health` - Health check (no auth required)

### Fixtures

- `mock-github/fixtures/viewer.json` - Response for viewer/profile queries
- `mock-github/fixtures/pull-requests.json` - Response for batched PR queries (2 repos, 5 PRs)
