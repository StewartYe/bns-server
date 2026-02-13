#!/bin/bash
set -e

PROJECT_ID="octopus-prod"
PROJECT_NUMBER="219952077564"
REGION="us-central1"
SERVICE_NAME="bns-server-testnet"
CONNECTOR_NAME="bns-connector"
REPO_NAME="bns"
IMAGE_TAG="latest"
IMAGE_URI="${REGION}-docker.pkg.dev/${PROJECT_ID}/${REPO_NAME}/${SERVICE_NAME}:${IMAGE_TAG}"

# Network configuration (testnet or mainnet)
NETWORK="testnet"

# Cloud SQL instance connection name
CLOUD_SQL_INSTANCE="${PROJECT_ID}:${REGION}:octopus"

# Redis/Valkey configuration (GCP Memorystore)
REDIS_HOST="10.128.15.193"
REDIS_PORT="6379"
REDIS_TLS="true"
REDIS_USE_IAM="true"
REDIS_CA_FILE_PATH="/usr/local/share/ca-certificates/valkey-ca.crt"

# Create Artifact Registry repository (if not exists)
echo "Creating Artifact Registry repository..."
gcloud artifacts repositories create ${REPO_NAME} \
    --repository-format=docker \
    --location=${REGION} \
    --project=${PROJECT_ID} \
    2>/dev/null || echo "Repository already exists"

# Build and push image
echo "Building image..."
gcloud builds submit --tag ${IMAGE_URI} --project=${PROJECT_ID}

# IC Canister configuration
BNS_CANISTER_ID="qbtjc-taaaa-aaaao-ql6tq-cai"
ORCHESTRATOR_CANISTER_ID="hvyp5-5yaaa-aaaao-qjxha-cai"
FEE_COLLECTOR="tb1qvkvm9prd9t34m23v8zks0edsltm35ynur98d0y"

# Deploy to Cloud Run
echo "Deploying to Cloud Run (${NETWORK})..."
gcloud run deploy ${SERVICE_NAME} \
    --image=${IMAGE_URI} \
    --platform=managed \
    --region=${REGION} \
    --vpc-connector=${CONNECTOR_NAME} \
    --vpc-egress=private-ranges-only \
    --add-cloudsql-instances=${CLOUD_SQL_INSTANCE} \
    --set-env-vars="NETWORK=${NETWORK},ORD_BACKEND_URL=http://10.128.15.243,BITCOIND_URL=http://omnity:k2BZNDQ4s71dKXa44pYaA5cTENtGzoPkI0JwqG0uvkY@10.128.15.238:8332,REDIS_HOST=${REDIS_HOST},REDIS_PORT=${REDIS_PORT},REDIS_TLS=${REDIS_TLS},REDIS_USE_IAM=${REDIS_USE_IAM},REDIS_CA_FILE_PATH=${REDIS_CA_FILE_PATH},BNS_CANISTER_ID=${BNS_CANISTER_ID},ORCHESTRATOR_CANISTER_ID=${ORCHESTRATOR_CANISTER_ID},FEE_COLLECTOR=${FEE_COLLECTOR}" \
    --set-secrets="DATABASE_URL=bns-testnet-database-url:latest,IC_IDENTITY_PEM=ic-identity-pem-testnet:latest" \
    --port=8080 \
    --cpu=1 \
    --memory=512Mi \
    --min-instances=0 \
    --max-instances=1 \
    --allow-unauthenticated \
    --service-account=${PROJECT_NUMBER}-compute@developer.gserviceaccount.com \
    --project=${PROJECT_ID}

echo ""
echo "Deployment complete!"
gcloud run services describe ${SERVICE_NAME} --region=${REGION} --project=${PROJECT_ID} --format='value(status.url)'
