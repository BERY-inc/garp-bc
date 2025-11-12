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