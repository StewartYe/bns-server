# BNS Server

A Rust API server for Bitcoin Name Service (BNS), deployed on Google Cloud Run.

## Features

- **Name Resolution**: Resolve rune names to Bitcoin addresses
- **BIP-322 Authentication**: Secure wallet-based authentication
- **Marketplace**: List and browse rune names for sale
- **Real-time Updates**: WebSocket support for live data

## API Documentation

See [API.md](./API.md) for complete API documentation with examples.

## Quick Start

```bash
# Resolve a name
curl https://bns-server-testnet-219952077564.us-central1.run.app/v1/names/P•X•H•M•B•W

# Health check
curl https://bns-server-testnet-219952077564.us-central1.run.app/health
```

## Deployment

```bash
./deploy.sh
```

## Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DATABASE_URL` | Yes | - | PostgreSQL connection string |
| `BITCOIND_URL` | Yes | - | Bitcoin Core RPC URL |
| `ORD_BACKEND_URL` | No | - | Ord indexer URL for name resolution |
| `REDIS_HOST` | Yes | - | Redis/Valkey host |
| `REDIS_PORT` | No | 6379 | Redis port |
| `REDIS_TLS` | No | false | Enable TLS for Redis |
| `REDIS_USE_IAM` | No | false | Use GCP IAM for Redis auth |
| `NETWORK` | No | testnet | Network type (testnet/mainnet) |
| `SESSION_TTL_SECS` | No | 86400 | Session TTL in seconds |
| `PORT` | No | 8080 | Server port (set by Cloud Run)
