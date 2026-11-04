#!/usr/bin/env bash
# One-time Google Cloud setup for deploying cxa to Cloud Run from GitHub
# Actions (keyless, via Workload Identity Federation).
#
# Run in Cloud Shell (or anywhere gcloud is authenticated as a project owner):
#
#   PROJECT_ID=my-project ./deploy/gcp-setup.sh
#
# Optional: REGION (us-central1), GITHUB_REPO (Yash-Kavaiya/dfcx-crafts),
# SERVICE (cxa), REPOSITORY (cxa), BUCKET (<project>-cxa-data),
# CREATE_API_KEY (1 = generate a management API key in Secret Manager).
#
# The script is idempotent: re-running it skips what already exists.
set -euo pipefail

PROJECT_ID="${PROJECT_ID:?set PROJECT_ID to your Google Cloud project id}"
REGION="${REGION:-us-central1}"
GITHUB_REPO="${GITHUB_REPO:-Yash-Kavaiya/dfcx-crafts}"
SERVICE="${SERVICE:-cxa}"
REPOSITORY="${REPOSITORY:-cxa}"
BUCKET="${BUCKET:-${PROJECT_ID}-cxa-data}"
CREATE_API_KEY="${CREATE_API_KEY:-1}"
API_KEY_SECRET="${API_KEY_SECRET:-cxa-api-key}"
POOL="${POOL:-github}"
PROVIDER="${PROVIDER:-github-actions}"
DEPLOY_SA_NAME="${DEPLOY_SA_NAME:-cxa-deployer}"
RUNTIME_SA_NAME="${RUNTIME_SA_NAME:-cxa-runtime}"

DEPLOY_SA="${DEPLOY_SA_NAME}@${PROJECT_ID}.iam.gserviceaccount.com"
RUNTIME_SA="${RUNTIME_SA_NAME}@${PROJECT_ID}.iam.gserviceaccount.com"

step() { printf '\n\033[1m==> %s\033[0m\n' "$*"; }
exists() { "$@" >/dev/null 2>&1; }

gcloud config set project "$PROJECT_ID" >/dev/null
PROJECT_NUMBER="$(gcloud projects describe "$PROJECT_ID" --format='value(projectNumber)')"

step "Enabling APIs"
gcloud services enable \
  run.googleapis.com artifactregistry.googleapis.com iam.googleapis.com \
  iamcredentials.googleapis.com sts.googleapis.com secretmanager.googleapis.com \
  storage.googleapis.com cloudresourcemanager.googleapis.com

step "Artifact Registry repository ${REPOSITORY} (${REGION})"
exists gcloud artifacts repositories describe "$REPOSITORY" --location "$REGION" ||
  gcloud artifacts repositories create "$REPOSITORY" --location "$REGION" \
    --repository-format docker --description "Conversational Agents platform images"

step "Service accounts"
exists gcloud iam service-accounts describe "$DEPLOY_SA" ||
  gcloud iam service-accounts create "$DEPLOY_SA_NAME" --display-name "cxa GitHub deployer"
exists gcloud iam service-accounts describe "$RUNTIME_SA" ||
  gcloud iam service-accounts create "$RUNTIME_SA_NAME" --display-name "cxa Cloud Run runtime"

step "Deployer permissions (least privilege)"
gcloud projects add-iam-policy-binding "$PROJECT_ID" --member "serviceAccount:${DEPLOY_SA}" \
  --role roles/run.admin --condition None >/dev/null
gcloud artifacts repositories add-iam-policy-binding "$REPOSITORY" --location "$REGION" \
  --member "serviceAccount:${DEPLOY_SA}" --role roles/artifactregistry.writer >/dev/null
# The deployer may only act as the runtime service account.
gcloud iam service-accounts add-iam-policy-binding "$RUNTIME_SA" \
  --member "serviceAccount:${DEPLOY_SA}" --role roles/iam.serviceAccountUser >/dev/null

step "Data bucket gs://${BUCKET} (platform state)"
exists gcloud storage buckets describe "gs://${BUCKET}" ||
  gcloud storage buckets create "gs://${BUCKET}" --location "$REGION" --uniform-bucket-level-access
