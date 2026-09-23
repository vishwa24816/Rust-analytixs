# podman (no docker daemon needed)

# build
podman build -f Containerfile -t rust-analytix .

# run (persist sqlite under ./data)
mkdir -p data
podman run --rm -p 8000:8000 \
  -v ./data:/data:Z \
  -e DATABASE_URL=sqlite:/data/app.db?mode=rwc \
  -e APP_URL=http://127.0.0.1:8000 \
  --env-file .env \
  rust-analytix

# one-off migration
podman run --rm -v ./data:/data:Z -e DATABASE_URL=sqlite:/data/app.db?mode=rwc rust-analytix rust_analytix migrate

# scheduled email reports (weekly/monthly) — systemd timer, daily
podman run --rm -v ./data:/data:Z --env-file .env rust-analytix rust_analytix reports-send

# first admin
podman run --rm -v ./data:/data:Z --env-file .env rust-analytix rust_analytix make-admin you@example.com
