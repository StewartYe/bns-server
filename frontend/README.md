# BNS SIWB Test Frontend

A simple test frontend for SIWB (Sign-In With Bitcoin) authentication.

## SIWB Canister IDs

| Network  | Canister ID                    |
|----------|--------------------------------|
| Mainnet  | `3ka66-oaaaa-aaaao-qk2kq-cai`  |
| Testnet  | `xhwud-7yaaa-aaaar-qbyqa-cai`  |

Currently configured to use **testnet**.

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
2. Click "Login with UniSat (Full Flow)" button
3. Approve the wallet connection in UniSat popup
4. Sign the SIWB message in UniSat popup
5. View the login result in the log output

## Flow Breakdown

The full login flow consists of:

1. **Connect Wallet** - Calls `setWalletProvider('unisat')` to connect UniSat
2. **Prepare Login** - Calls `siwb_prepare_login(address)` on the canister to get a message to sign
3. **Login** - Signs the message with UniSat, then calls `siwb_login(signature, address, pubkey, sessionKey, signType)` on the canister

## Switching Networks

Edit `src/config.ts` to switch between testnet and mainnet:

```ts
// Change to 'mainnet' for production
export const CURRENT_NETWORK = 'testnet' as const;
```
