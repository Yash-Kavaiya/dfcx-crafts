# Deploying to Google Cloud Run (CI/CD)

Every push to `main` runs `.github/workflows/deploy-cloud-run.yml`:

1. **test** – `cargo test --workspace` and `cxa test` for every example agent.
2. **deploy** – builds the container, pushes it to Artifact Registry,
   deploys it to Cloud Run, and smoke-tests the live service. The smoke test
   checks `/healthz`, `/v3/serverInfo`, the console, `/messenger.js` and a
   `detectIntent` against the seeded demo agent.

GitHub authenticates to Google Cloud with **Workload Identity Federation**.
No service-account keys are created or stored. The deploy job is skipped
until the repository variables below exist, so the pipeline stays green
before setup.

```
push to main ──► test ──► docker build ──► Artifact Registry ──► Cloud Run ──► smoke test
                         (WIF auth: GitHub OIDC → cxa-deployer service account)
```

## One-time setup (about 5 minutes)

1. Open [Cloud Shell](https://shell.cloud.google.com) in the target project
   and clone the repository.
2. Run the setup script:

   ```bash
   PROJECT_ID=my-project REGION=us-central1 ./deploy/gcp-setup.sh
   ```

   The script is idempotent: re-running it skips anything that already
   exists. It does the following:

   | What | Why |
   |---|---|
   | Enables the Run, Artifact Registry, IAM, STS, Secret Manager and Storage APIs | Required services |
   | Creates the Artifact Registry Docker repository `cxa` | Image storage |
   | Creates the `cxa-deployer` service account, with `run.admin`, `artifactregistry.writer` on the repo, and `serviceAccountUser` on the runtime SA only | Used by GitHub Actions |
   | Creates the `cxa-runtime` service account, with `storage.objectUser` on the data bucket and `secretAccessor` on the API key | Identity of the running service |
   | Creates the bucket `gs://<project>-cxa-data` | Persists the platform state, mounted at `/data` |
   | Creates the secret `cxa-api-key` with a random key, and prints it once | Protects the management API |
   | Creates the Workload Identity pool `github` and OIDC provider `github-actions`, restricted to this repository | Keyless GitHub → GCP authentication |

3. Add the repository variables the script prints, under GitHub →
   Settings → Secrets and variables → Actions → **Variables**. Or paste the
   printed `gh variable set …` commands. None of them are secrets.

   | Variable | Example |
   |---|---|
   | `GCP_PROJECT_ID` | `my-project` |
   | `GCP_REGION` | `us-central1` |
   | `GCP_WORKLOAD_IDENTITY_PROVIDER` | `projects/123/locations/global/workloadIdentityPools/github/providers/github-actions` |
   | `GCP_DEPLOY_SERVICE_ACCOUNT` | `cxa-deployer@my-project.iam.gserviceaccount.com` |
   | `GCP_RUNTIME_SERVICE_ACCOUNT` | `cxa-runtime@my-project.iam.gserviceaccount.com` |
   | `GCS_BUCKET` | `my-project-cxa-data` |
   | `API_KEY_SECRET` | `cxa-api-key` |

4. Push to `main`, or run **Actions → Deploy to Cloud Run → Run workflow**.
   The job summary shows the console URL.

## Optional variables

| Variable | Default | Effect |
|---|---|---|
| `CLOUD_RUN_SERVICE` | `cxa` | Service name |
| `ARTIFACT_REPOSITORY` | `cxa` | Artifact Registry repository |
| `PUBLIC_SESSIONS` | `1` | `1` lets `:detectIntent` / `:matchIntent` run without the API key, so the `<cx-messenger>` widget can chat. The management API always needs the key. |
| `CXA_SEED` | the three example agents | Comma-separated agent files baked into the image (`/app/examples/...`), created in project `demo` if missing. Set it to a space to seed nothing. |
| `CXA_LLM_URL`, `CXA_LLM_MODEL` | unset | OpenAI-compatible endpoint for playbooks and generators |

## Using the deployed platform

- **Console:** `https://<service-url>/projects`. It asks for the API key once
  and keeps it in that browser. "Sign out" forgets it.
- **API:** send `Authorization: Bearer <key>`, `X-Api-Key: <key>` or `?key=<key>`.
  ```bash
  curl -H "Authorization: Bearer $KEY" https://<service-url>/v3/projects
  ```
- **Widget:** embed `<script src="https://<service-url>/messenger.js">` and a
  `<cx-messenger agent="…">` element. The console's Integrations page
  generates the snippet.

## Runtime configuration (container environment)

| Variable | Meaning |
|---|---|
| `PORT` | Listen port, set by Cloud Run. The server binds `0.0.0.0:$PORT`. |
| `CXA_DATA` | State file (default in the image: `/data/cxa-state.json`) |
| `CXA_MEMORY=1` | Do not persist anything |
| `CXA_API_KEY` | Management API key (from Secret Manager in the pipeline) |
| `CXA_PUBLIC_SESSIONS=1` | Allow session calls without the key |
| `CXA_SEED` | Agents to create at start-up |
| `CXA_LOG=0` | Silence request logs |

## Design notes and limits

- **Single instance.** The platform state is one JSON document written
  atomically on each change, so the service runs with `--max-instances 1`.
  That suits a team console and moderate chat traffic. To scale out
  horizontally, the store would move to Firestore or Cloud SQL; the `Store`
  type is the only component to replace.
- **Persistence.** With `GCS_BUCKET` set, `/data` is a Cloud Storage volume
  mount (second-generation execution environment), so agents survive
  restarts and redeploys. Without it, data lives only as long as the
  instance.
- **Scale to zero.** `--min-instances 0` keeps cost near zero when idle. Set
  it to 1 for faster first responses.
- **Image.** A multi-stage build: `rust:slim` to compile, then
  `gcr.io/distroless/cc-debian12:nonroot` at runtime. The result is a few MB,
  with no shell, running as uid 65532.

## Local container

```bash
docker build -t cxa .
docker run -p 8080:8080 -e CXA_SEED=/app/examples/pizza-agent.json cxa
# open http://localhost:8080/projects
```
