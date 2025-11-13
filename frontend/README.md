# GARP Blockchain Frontend

This is the frontend interface for the GARP Blockchain Platform.

## Features

- Blockchain health monitoring
- Transaction submission
- Account information lookup
- Latest block information display

## Technologies Used

- HTML5
- CSS3
- JavaScript (Vanilla JS)
- Nginx (for serving static files)

## API Integration

The frontend communicates with the blockchain backend through the following endpoints:

- `/api/health` - Health check
- `/api/accounts/{address}` - Account information
- `/api/transactions` - Transaction submission
- `/api/blocks/latest` - Latest block information

## Deployment

The frontend is deployed as a Docker container and can be run using:

```bash
docker build -t garp-frontend .
docker run -p 80:80 garp-frontend
```

Or using the docker-compose.yml file in the root directory:

```bash
docker-compose up frontend
```

## Documentation Site

- The documentation is served under the `/docs` path.
- Files are located at `frontend/docs/` (e.g., `index.html`, `styles.css`, `script.js`).
- When running via Docker or docker-compose, navigate to `http://localhost/docs/`.

### Local Preview (without Docker)

You can open `frontend/docs/index.html` directly in your browser for a quick preview, or run a simple local server:

```bash
# PowerShell (Windows)
cd frontend/docs
python -m http.server 8088
# Then open http://localhost:8088/
```

### Nginx Routing

The Nginx config includes a `/docs/` location block that serves the docs and falls back to `index.html` for nested paths:

```
location /docs/ {
  alias /usr/share/nginx/html/docs/;
  index index.html;
  try_files $uri $uri/ /docs/index.html;
}
```