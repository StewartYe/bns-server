# BNS BIP-322 Login Test

A simple test frontend for BIP-322 authentication with UniSat wallet.

## Prerequisites

- Node.js 18+
- UniSat Wallet browser extension

## Setup

```bash
cd frontend
npm install
npm run dev
```

Then open http://localhost:5173 in your browser.

## Usage

1. Make sure UniSat wallet is installed and unlocked
2. Click "Login with UniSat" button
3. Approve the wallet connection in UniSat popup
4. Sign the login message in UniSat popup
5. View the login result in the log output

## Message Format

The login message format is:
```
Sign in to bns.zone at {timestamp} with nonce {nonce}
```

- `timestamp`: Unix timestamp in seconds
- `nonce`: Random 16-character alphanumeric string

## API Endpoint

The frontend calls `POST /v1/auth/login` with:
```json
{
  "address": "tb1q...",
  "message": "Sign in to bns.zone at 1735344000 with nonce abc123def456",
  "signature": "base64...",
  "timestamp": 1735344000,
  "nonce": "abc123def456"
}
```
