# BNS Server API Documentation

Base URL: `https://bns-server-testnet-219952077564.us-central1.run.app`

## Table of Contents

- [Name Resolution](#name-resolution)
  - [Resolve Name](#resolve-name)
  - [Resolve Address](#resolve-address)
  - [Update Name Metadata](#update-name-metadata)
- [User Settings](#user-settings)
  - [Set Primary Name](#set-primary-name)
  - [Clear Primary Name](#clear-primary-name)
- [Authentication](#authentication)
  - [Login (BIP-322)](#login-bip-322)
  - [Get Current User](#get-current-user)
  - [Logout](#logout)
- [Pool](#pool)
  - [Get/Create Pool](#getcreate-pool)
- [Listings](#listings)
  - [Create Listing](#create-listing)
  - [Get All Listings](#get-all-listings)
- [Rankings](#rankings)
  - [Get Ranking](#get-ranking)
- [WebSocket](#websocket)
  - [Architecture](#architecture)
  - [Connection](#connection)
  - [Subscription Model](#subscription-model)
  - [Message Types](#message-types)
  - [Channels](#channels)
- [Health Check](#health-check)

---

## Name Resolution

### Resolve Name

Get the Bitcoin address, inscription details, and metadata for a rune name.

**Endpoint:** `GET /v1/names/{name}`

**Example:**

```bash
curl https://bns-server-testnet-219952077564.us-central1.run.app/v1/names/P•X•H•M•B•W
```

**Response:**

```json
{
  "result": {
    "name": "P•X•H•M•B•W",
    "address": "tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2",
    "id": "111800:2",
    "inscription_id": "cc26da50bf2866bb3051c9c1c47671bc186f1fad86f085351acc386175a04db9i0",
    "inscription_number": 256889,
    "confirmations": 10001,
    "metadata": {
      "twitter": "OmnityBTCdApps",
      "description": "The official BNS for BNS.ZONE",
      "url": "https://bns.zone",
      "email": "hi@oct.network"
    }
  }
}
```

**Result Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | The queried rune name |
| `address` | string | Bitcoin address that owns the name |
| `id` | string | Rune ID (format: `block:index`) |
| `inscription_id` | string | Inscription ID |
| `inscription_number` | number | Inscription number |
| `confirmations` | number | Number of blockchain confirmations |
| `metadata` | object | Key-value metadata (description, url, twitter, email) |

### Resolve Address

List all rune names belonging to a Bitcoin address with pagination.

**Endpoint:** `GET /v1/addresses/{address}/names`

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `page` | number | 1 | Page number (1-indexed) |
| `page_size` | number | 20 | Number of names per page (max: 100) |

**Example:**

```bash
curl "https://bns-server-testnet-219952077564.us-central1.run.app/v1/addresses/tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2/names?page=1&page_size=20"
```

**Response:**

```json
{
  "address": "tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2",
  "names": [
    {
      "name": "HOPE•YOU•GIVE•RICH•AGAIN",
      "id": "86311:49",
      "is_primary": false,
      "confirmations": 10001
    },
    {
      "name": "HOPE•YOU•GET•RICH•AGAIN",
      "id": "82913:22",
      "is_primary": false,
      "confirmations": 10001
    },
    {
      "name": "PXHMBZ",
      "id": "111800:1",
      "is_primary": false,
      "confirmations": 8767
    },
    {
      "name": "PWAAAA",
      "id": "111837:2",
      "is_primary": false,
      "confirmations": 6001
    },
    {
      "name": "P•X•H•M•B•W",
      "id": "111800:2",
      "is_primary": true,
      "confirmations": 10001
    }
  ],
  "page": 1,
  "page_size": 20,
  "total": 5
}
```

**Name Entry Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | The rune name |
| `id` | string | Rune ID (format: `block:index`) |
| `is_primary` | boolean | Whether this is the user's primary name |
| `confirmations` | number | Number of blockchain confirmations |

### Update Name Metadata

Update metadata for a name you own. Requires authentication. The name must have at least 3 confirmations.

**Endpoint:** `PUT /v1/names/{name}/metadata`

**Headers:**
```
Authorization: Bearer {session_id}
Content-Type: application/json
```

**Request Body:**

```json
{
  "description": "The official BNS for BNS.ZONE",
  "url": "https://bns.zone",
  "twitter": "OmnityBTCdApps",
  "email": "hi@oct.network"
}
```

All fields are optional. Only include the fields you want to set or update.

**Example:**

```bash
curl -X PUT https://bns-server-testnet-219952077564.us-central1.run.app/v1/names/P•X•H•M•B•W/metadata \
  -H "Authorization: Bearer eef97f47-2482-4390-9686-9857df9f3b97:a1b2c3d4-5678-90ab-cdef-1234567890ab" \
  -H "Content-Type: application/json" \
  -d '{
    "description": "My awesome name",
    "url": "https://example.com"
  }'
```

**Response:**

```json
{
  "name": "P•X•H•M•B•W",
  "metadata": {
    "description": "My awesome name",
    "url": "https://example.com"
  }
}
```

**Errors:**

| Status | Description |
|--------|-------------|
| `400` | Name has fewer than 3 confirmations |
| `401` | Not authenticated |
| `403` | Name does not belong to the authenticated address |

---

## User Settings

### Set Primary Name

Set a name as your primary name. The name must belong to your address and have at least 3 confirmations.

**Endpoint:** `PUT /v1/user/primary-name`

**Headers:**
```
Authorization: Bearer {session_id}
Content-Type: application/json
```

**Request Body:**

```json
{
  "name": "P•X•H•M•B•W"
}
```

**Example:**

```bash
curl -X PUT https://bns-server-testnet-219952077564.us-central1.run.app/v1/user/primary-name \
  -H "Authorization: Bearer eef97f47-2482-4390-9686-9857df9f3b97:a1b2c3d4-5678-90ab-cdef-1234567890ab" \
  -H "Content-Type: application/json" \
  -d '{"name": "P•X•H•M•B•W"}'
```

**Response:**

```json
{
  "address": "tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2",
  "primary_name": "P•X•H•M•B•W"
}
```

**Errors:**

| Status | Description |
|--------|-------------|
| `400` | Name has fewer than 3 confirmations |
| `401` | Not authenticated |
| `403` | Name does not belong to the authenticated address |

### Clear Primary Name

Remove your primary name setting.

**Endpoint:** `DELETE /v1/user/primary-name`

**Headers:**
```
Authorization: Bearer {session_id}
```

**Example:**

```bash
curl -X DELETE https://bns-server-testnet-219952077564.us-central1.run.app/v1/user/primary-name \
  -H "Authorization: Bearer eef97f47-2482-4390-9686-9857df9f3b97:a1b2c3d4-5678-90ab-cdef-1234567890ab"
```

**Response:**

```json
{
  "address": "tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2",
  "primary_name": null
}
```

---

## Authentication

BNS Server uses BIP-322 message signing for authentication. This is supported by modern Bitcoin wallets like UniSat.

### Session Management

Sessions are stored in Redis with network-prefixed keys (testnet/mainnet). Authentication supports two methods:

1. **Secure HttpOnly Cookies** (recommended for browsers): The login response sets a `bns_session` cookie with `Secure`, `HttpOnly`, and `SameSite=Strict` attributes.
2. **Bearer Token**: The session token is also returned in the response body for API clients.

### Login (BIP-322)

Authenticate using a BIP-322 signed message.

**Endpoint:** `POST /v1/auth/login`

**Message Format:**
```
Sign in to bns.zone at {timestamp} with nonce {nonce}
```

| Field | Description |
|-------|-------------|
| `timestamp` | Unix timestamp in seconds |
| `nonce` | Random alphanumeric string (8-64 characters) |

**Request Body:**

```json
{
  "address": "tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2",
  "message": "Sign in to bns.zone at 1735344000 with nonce abc123def456",
  "signature": "AUBvt7L2...(base64 BIP-322 signature)"
}
```

**Example:**

```bash
curl -X POST https://bns-server-testnet-219952077564.us-central1.run.app/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "address": "tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2",
    "message": "Sign in to bns.zone at 1735344000 with nonce abc123def456",
    "signature": "AUBvt7L2..."
  }'
```

**Response:**

```json
{
  "session_id": "eef97f47-2482-4390-9686-9857df9f3b97:a1b2c3d4-5678-90ab-cdef-1234567890ab",
  "btc_address": "tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2",
  "expires_at": "2025-12-26T15:24:54.805664323Z",
  "is_new_user": true
}
```

**Response Headers:**
```
Set-Cookie: bns_session=...; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=86400
```

> **Security Notes:**
> - The `session_id` is in the format `session_id:session_secret`. Only SHA256(session_secret) is stored in Redis.
> - Sessions are stored in Redis with network-prefixed keys (`testnet:session:*` or `mainnet:session:*`).
> - Re-login invalidates all previous sessions for the same address.

### Get Current User

Get the currently authenticated user's session information.

**Endpoint:** `GET /v1/auth/me`

**Authentication:** Session cookie or Bearer token

**Example (with Bearer token):**

```bash
curl https://bns-server-testnet-219952077564.us-central1.run.app/v1/auth/me \
  -H "Authorization: Bearer eef97f47-2482-4390-9686-9857df9f3b97:a1b2c3d4-5678-90ab-cdef-1234567890ab"
```

**Example (with cookie):**

```bash
curl https://bns-server-testnet-219952077564.us-central1.run.app/v1/auth/me \
  --cookie "bns_session=eef97f47-2482-4390-9686-9857df9f3b97:a1b2c3d4-5678-90ab-cdef-1234567890ab"
```

**Response:**

```json
{
  "session_id": "eef97f47-2482-4390-9686-9857df9f3b97",
  "btc_address": "tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2",
  "created_at": "2025-12-25T15:24:54.805664Z",
  "expires_at": "2025-12-26T15:24:54.805664Z"
}
```

### Logout

Invalidate the current session and clear the cookie.

**Endpoint:** `POST /v1/auth/logout`

**Authentication:** Session cookie or Bearer token

**Example:**

```bash
curl -X POST https://bns-server-testnet-219952077564.us-central1.run.app/v1/auth/logout \
  -H "Authorization: Bearer eef97f47-2482-4390-9686-9857df9f3b97:a1b2c3d4-5678-90ab-cdef-1234567890ab"
```

**Response:** `204 No Content`

**Response Headers:**
```
Set-Cookie: bns_session=; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=0
```

---

## Pool

### Get/Create Pool

Get or create a pool address for listing a rune name. This is the first step in the listing process. The pool is a Bitcoin address managed by the BNS canister where the rune will be sent for listing.

**Endpoint:** `POST /v1/pool`

**Authentication:** Required (Bearer token or session cookie)

**Headers:**
```
Authorization: Bearer {session_id}
Content-Type: application/json
```

**Request Body:**

```json
{
  "name": "MY•RUNE•NAME"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | The rune name to get/create a pool for |

**Example:**

```bash
curl -X POST https://bns-server-testnet-219952077564.us-central1.run.app/v1/pool \
  -H "Authorization: Bearer eef97f47-2482-4390-9686-9857df9f3b97:a1b2c3d4-5678-90ab-cdef-1234567890ab" \
  -H "Content-Type: application/json" \
  -d '{"name": "MY•RUNE•NAME"}'
```

**Success Response:**

```json
{
  "name": "MY•RUNE•NAME",
  "pool_address": "bc1q..."
}
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | The rune name |
| `pool_address` | string | Bitcoin address to send the rune to for listing |

**Error Response:**

```json
{
  "error": "Error message",
  "code": "ERROR_CODE"
}
```

**Error Codes:**

| Status | Code | Description |
|--------|------|-------------|
| `400` | `BAD_REQUEST` | Invalid request body or JSON |
| `401` | `UNAUTHORIZED` | Authentication required |
| `403` | `NAME_NOT_OWNED` | Name does not belong to authenticated address |
| `403` | `INSUFFICIENT_CONFIRMATIONS` | Name has fewer than 3 confirmations |
| `409` | `POOL_ALREADY_EXISTS` | Pool already exists for this name |
| `502` | `BACKEND_ERROR` | Ord backend unavailable or returned error |
| `502` | `CANISTER_ERROR` | BNS canister call failed |
| `503` | `SERVICE_UNAVAILABLE` | Ord backend not configured |
| `500` | `INTERNAL_ERROR` | Unexpected server error |

**Workflow:**

1. User authenticates via BIP-322
2. User calls `POST /v1/pool` with the rune name they want to list
3. Server verifies:
   - User is authenticated
   - Name belongs to user's Bitcoin address (via Ord backend)
   - Name has at least 3 confirmations
4. Server calls BNS canister to create/get pool
5. Server returns the pool address
6. User sends their rune to the pool address
7. User calls `POST /v1/listings` to complete the listing

---

## Listings

### Create Listing

List a rune name for sale via the IC orchestrator canister invoke flow.

**Endpoint:** `POST /v1/listings`

**Request Body:**

| Field | Type | Description |
|-------|------|-------------|
| `intentionSet` | object | The intention set containing listing intentions |
| `intentionSet.txFeeInSats` | number | Transaction fee in satoshis |
| `intentionSet.initiatorAddress` | string | Initiator's Bitcoin address |
| `intentionSet.intentions` | array | Array of intentions (see below) |
| `psbtHex` | string | PSBT hex string (unsigned, will be signed by canister) |
| `initiatorUtxoProof` | string | Base64 encoded UTXO proof blob from frontend |

**Intention Object:**

| Field | Type | Description |
|-------|------|-------------|
| `action` | string | Action type (e.g., "list") |
| `exchangeId` | string | Exchange identifier |
| `poolAddress` | string | Pool address for the listing |
| `nonce` | number | Transaction nonce |
| `actionParams` | string | JSON string with action parameters |
| `inputCoins` | array | Input coin balances |
| `outputCoins` | array | Output coin destinations |
| `poolUtxoSpent` | array | Pool UTXOs being spent |
| `poolUtxoReceived` | array | Pool UTXOs being received |

**Action Parameters (JSON string in `actionParams`):**

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | The rune name to list |
| `seller_address` | string | Seller's Bitcoin address |
| `seller_token_address` | string? | Optional token receiving address |
| `price` | number | Price in satoshis |

**Example:**

```bash
curl -X POST https://bns-server-testnet-219952077564.us-central1.run.app/v1/listings \
  -H "Content-Type: application/json" \
  -d '{
    "intentionSet": {
      "txFeeInSats": 1000,
      "initiatorAddress": "tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2",
      "intentions": [{
        "action": "ListName",
        "exchangeId": "BNS_Canister",
        "poolAddress": "bc1q...",
        "nonce": 1,
        "actionParams": "{\"name\":\"MY•RUNE•NAME\",\"seller_address\":\"tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2\",\"price\":100000}",
        "inputCoins": [],
        "outputCoins": [],
        "poolUtxoSpent": [],
        "poolUtxoReceived": []
      }]
    },
    "psbtHex": "70736274ff...",
    "initiatorUtxoProof": "base64encodedproof..."
  }'
```

**Response:**

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "txId": "a1b2c3d4e5f6...",
  "name": "MY•RUNE•NAME",
  "priceSats": 100000,
  "sellerAddress": "tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2"
}
```

> **Note:** The transaction is submitted to the IC orchestrator canister for processing. Listing status updates are tracked via canister events.

### Get All Listings

Retrieve all listings with pagination.

**Endpoint:** `GET /v1/listings`

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `limit` | number | 50 | Maximum number of listings to return (max: 100) |
| `offset` | number | 0 | Number of listings to skip |

**Example:**

```bash
curl "https://bns-server-testnet-219952077564.us-central1.run.app/v1/listings?limit=10&offset=0"
```

**Response:**

```json
{
  "listings": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "MY•RUNE•NAME",
      "sellerAddress": "tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2",
      "poolAddress": "bc1q...",
      "priceSats": 100000,
      "status": "pending",
      "listedAt": "2025-12-25T15:24:54.805664Z",
      "txId": "a1b2c3d4e5f6..."
    }
  ],
  "total": 1
}
```

**Listing Status Values:**

| Status | Description |
|--------|-------------|
| `pending` | Transaction broadcast, waiting for confirmations |
| `active` | Listed and available for purchase |
| `sold` | Has been purchased |
| `delisted` | Removed by owner |
| `cancelled` | Cancelled due to error |

---

## Rankings

Rankings provide sorted lists of items (max 20 per ranking). Use the REST endpoint for initial data snapshot, and WebSocket for real-time delta updates.

### Get Ranking

Get the initial snapshot of a ranking.

**Endpoint:** `GET /v1/rankings/{type}`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `type` | string | Ranking type (see supported types below) |

**Supported Ranking Types:**

| Type | Description | Status |
|------|-------------|--------|
| `new-listings` | Newest 20 listings sorted by listing time | Implemented |
| `recent-sales` | Recent sales sorted by sale time | Placeholder |
| `top-earners` | Addresses ranked by cumulative profit | Placeholder |
| `most-traded` | Names ranked by number of transactions | Placeholder |
| `top-sales` | Names ranked by highest sale price | Placeholder |
| `best-deals` | Current listings with highest discount percentage | Placeholder |

**Example:**

```bash
curl https://bns-server-testnet-219952077564.us-central1.run.app/v1/rankings/new-listings
```

**Response (new-listings):**

```json
{
  "rankingType": "new-listings",
  "items": [
    {
      "name": "MY•RUNE•NAME",
      "priceSats": 100000,
      "sellerAddress": "tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2",
      "listedAt": 1735344000,
      "txId": "a1b2c3d4e5f6..."
    }
  ],
  "total": 1
}
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `rankingType` | string | The ranking type |
| `items` | array | List of ranking items (max 20) |
| `total` | number | Total count of items returned |

**Ranking Item Fields by Type:**

**new-listings:**

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | The rune name |
| `priceSats` | number | Price in satoshis |
| `sellerAddress` | string | Seller's Bitcoin address |
| `listedAt` | number | Unix timestamp when listed |
| `txId` | string? | Bitcoin transaction ID (optional) |

**recent-sales:**

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | The rune name |
| `priceSats` | number | Sale price in satoshis |
| `sellerAddress` | string | Seller's Bitcoin address |
| `buyerAddress` | string | Buyer's Bitcoin address |
| `soldAt` | number | Unix timestamp when sold |
| `txId` | string? | Bitcoin transaction ID (optional) |

**top-earners:**

| Field | Type | Description |
|-------|------|-------------|
| `address` | string | Bitcoin address |
| `totalProfitSats` | number | Total profit in satoshis |
| `tradeCount` | number | Number of trades |

**most-traded:**

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | The rune name |
| `tradeCount` | number | Number of transactions |
| `lastPriceSats` | number | Last sale price in satoshis |
| `lastTradedAt` | number | Unix timestamp of last trade |

**top-sales:**

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | The rune name |
| `priceSats` | number | Sale price in satoshis |
| `sellerAddress` | string | Seller's Bitcoin address |
| `buyerAddress` | string | Buyer's Bitcoin address |
| `soldAt` | number | Unix timestamp when sold |

**best-deals:**

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | The rune name |
| `currentPriceSats` | number | Current listing price in satoshis |
| `previousPriceSats` | number | Previous price in satoshis |
| `discountPercent` | number | Discount percentage |
| `sellerAddress` | string | Seller's Bitcoin address |
| `listedAt` | number | Unix timestamp when listed |

**Error Response:**

```json
{
  "error": "Unknown ranking type: invalid-type",
  "supported": ["new-listings", "recent-sales", "top-earners", "most-traded", "top-sales", "best-deals"]
}
```

**Placeholder Response (for unimplemented rankings):**

```json
{
  "rankingType": "recent-sales",
  "items": [],
  "total": 0,
  "message": "This ranking is not yet implemented"
}
```

---

## WebSocket

Real-time delta updates for rankings via Redis Pub/Sub.

### Architecture

The WebSocket system uses a **snapshot + delta** pattern:

1. **Initial Snapshot**: Use `GET /v1/rankings/{type}` to get the full initial data (max 20 items)
2. **Delta Updates**: Subscribe to WebSocket channel to receive real-time updates when new items are added

When receiving a delta update, clients should prepend the new item to their local list and trim to 20 items.

### Connection

**Endpoint:** `wss://bns-server-testnet-219952077564.us-central1.run.app/v1/ws/connect`

### Subscription Model

Clients must explicitly subscribe to channels to receive updates.

**Subscribe:**

```json
{"type": "subscribe", "channel": "new-listings"}
```

**Unsubscribe:**

```json
{"type": "unsubscribe", "channel": "new-listings"}
```

### Channels

| Channel | Description | Status |
|---------|-------------|--------|
| `new-listings` | Delta updates when new listings are added | Implemented |
| `recent-sales` | Delta updates when sales occur | Placeholder |
| `top-earners` | Delta updates for top earner rankings | Placeholder |
| `most-traded` | Delta updates for most traded names | Placeholder |
| `top-sales` | Delta updates for highest sales | Placeholder |
| `best-deals` | Delta updates for best deal listings | Placeholder |

### Message Types

**Subscription Confirmation:**

```json
{"type": "subscribed", "channel": "new-listings"}
```

**Unsubscription Confirmation:**

```json
{"type": "unsubscribed", "channel": "new-listings"}
```

**Delta Update:**

```json
{
  "type": "delta",
  "channel": "new-listings",
  "data": {
    "type": "new_listing",
    "data": {
      "name": "MY•RUNE•NAME",
      "price_sats": 100000,
      "seller_address": "tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2",
      "listed_at": 1735344000,
      "tx_id": "a1b2c3d4e5f6..."
    }
  }
}
```

**Error:**

```json
{"type": "error", "message": "Unknown channel: invalid-channel"}
```

```json
{"type": "error", "message": "Already subscribed to new-listings"}
```

---

## Health Check

Check if the server is running.

**Endpoint:** `GET /health`

**Example:**

```bash
curl https://bns-server-testnet-219952077564.us-central1.run.app/health
```

**Response:** `OK`

---

## Error Responses

All API errors follow this format:

```json
{
  "error": "Error message describing what went wrong"
}
```

**Common HTTP Status Codes:**

| Status | Description |
|--------|-------------|
| `200` | Success |
| `204` | Success (no content) |
| `400` | Bad request (invalid input) |
| `401` | Unauthorized (missing or invalid session) |
| `404` | Not found |
| `500` | Internal server error |
| `502` | Bad gateway (backend service unavailable) |
| `503` | Service unavailable |
