#!/bin/bash
set -e

PROJECT_ID="octopus-prod"
REGION="us-central1"
SERVICE_NAME="bns-server-testnet"
CONNECTOR_NAME="bns-connector"
REPO_NAME="bns"
IMAGE_TAG="latest"
IMAGE_URI="${REGION}-docker.pkg.dev/${PROJECT_ID}/${REPO_NAME}/${SERVICE_NAME}:${IMAGE_TAG}"

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

# Deploy to Cloud Run
echo "Deploying to Cloud Run..."
gcloud run deploy ${SERVICE_NAME} \
    --image=${IMAGE_URI} \
    --platform=managed \
    --region=${REGION} \
    --vpc-connector=${CONNECTOR_NAME} \
    --vpc-egress=private-ranges-only \
    --set-env-vars="ORD_BACKEND_URL=http://10.128.15.243" \
    --port=8080 \
    --cpu=1 \
    --memory=512Mi \
    --min-instances=0 \
    --max-instances=10 \
    --allow-unauthenticated \
    --project=${PROJECT_ID}

echo ""
echo "Deployment complete!"
gcloud run services describe ${SERVICE_NAME} --region=${REGION} --project=${PROJECT_ID} --format='value(status.url)'
