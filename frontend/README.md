# GARP Blockchain Frontend

This is the web-based frontend interface for the GARP Blockchain Platform. It provides a comprehensive GUI for interacting with the blockchain network.

## Features

### Dashboard
- Network status monitoring
- Latest block information
- Account balance checking
- Network statistics

### Transactions
- Submit standard transfers
- View recent transactions
- Transaction history tracking

### Accounts
- Account information lookup
- Balance checking
- Transaction history by account

### Cross-Chain Bridge
- Transfer assets between supported blockchains
- Bridge transaction monitoring
- Asset mapping management

### Smart Contracts
- Deploy new contracts
- Interact with existing contracts
- List deployed contracts

### Blocks
- View specific block information
- Latest block details
- Recent blocks list

### Chat
- End-to-end encrypted messaging demo
- Conversation history

## Development

### Prerequisites
- Docker and Docker Compose
- Node.js (for development)

### Running with Docker
```bash
# Start all services including the frontend
docker-compose up -d

# The frontend will be available at http://localhost
```

### Development Server
For development, you can run a local development server:

```bash
# Navigate to the frontend directory
cd frontend

# Serve the files locally (requires a static file server)
npx serve
```

## API Integration

The frontend communicates with several backend services:

1. **Participant Node** (`/node`): Core blockchain operations
2. **Backend Service** (`/api`): Account and transaction management
3. **Global Synchronizer** (`/sync`): Cross-domain coordination
4. **Bridge Service** (`/bridge`): Cross-chain functionality

## Technologies Used

- HTML5
- CSS3 (with Flexbox and Grid)
- Vanilla JavaScript (ES6+)
- Nginx (for serving and proxying)

## Responsive Design

The frontend is fully responsive and works on:
- Desktop computers
- Tablets
- Mobile devices

## Customization

To customize the frontend:
1. Modify `index.html` for structure changes
2. Update `styles.css` for styling
3. Enhance `app.js` for additional functionality

## License

This project is part of the GARP Blockchain Platform and is licensed under the MIT License.