gcloud storage buckets add-iam-policy-binding "gs://${BUCKET}" \
  --member "serviceAccount:${RUNTIME_SA}" --role roles/storage.objectUser >/dev/null

if [ "$CREATE_API_KEY" = "1" ]; then
  step "Management API key in Secret Manager (${API_KEY_SECRET})"
  if ! exists gcloud secrets describe "$API_KEY_SECRET"; then
    KEY="$(head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n')"
    printf '%s' "$KEY" | gcloud secrets create "$API_KEY_SECRET" --replication-policy automatic --data-file=-
    echo "Generated API key (store it in your password manager; it is also retrievable from Secret Manager):"
    echo "  $KEY"
  fi
  gcloud secrets add-iam-policy-binding "$API_KEY_SECRET" \
    --member "serviceAccount:${RUNTIME_SA}" --role roles/secretmanager.secretAccessor >/dev/null
fi

step "Workload Identity Federation for ${GITHUB_REPO}"
exists gcloud iam workload-identity-pools describe "$POOL" --location global ||
  gcloud iam workload-identity-pools create "$POOL" --location global --display-name "GitHub Actions"
exists gcloud iam workload-identity-pools providers describe "$PROVIDER" --location global --workload-identity-pool "$POOL" ||
  gcloud iam workload-identity-pools providers create-oidc "$PROVIDER" \
    --location global --workload-identity-pool "$POOL" \
    --display-name "GitHub Actions OIDC" \
    --issuer-uri "https://token.actions.githubusercontent.com" \
    --attribute-mapping "google.subject=assertion.sub,attribute.repository=assertion.repository,attribute.ref=assertion.ref" \
    --attribute-condition "assertion.repository == '${GITHUB_REPO}'"
gcloud iam service-accounts add-iam-policy-binding "$DEPLOY_SA" \
  --role roles/iam.workloadIdentityUser \
  --member "principalSet://iam.googleapis.com/projects/${PROJECT_NUMBER}/locations/global/workloadIdentityPools/${POOL}/attribute.repository/${GITHUB_REPO}" >/dev/null

WIF_PROVIDER="projects/${PROJECT_NUMBER}/locations/global/workloadIdentityPools/${POOL}/providers/${PROVIDER}"

step "Done. Add these GitHub repository variables (Settings → Secrets and variables → Actions → Variables):"
cat <<EOF
  GCP_PROJECT_ID                 = ${PROJECT_ID}
  GCP_REGION                     = ${REGION}
  GCP_WORKLOAD_IDENTITY_PROVIDER = ${WIF_PROVIDER}
  GCP_DEPLOY_SERVICE_ACCOUNT     = ${DEPLOY_SA}
  GCP_RUNTIME_SERVICE_ACCOUNT    = ${RUNTIME_SA}
  GCS_BUCKET                     = ${BUCKET}
  API_KEY_SECRET                 = $( [ "$CREATE_API_KEY" = "1" ] && echo "$API_KEY_SECRET" || echo "(not set)" )
  CLOUD_RUN_SERVICE              = ${SERVICE}
  ARTIFACT_REPOSITORY            = ${REPOSITORY}

Or with the GitHub CLI:
  gh variable set GCP_PROJECT_ID --body "${PROJECT_ID}" -R ${GITHUB_REPO}
  gh variable set GCP_REGION --body "${REGION}" -R ${GITHUB_REPO}
  gh variable set GCP_WORKLOAD_IDENTITY_PROVIDER --body "${WIF_PROVIDER}" -R ${GITHUB_REPO}
  gh variable set GCP_DEPLOY_SERVICE_ACCOUNT --body "${DEPLOY_SA}" -R ${GITHUB_REPO}
  gh variable set GCP_RUNTIME_SERVICE_ACCOUNT --body "${RUNTIME_SA}" -R ${GITHUB_REPO}
  gh variable set GCS_BUCKET --body "${BUCKET}" -R ${GITHUB_REPO}
EOF
[ "$CREATE_API_KEY" = "1" ] && echo "  gh variable set API_KEY_SECRET --body \"${API_KEY_SECRET}\" -R ${GITHUB_REPO}"
echo
echo "Then push to main (or run the 'Deploy to Cloud Run' workflow manually)."
