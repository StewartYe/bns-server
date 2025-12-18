# BNS Server

A proxy API server for Bitcoin Name Service (BNS), deployed on Google Cloud Run.

## API Endpoints

### Resolve Rune

Get the Bitcoin address and inscription ID for a rune.

```bash
curl https://bns-server-testnet-219952077564.us-central1.run.app/resolve_rune/P•X•H•M•B•W
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
curl https://bns-server-testnet-219952077564.us-central1.run.app/resolve_address/tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2
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
- `PORT` - Server port (default: 8080, set by Cloud Run)
