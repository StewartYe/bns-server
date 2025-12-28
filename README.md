# BNS Server

A proxy API server for Bitcoin Name Service (BNS), deployed on Google Cloud Run.

## API Endpoints

### Resolve Name

Get the Bitcoin address and inscription ID for a rune name.

```bash
curl https://bns-server-testnet-219952077564.us-central1.run.app/v1/names/P•X•H•M•B•W
```

Response:
```json
{
  "result": {
    "address": "tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2",
    "inscription_id": "cc26da50bf2866bb3051c9c1c47671bc186f1fad86f085351acc386175a04db9i0"
  }
}
```

### Resolve Address

List all runes belonging to a Bitcoin address.

```bash
curl https://bns-server-testnet-219952077564.us-central1.run.app/v1/addresses/tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2/names
```

Response:
```json
{
  "rune_names": [
    "HOPE•YOU•GIVE•RICH•AGAIN",
    "HOPE•YOU•GET•RICH•AGAIN",
    "PXHMBZ",
    "PWAAAA",
    "HOPE•YOU•GETTX•RICH•AGAIN",
    "MAKE•RICH•GREAT•AGAIN",
    "P•X•H•M•B•W",
    "HOPE•YOU•NLP•RICH•CISOO",
    "MYTHIC•OMNITY•NETWORK",
    "HOPE•YOU•GET•RICC"
  ]
}
```

### Authentication

#### BIP-322 Login

Authenticate using BIP-322 message signing (supported by UniSat and other modern wallets).

**Message Format:**
```
Sign in to bns.zone at {timestamp} with nonce {nonce}
```

- `timestamp`: Unix timestamp in seconds
- `nonce`: Random alphanumeric string (8-64 characters)

```bash
curl -X POST https://bns-server-testnet-219952077564.us-central1.run.app/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "address": "tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2",
    "message": "Sign in to bns.zone at 1735344000 with nonce abc123def456",
    "signature": "AUBvt7L2...(base64 BIP-322 signature)",
    "timestamp": 1735344000,
    "nonce": "abc123def456"
  }'
```

Response:
```json
{
  "session_id": "eef97f47-2482-4390-9686-9857df9f3b97:a1b2c3d4-5678-90ab-cdef-1234567890ab",
  "btc_address": "tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2",
  "expires_at": "2025-12-26T15:24:54.805664323Z",
  "is_new_user": true
}
```

> **Security Note:** The `session_id` returned is a secure token in the format `session_id:session_secret`. Only the hash of `session_secret` is stored in the database, preventing database administrators from impersonating users. Re-login invalidates all previous sessions to prevent session fixation attacks.

#### Get Current User

Get the currently authenticated user's information.

```bash
curl https://bns-server-testnet-219952077564.us-central1.run.app/v1/auth/me \
  -H "Authorization: Bearer eef97f47-2482-4390-9686-9857df9f3b97:a1b2c3d4-5678-90ab-cdef-1234567890ab"
```

Response:
```json
{
  "session_id": "eef97f47-2482-4390-9686-9857df9f3b97",
  "btc_address": "tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2",
  "created_at": "2025-12-25T15:24:54.805664Z",
  "expires_at": "2025-12-26T15:24:54.805664Z"
}
```

#### Logout

Invalidate the current session.

```bash
curl -X POST https://bns-server-testnet-219952077564.us-central1.run.app/v1/auth/logout \
  -H "Authorization: Bearer eef97f47-2482-4390-9686-9857df9f3b97:a1b2c3d4-5678-90ab-cdef-1234567890ab"
```

Response: `204 No Content`

### Health Check

```bash
curl https://bns-server-testnet-219952077564.us-central1.run.app/health
```

## Deployment

```bash
./deploy.sh
```

## Environment Variables

- `ORD_BACKEND_URL` - Backend service URL (GKE Internal LB)
- `DATABASE_URL` - PostgreSQL connection string
- `SESSION_TTL_SECS` - Session TTL in seconds (default: 86400)
- `PORT` - Server port (default: 8080, set by Cloud Run)
