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
- [Listings](#listings)
  - [Create Listing](#create-listing)
  - [Get All Listings](#get-all-listings)
  - [Get New Listings](#get-new-listings)
- [WebSocket](#websocket)
  - [Connection](#connection)
  - [Subscription Model](#subscription-model)
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

> **Security Note:** The `session_id` is in the format `session_id:session_secret`. Only the hash of `session_secret` is stored in the database. Re-login invalidates all previous sessions.

### Get Current User

Get the currently authenticated user's session information.

**Endpoint:** `GET /v1/auth/me`

**Headers:**
```
Authorization: Bearer {session_id}
```

**Example:**

```bash
curl https://bns-server-testnet-219952077564.us-central1.run.app/v1/auth/me \
  -H "Authorization: Bearer eef97f47-2482-4390-9686-9857df9f3b97:a1b2c3d4-5678-90ab-cdef-1234567890ab"
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

Invalidate the current session.

**Endpoint:** `POST /v1/auth/logout`

**Headers:**
```
Authorization: Bearer {session_id}
```

**Example:**

```bash
curl -X POST https://bns-server-testnet-219952077564.us-central1.run.app/v1/auth/logout \
  -H "Authorization: Bearer eef97f47-2482-4390-9686-9857df9f3b97:a1b2c3d4-5678-90ab-cdef-1234567890ab"
```

**Response:** `204 No Content`

---

## Listings

### Create Listing

List a rune name for sale by broadcasting a signed PSBT.

**Endpoint:** `POST /v1/listings`

**Request Body:**

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | The rune name to list |
| `priceSats` | number | Price in satoshis |
| `sellerAddress` | string | Seller's Bitcoin address |
| `psbt` | string | Base64 encoded signed PSBT (or txid if already broadcast) |

**Example:**

```bash
curl -X POST https://bns-server-testnet-219952077564.us-central1.run.app/v1/listings \
  -H "Content-Type: application/json" \
  -d '{
    "name": "MY•RUNE•NAME",
    "priceSats": 100000,
    "sellerAddress": "tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2",
    "psbt": "cHNidP8B..."
  }'
```

**Response:**

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "txId": "a1b2c3d4e5f6...",
  "name": "MY•RUNE•NAME",
  "priceSats": 100000,
  "sellerAddress": "tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2",
  "confirmations": 0
}
```

> **Note:** The listing starts with status `pending`. Once the transaction reaches 3 confirmations, the status changes to `active`.

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
      "poolAddress": "",
      "priceSats": 100000,
      "status": "pending",
      "listedAt": "2025-12-25T15:24:54.805664Z",
      "txId": "a1b2c3d4e5f6...",
      "confirmations": 2
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

### Get New Listings

Get the 20 newest listings (from Redis cache, faster than database query).

**Endpoint:** `GET /v1/listings/new`

**Example:**

```bash
curl https://bns-server-testnet-219952077564.us-central1.run.app/v1/listings/new
```

**Response:**

```json
{
  "listings": [
    {
      "name": "MY•RUNE•NAME",
      "price_sats": 100000,
      "seller_address": "tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2",
      "confirmations": 2,
      "listed_at": 1735344000,
      "tx_id": "a1b2c3d4e5f6..."
    }
  ]
}
```

---

## WebSocket

Real-time updates for listings and other events.

### Connection

**Endpoint:** `wss://bns-server-testnet-219952077564.us-central1.run.app/v1/ws/connect`

**JavaScript Example:**

```javascript
const ws = new WebSocket('wss://bns-server-testnet-219952077564.us-central1.run.app/v1/ws/connect');

ws.onopen = () => {
  console.log('Connected');
  // Subscribe to a channel
  ws.send(JSON.stringify({
    type: 'subscribe',
    channel: 'new-listings'
  }));
};

ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log('Received:', data);
};

ws.onclose = () => {
  console.log('Disconnected');
};
```

### Subscription Model

The WebSocket uses a subscription-based model. Clients must explicitly subscribe to channels to receive updates.

**Subscribe to a channel:**

```json
{
  "type": "subscribe",
  "channel": "new-listings"
}
```

**Unsubscribe from a channel:**

```json
{
  "type": "unsubscribe",
  "channel": "new-listings"
}
```

### Message Types

| Type | Description |
|------|-------------|
| `subscribed` | Confirmation that subscription was successful |
| `unsubscribed` | Confirmation that unsubscription was successful |
| `snapshot` | Initial data sent immediately after subscribing |
| `update` | Periodic updates (every 5 seconds) |

### Channels

#### `new-listings`

Receive updates about the newest 20 listings.

**Subscribe:**

```json
{"type": "subscribe", "channel": "new-listings"}
```

**Subscription Confirmation:**

```json
{"type": "subscribed", "channel": "new-listings"}
```

**Snapshot (sent immediately after subscribe):**

```json
{
  "type": "snapshot",
  "channel": "new-listings",
  "data": [
    {
      "name": "MY•RUNE•NAME",
      "price_sats": 100000,
      "seller_address": "tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2",
      "confirmations": 2,
      "listed_at": 1735344000,
      "tx_id": "a1b2c3d4e5f6..."
    }
  ]
}
```

**Periodic Update:**

```json
{
  "type": "update",
  "channel": "new-listings",
  "data": [
    {
      "name": "MY•RUNE•NAME",
      "price_sats": 100000,
      "seller_address": "tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2",
      "confirmations": 3,
      "listed_at": 1735344000,
      "tx_id": "a1b2c3d4e5f6..."
    }
  ]
}
```

### Complete WebSocket Example

```javascript
class BNSWebSocket {
  constructor(url) {
    this.url = url;
    this.ws = null;
    this.reconnectInterval = 3000;
  }

  connect() {
    this.ws = new WebSocket(this.url);

    this.ws.onopen = () => {
      console.log('BNS WebSocket connected');
      // Subscribe to new listings
      this.subscribe('new-listings');
    };

    this.ws.onmessage = (event) => {
      const message = JSON.parse(event.data);
      this.handleMessage(message);
    };

    this.ws.onclose = () => {
      console.log('BNS WebSocket disconnected, reconnecting...');
      setTimeout(() => this.connect(), this.reconnectInterval);
    };

    this.ws.onerror = (error) => {
      console.error('BNS WebSocket error:', error);
    };
  }

  subscribe(channel) {
    this.ws.send(JSON.stringify({
      type: 'subscribe',
      channel: channel
    }));
  }

  unsubscribe(channel) {
    this.ws.send(JSON.stringify({
      type: 'unsubscribe',
      channel: channel
    }));
  }

  handleMessage(message) {
    switch (message.type) {
      case 'subscribed':
        console.log(`Subscribed to ${message.channel}`);
        break;
      case 'unsubscribed':
        console.log(`Unsubscribed from ${message.channel}`);
        break;
      case 'snapshot':
        console.log(`Initial data for ${message.channel}:`, message.data);
        break;
      case 'update':
        console.log(`Update for ${message.channel}:`, message.data);
        break;
    }
  }
}

// Usage
const bns = new BNSWebSocket('wss://bns-server-testnet-219952077564.us-central1.run.app/v1/ws/connect');
bns.connect();
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
