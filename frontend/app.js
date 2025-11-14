// API Configuration
const API_BASE_URL = '/api'; // This will be proxied to your backend
const NODE_URL = '/node';     // This will be proxied to your participant node
const BRIDGE_URL = '/bridge'; // Bridge API endpoint

// Currency Metadata
const CURRENCY_NAME = 'BERY';
const CURRENCY_SYMBOL = 'BRY';

// DOM Elements
const checkHealthBtn = document.getElementById('checkHealth');
const statusMessage = document.getElementById('statusMessage');
const blockchainInfo = document.getElementById('blockchainInfo');
const transactionForm = document.getElementById('transactionForm');
const transactionResult = document.getElementById('transactionResult');
const getAccountBtn = document.getElementById('getAccount');
const accountInfo = document.getElementById('accountInfo');
const sendMessageBtn = document.getElementById('sendMessage');
const loadConversationBtn = document.getElementById('loadConversation');
const chatResult = document.getElementById('chatResult');
const chatMessages = document.getElementById('chatMessages');

// New DOM Elements
const dashboardSection = document.getElementById('dashboard');
const transactionsSection = document.getElementById('transactions');
const accountsSection = document.getElementById('accounts');
const bridgeSection = document.getElementById('bridge');
const contractsSection = document.getElementById('contracts');
const blocksSection = document.getElementById('blocks');
const chatSection = document.getElementById('chat');

// Dashboard Elements
const networkStatus = document.getElementById('networkStatus');
const latestBlockInfo = document.getElementById('latestBlockInfo');
const getDashboardBalanceBtn = document.getElementById('getDashboardBalance');
const accountBalanceInfo = document.getElementById('accountBalanceInfo');
const networkStats = document.getElementById('networkStats');

// Bridge Elements
const initiateBridgeBtn = document.getElementById('initiateBridge');
const bridgeResult = document.getElementById('bridgeResult');
const bridgeTransactions = document.getElementById('bridgeTransactions');

// Contract Elements
const deployContractBtn = document.getElementById('deployContractBtn');
const callContractBtn = document.getElementById('callContractBtn');
const listContractsBtn = document.getElementById('listContractsBtn');
const deployResult = document.getElementById('deployResult');
const callResult = document.getElementById('callResult');
const contractsList = document.getElementById('contractsList');

// Block Elements
const getBlockBtn = document.getElementById('getBlock');
const getLatestBlockBtn = document.getElementById('getLatestBlock');
const blockInfo = document.getElementById('blockInfo');
const recentBlocks = document.getElementById('recentBlocks');

// Tab Elements
const tabButtons = document.querySelectorAll('.tab-button');

// Navigation
document.addEventListener('DOMContentLoaded', function() {
    // Show dashboard by default
    showSection('dashboard');
    
    // Add event listeners to navigation links
    document.querySelectorAll('nav a').forEach(link => {
        link.addEventListener('click', function(e) {
            e.preventDefault();
            const sectionId = this.getAttribute('href').substring(1);
            showSection(sectionId);
        });
    });
    
    // Add event listeners to tab buttons
    tabButtons.forEach(button => {
        button.addEventListener('click', function() {
            const tabId = this.getAttribute('data-tab');
            activateTab(tabId);
        });
    });
    
    // Initialize dashboard
    setTimeout(checkBlockchainHealth, 1000);
    loadNetworkStats();
    loadRecentBlocks();
    loadRecentTransactions();
});

// Show specific section and hide others
function showSection(sectionId) {
    // Hide all sections
    document.querySelectorAll('.content-section').forEach(section => {
        section.classList.remove('active');
    });
    
    // Show requested section
    document.getElementById(sectionId).classList.add('active');
    
    // Update active nav link
    document.querySelectorAll('nav a').forEach(link => {
        link.classList.remove('active');
        if (link.getAttribute('href') === `#${sectionId}`) {
            link.classList.add('active');
        }
    });
}

// Activate tab
function activateTab(tabId) {
    // Deactivate all tabs
    document.querySelectorAll('.tab-button').forEach(button => {
        button.classList.remove('active');
    });
    
    document.querySelectorAll('.tab-content').forEach(content => {
        content.classList.remove('active');
    });
    
    // Activate selected tab
    document.querySelector(`.tab-button[data-tab="${tabId}"]`).classList.add('active');
    document.getElementById(tabId).classList.add('active');
}

// Check Blockchain Health
async function checkBlockchainHealth() {
    try {
        showStatus('Checking blockchain status...', 'info');
        
        const response = await fetch(`${API_BASE_URL}/health`);
        const data = await response.json();
        
        if (response.ok) {
            showStatus('Blockchain is running and healthy!', 'ok');
            loadDashboardInfo();
        } else {
            showStatus('Blockchain is not healthy', 'error');
        }
    } catch (error) {
        console.error('Error checking blockchain health:', error);
        showStatus('Error connecting to blockchain', 'error');
    }
}

// Load Dashboard Information
async function loadDashboardInfo() {
    loadNetworkStatus();
    loadLatestBlock();
}

// Load Network Status
async function loadNetworkStatus() {
    try {
        const response = await fetch(`${NODE_URL}/api/v1/node/status`);
        const status = await response.json();
        
        if (response.ok && status.data) {
            networkStatus.innerHTML = `
                <p><strong>Status:</strong> <span class="status-ok">Online</span></p>
                <p><strong>Participant ID:</strong> ${status.data.participant_id || 'N/A'}</p>
                <p><strong>Connected Peers:</strong> ${status.data.connected_peers || 0}</p>
                <p><strong>Sync Status:</strong> ${status.data.in_sync ? 'In Sync' : 'Syncing'}</p>
            `;
        } else {
            networkStatus.innerHTML = '<p>Unable to load network status</p>';
        }
    } catch (error) {
        console.error('Error loading network status:', error);
        networkStatus.innerHTML = '<p>Error loading network status</p>';
    }
}

// Load Latest Block
async function loadLatestBlock() {
    try {
        const response = await fetch(`${NODE_URL}/api/v1/blocks/latest`);
        const block = await response.json();
        
        if (response.ok && block.data) {
            latestBlockInfo.innerHTML = `
                <p><strong>Block Number:</strong> ${block.data.number || 'N/A'}</p>
                <p><strong>Hash:</strong> ${block.data.hash ? block.data.hash.substring(0, 16) + '...' : 'N/A'}</p>
                <p><strong>Timestamp:</strong> ${block.data.timestamp ? new Date(block.data.timestamp).toLocaleString() : 'N/A'}</p>
                <p><strong>Transactions:</strong> ${block.data.transactions?.length || 0}</p>
            `;
        } else {
            latestBlockInfo.innerHTML = '<p>Unable to load latest block</p>';
        }
    } catch (error) {
        console.error('Error loading latest block:', error);
        latestBlockInfo.innerHTML = '<p>Error loading latest block</p>';
    }
}

// Load Network Stats
async function loadNetworkStats() {
    try {
        // Simulate network stats
        networkStats.innerHTML = `
            <p><strong>TPS:</strong> 1,250</p>
            <p><strong>Block Time:</strong> 1.2s</p>
            <p><strong>Total Transactions:</strong> 1,245,678</p>
            <p><strong>Active Nodes:</strong> 42</p>
        `;
    } catch (error) {
        console.error('Error loading network stats:', error);
        networkStats.innerHTML = '<p>Error loading network stats</p>';
    }
}

// Get Dashboard Balance
async function getDashboardBalance() {
    const accountAddress = document.getElementById('dashboardAccountAddress').value;
    
    if (!accountAddress) {
        accountBalanceInfo.innerHTML = '<p class="status-error">Please enter an account address</p>';
        return;
    }
    
    try {
        accountBalanceInfo.innerHTML = '<p>Loading balance...</p>';
        
        // Simulate balance fetch
        setTimeout(() => {
            accountBalanceInfo.innerHTML = `
                <p><strong>Address:</strong> ${accountAddress.substring(0, 16)}...</p>
                <p><strong>Balance:</strong> 1,250.50 ${CURRENCY_SYMBOL}</p>
                <p><strong>Status:</strong> <span class="status-ok">Active</span></p>
            `;
        }, 500);
    } catch (error) {
        console.error('Error getting balance:', error);
        accountBalanceInfo.innerHTML = `<p class="status-error">Error: ${error.message || 'Network error'}</p>`;
    }
}

// Chat: Send Message
async function sendChatMessage() {
    const self = document.getElementById('chatSelf').value;
    const peer = document.getElementById('chatPeer').value;
    const ciphertext = document.getElementById('chatCiphertext').value;
    const nonce = document.getElementById('chatNonce').value;
    if (!self || !peer || !ciphertext || !nonce) {
        chatResult.innerHTML = '<p class="status-error">Please fill all chat fields</p>';
        return;
    }
    try {
        chatResult.innerHTML = '<p>Sending...</p>';
        const res = await fetch(`${API_BASE_URL}/messages`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ sender: self, recipient: peer, content_ciphertext: ciphertext, content_nonce: nonce })
        });
        const data = await res.json();
        if (res.ok) {
            chatResult.innerHTML = `<div class="status-ok">Message sent. ID: ${data.id}, hash: ${data.hash}</div>`;
        } else {
            chatResult.innerHTML = `<div class="status-error">Error: ${data.error || 'Unknown error'}</div>`;
        }
    } catch (e) {
        chatResult.innerHTML = `<div class="status-error">Error: ${e.message}</div>`;
    }
}

// Chat: Load Conversation
async function loadChatMessages() {
    const self = document.getElementById('chatSelf').value;
    const peer = document.getElementById('chatPeer').value;
    if (!self || !peer) {
        chatMessages.innerHTML = '<p class="status-error">Please enter your address and peer.</p>';
        return;
    }
    try {
        chatMessages.innerHTML = '<p>Loading messages...</p>';
        const res = await fetch(`${API_BASE_URL}/messages?address=${encodeURIComponent(self)}&peer=${encodeURIComponent(peer)}&limit=50`);
        const data = await res.json();
        if (res.ok && Array.isArray(data)) {
            chatMessages.innerHTML = data.map(m => `
                <div class="message ${m.sender === self ? 'own' : ''}">
                    <p><strong>${m.sender === self ? 'You' : 'Peer'}:</strong> [ciphertext ${m.content_ciphertext?.length || 0} bytes]</p>
                    <p><small>${new Date(m.created_at).toLocaleString()} | hash ${m.hash.slice(0,8)}…</small></p>
                </div>
            `).join('');
        } else {
            chatMessages.innerHTML = `<div class="status-error">Error loading messages: ${data.error || 'Unknown error'}</div>`;
        }
    } catch (e) {
        chatMessages.innerHTML = `<div class="status-error">Error: ${e.message}</div>`;
    }
}

// Submit Transaction
async function submitTransaction(event) {
    event.preventDefault();
    
    const fromAddress = document.getElementById('fromAddress').value;
    const toAddress = document.getElementById('toAddress').value;
    const amount = document.getElementById('amount').value;
    const transactionType = document.getElementById('transactionType').value;
    
    try {
        transactionResult.innerHTML = '<p>Submitting transaction...</p>';
        
        // Simulate transaction submission
        setTimeout(() => {
            transactionResult.innerHTML = `
                <div class="status-ok">
                    <p><strong>Transaction Submitted Successfully!</strong></p>
                    <p>Transaction ID: 0x${Math.random().toString(16).substr(2, 64)}</p>
                    <p>Amount: ${amount} ${CURRENCY_SYMBOL}</p>
                    <p>From: ${fromAddress.substring(0, 16)}...</p>
                    <p>To: ${toAddress.substring(0, 16)}...</p>
                    <p>Status: <span class="status-info">Pending</span></p>
                </div>
            `;
        }, 1000);
    } catch (error) {
        console.error('Error submitting transaction:', error);
        transactionResult.innerHTML = `
            <div class="status-error">
                <p><strong>Transaction Failed</strong></p>
                <p>Error: ${error.message || 'Network error'}</p>
            </div>
        `;
    }
}

// Get Account Information
async function getAccountInfo() {
    const accountAddress = document.getElementById('accountAddress').value;
    
    if (!accountAddress) {
        accountInfo.innerHTML = '<p class="status-error">Please enter an account address</p>';
        return;
    }
    
    try {
        accountInfo.innerHTML = '<p>Loading account information...</p>';
        
        // Simulate account info fetch
        setTimeout(() => {
            accountInfo.innerHTML = `
                <h3>Account Information</h3>
                <p><strong>Address:</strong> ${accountAddress}</p>
                <p><strong>Balance:</strong> 1,250.50 ${CURRENCY_SYMBOL}</p>
                <p><strong>Status:</strong> <span class="status-ok">Active</span></p>
                <p><strong>Nonce:</strong> 42</p>
                <p><strong>Created:</strong> 2025-01-15</p>
            `;
            
            // Load account transactions
            loadAccountTransactions(accountAddress);
        }, 500);
    } catch (error) {
        console.error('Error getting account info:', error);
        accountInfo.innerHTML = `
            <div class="status-error">
                <p>Error loading account information: ${error.message || 'Network error'}</p>
            </div>
        `;
    }
}

// Load Account Transactions
async function loadAccountTransactions(accountAddress) {
    try {
        document.getElementById('accountTransactions').innerHTML = '<p>Loading transactions...</p>';
        
        // Simulate transactions fetch
        setTimeout(() => {
            document.getElementById('accountTransactions').innerHTML = `
                <table>
                    <thead>
                        <tr>
                            <th>Transaction ID</th>
                            <th>Type</th>
                            <th>Amount</th>
                            <th>Status</th>
                            <th>Date</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr>
                            <td>0x1a2b3c...</td>
                            <td>Transfer</td>
                            <td>125.00 ${CURRENCY_SYMBOL}</td>
                            <td><span class="status-ok">Confirmed</span></td>
                            <td>2025-01-20 14:30</td>
                        </tr>
                        <tr>
                            <td>0x4d5e6f...</td>
                            <td>Transfer</td>
                            <td>50.00 ${CURRENCY_SYMBOL}</td>
                            <td><span class="status-ok">Confirmed</span></td>
                            <td>2025-01-19 10:15</td>
                        </tr>
                        <tr>
                            <td>0x7g8h9i...</td>
                            <td>Contract</td>
                            <td>0.00 ${CURRENCY_SYMBOL}</td>
                            <td><span class="status-info">Pending</span></td>
                            <td>2025-01-18 16:45</td>
                        </tr>
                    </tbody>
                </table>
            `;
        }, 500);
    } catch (error) {
        console.error('Error loading account transactions:', error);
        document.getElementById('accountTransactions').innerHTML = `<p class="status-error">Error loading transactions: ${error.message || 'Network error'}</p>`;
    }
}

// Initiate Bridge Transfer
async function initiateBridgeTransfer() {
    const sourceChain = document.getElementById('sourceChain').value;
    const targetChain = document.getElementById('targetChain').value;
    const bridgeAsset = document.getElementById('bridgeAsset').value;
    const bridgeAmount = document.getElementById('bridgeAmount').value;
    const sourceAddress = document.getElementById('sourceAddress').value;
    const targetAddress = document.getElementById('targetAddress').value;
    
    if (!sourceAddress || !targetAddress || !bridgeAmount) {
        bridgeResult.innerHTML = '<p class="status-error">Please fill all required fields</p>';
        return;
    }
    
    try {
        bridgeResult.innerHTML = '<p>Initiating bridge transfer...</p>';
        
        // Simulate bridge transfer
        setTimeout(() => {
            const bridgeTxId = 'bridge_' + Math.random().toString(16).substr(2, 32);
            bridgeResult.innerHTML = `
                <div class="status-ok">
                    <p><strong>Bridge Transfer Initiated Successfully!</strong></p>
                    <p>Bridge Transaction ID: ${bridgeTxId}</p>
                    <p>Amount: ${bridgeAmount} ${bridgeAsset}</p>
                    <p>From: ${sourceChain} (${sourceAddress.substring(0, 16)}...)</p>
                    <p>To: ${targetChain} (${targetAddress.substring(0, 16)}...)</p>
                    <p>Status: <span class="status-info">Processing</span></p>
                </div>
            `;
            
            // Add to bridge transactions list
            addBridgeTransaction(bridgeTxId, sourceChain, targetChain, bridgeAsset, bridgeAmount, sourceAddress, targetAddress);
        }, 1500);
    } catch (error) {
        console.error('Error initiating bridge transfer:', error);
        bridgeResult.innerHTML = `
            <div class="status-error">
                <p><strong>Bridge Transfer Failed</strong></p>
                <p>Error: ${error.message || 'Network error'}</p>
            </div>
        `;
    }
}

// Add Bridge Transaction to List
function addBridgeTransaction(txId, sourceChain, targetChain, asset, amount, sourceAddress, targetAddress) {
    const transactionElement = document.createElement('div');
    transactionElement.className = 'message';
    transactionElement.innerHTML = `
        <p><strong>Bridge Transaction:</strong> ${txId}</p>
        <p>${amount} ${asset} from ${sourceChain} to ${targetChain}</p>
        <p>Source: ${sourceAddress.substring(0, 16)}...</p>
        <p>Target: ${targetAddress.substring(0, 16)}...</p>
        <p>Status: <span class="status-info">Processing</span></p>
        <p><small>${new Date().toLocaleString()}</small></p>
    `;
    
    bridgeTransactions.prepend(transactionElement);
}

// Deploy Contract
async function deployContract() {
    const contractCode = document.getElementById('contractCode').value;
    const contractArguments = document.getElementById('contractArguments').value;
    
    if (!contractCode) {
        deployResult.innerHTML = '<p class="status-error">Please enter contract code</p>';
        return;
    }
    
    try {
        deployResult.innerHTML = '<p>Deploying contract...</p>';
        
        // Simulate contract deployment
        setTimeout(() => {
            const contractAddress = '0x' + Math.random().toString(16).substr(2, 40);
            deployResult.innerHTML = `
                <div class="status-ok">
                    <p><strong>Contract Deployed Successfully!</strong></p>
                    <p>Contract Address: ${contractAddress}</p>
                    <p>Status: <span class="status-ok">Deployed</span></p>
                    <p><small>Transaction ID: 0x${Math.random().toString(16).substr(2, 64)}</small></p>
                </div>
            `;
        }, 2000);
    } catch (error) {
        console.error('Error deploying contract:', error);
        deployResult.innerHTML = `
            <div class="status-error">
                <p><strong>Contract Deployment Failed</strong></p>
                <p>Error: ${error.message || 'Network error'}</p>
            </div>
        `;
    }
}

// Call Contract Function
async function callContractFunction() {
    const contractAddress = document.getElementById('contractAddress').value;
    const contractFunction = document.getElementById('contractFunction').value;
    const functionArguments = document.getElementById('functionArguments').value;
    
    if (!contractAddress || !contractFunction) {
        callResult.innerHTML = '<p class="status-error">Please enter contract address and function name</p>';
        return;
    }
    
    try {
        callResult.innerHTML = '<p>Calling contract function...</p>';
        
        // Simulate contract call
        setTimeout(() => {
            callResult.innerHTML = `
                <div class="status-ok">
                    <p><strong>Function Called Successfully!</strong></p>
                    <p>Contract: ${contractAddress.substring(0, 16)}...</p>
                    <p>Function: ${contractFunction}</p>
                    <p>Status: <span class="status-ok">Executed</span></p>
                    <p>Result: "Success"</p>
                </div>
            `;
        }, 1500);
    } catch (error) {
        console.error('Error calling contract function:', error);
        callResult.innerHTML = `
            <div class="status-error">
                <p><strong>Function Call Failed</strong></p>
                <p>Error: ${error.message || 'Network error'}</p>
            </div>
        `;
    }
}

// List Contracts
async function listContracts() {
    const ownerAddress = document.getElementById('ownerAddress').value;
    
    if (!ownerAddress) {
        contractsList.innerHTML = '<p class="status-error">Please enter owner address</p>';
        return;
    }
    
    try {
        contractsList.innerHTML = '<p>Loading contracts...</p>';
        
        // Simulate contracts list
        setTimeout(() => {
            contractsList.innerHTML = `
                <table>
                    <thead>
                        <tr>
                            <th>Contract Address</th>
                            <th>Name</th>
                            <th>Status</th>
                            <th>Created</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr>
                            <td>0x1a2b3c4d5e...</td>
                            <td>TokenContract</td>
                            <td><span class="status-ok">Active</span></td>
                            <td>2025-01-15</td>
                        </tr>
                        <tr>
                            <td>0xf0e1d2c3b4...</td>
                            <td>VotingSystem</td>
                            <td><span class="status-ok">Active</span></td>
                            <td>2025-01-10</td>
                        </tr>
                        <tr>
                            <td>0xa1b2c3d4e5...</td>
                            <td>LendingProtocol</td>
                            <td><span class="status-warning">Paused</span></td>
                            <td>2025-01-05</td>
                        </tr>
                    </tbody>
                </table>
            `;
        }, 1000);
    } catch (error) {
        console.error('Error listing contracts:', error);
        contractsList.innerHTML = `<p class="status-error">Error loading contracts: ${error.message || 'Network error'}</p>`;
    }
}

// Get Block Information
async function getBlock() {
    const blockNumber = document.getElementById('blockNumber').value;
    
    if (!blockNumber) {
        blockInfo.innerHTML = '<p class="status-error">Please enter a block number</p>';
        return;
    }
    
    try {
        blockInfo.innerHTML = '<p>Loading block information...</p>';
        
        // Simulate block fetch
        setTimeout(() => {
            blockInfo.innerHTML = `
                <h3>Block #${blockNumber}</h3>
                <p><strong>Hash:</strong> 0x${Math.random().toString(16).substr(2, 64)}</p>
                <p><strong>Previous Hash:</strong> 0x${Math.random().toString(16).substr(2, 64)}</p>
                <p><strong>Timestamp:</strong> ${new Date().toLocaleString()}</p>
                <p><strong>Transactions:</strong> 12</p>
                <p><strong>Size:</strong> 1,245 bytes</p>
                <p><strong>Miner:</strong> 0x${Math.random().toString(16).substr(2, 40)}</p>
            `;
        }, 500);
    } catch (error) {
        console.error('Error getting block info:', error);
        blockInfo.innerHTML = `<p class="status-error">Error loading block information: ${error.message || 'Network error'}</p>`;
    }
}

// Get Latest Block
async function getLatestBlock() {
    document.getElementById('blockNumber').value = '';
    blockInfo.innerHTML = '<p>Loading latest block...</p>';
    
    // Simulate latest block fetch
    setTimeout(() => {
        const blockNumber = Math.floor(Math.random() * 1000000);
        blockInfo.innerHTML = `
            <h3>Latest Block #${blockNumber}</h3>
            <p><strong>Hash:</strong> 0x${Math.random().toString(16).substr(2, 64)}</p>
            <p><strong>Previous Hash:</strong> 0x${Math.random().toString(16).substr(2, 64)}</p>
            <p><strong>Timestamp:</strong> ${new Date().toLocaleString()}</p>
            <p><strong>Transactions:</strong> ${Math.floor(Math.random() * 50)}</p>
            <p><strong>Size:</strong> ${Math.floor(Math.random() * 2000)} bytes</p>
            <p><strong>Miner:</strong> 0x${Math.random().toString(16).substr(2, 40)}</p>
        `;
    }, 500);
}

// Load Recent Blocks
async function loadRecentBlocks() {
    try {
        recentBlocks.innerHTML = '<p>Loading recent blocks...</p>';
        
        // Simulate recent blocks
        setTimeout(() => {
            recentBlocks.innerHTML = `
                <table>
                    <thead>
                        <tr>
                            <th>Block #</th>
                            <th>Hash</th>
                            <th>Transactions</th>
                            <th>Timestamp</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr>
                            <td>1,245,678</td>
                            <td>0x1a2b...c3d4</td>
                            <td>12</td>
                            <td>${new Date(Date.now() - 10000).toLocaleTimeString()}</td>
                        </tr>
                        <tr>
                            <td>1,245,677</td>
                            <td>0x5e6f...7g8h</td>
                            <td>8</td>
                            <td>${new Date(Date.now() - 20000).toLocaleTimeString()}</td>
                        </tr>
                        <tr>
                            <td>1,245,676</td>
                            <td>0x9i0j...1k2l</td>
                            <td>15</td>
                            <td>${new Date(Date.now() - 30000).toLocaleTimeString()}</td>
                        </tr>
                    </tbody>
                </table>
            `;
        }, 1000);
    } catch (error) {
        console.error('Error loading recent blocks:', error);
        recentBlocks.innerHTML = `<p class="status-error">Error loading recent blocks: ${error.message || 'Network error'}</p>`;
    }
}

// Load Recent Transactions
async function loadRecentTransactions() {
    const recentTransactions = document.getElementById('recentTransactions');
    try {
        recentTransactions.innerHTML = '<p>Loading recent transactions...</p>';
        
        // Simulate recent transactions
        setTimeout(() => {
            recentTransactions.innerHTML = `
                <table>
                    <thead>
                        <tr>
                            <th>Transaction ID</th>
                            <th>Type</th>
                            <th>Amount</th>
                            <th>Status</th>
                            <th>Block</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr>
                            <td>0x1a2b...c3d4</td>
                            <td>Transfer</td>
                            <td>125.00 ${CURRENCY_SYMBOL}</td>
                            <td><span class="status-ok">Confirmed</span></td>
                            <td>1,245,678</td>
                        </tr>
                        <tr>
                            <td>0x5e6f...7g8h</td>
                            <td>Contract</td>
                            <td>0.00 ${CURRENCY_SYMBOL}</td>
                            <td><span class="status-ok">Confirmed</span></td>
                            <td>1,245,677</td>
                        </tr>
                        <tr>
                            <td>0x9i0j...1k2l</td>
                            <td>Transfer</td>
                            <td>50.00 ${CURRENCY_SYMBOL}</td>
                            <td><span class="status-info">Pending</span></td>
                            <td>N/A</td>
                        </tr>
                    </tbody>
                </table>
            `;
        }, 1000);
    } catch (error) {
        console.error('Error loading recent transactions:', error);
        recentTransactions.innerHTML = `<p class="status-error">Error loading recent transactions: ${error.message || 'Network error'}</p>`;
    }
}

// Show Status Message
function showStatus(message, type) {
    statusMessage.textContent = message;
    statusMessage.className = '';
    
    switch (type) {
        case 'ok':
            statusMessage.classList.add('status-ok');
            break;
        case 'error':
            statusMessage.classList.add('status-error');
            break;
        case 'warning':
            statusMessage.classList.add('status-warning');
            break;
        default:
            statusMessage.classList.add('status-info');
    }
}

// Event Listeners
if (checkHealthBtn) {
    checkHealthBtn.addEventListener('click', checkBlockchainHealth);
}

if (transactionForm) {
    transactionForm.addEventListener('submit', submitTransaction);
}

if (getAccountBtn) {
    getAccountBtn.addEventListener('click', getAccountInfo);
}

if (sendMessageBtn) {
    sendMessageBtn.addEventListener('click', sendChatMessage);
}

if (loadConversationBtn) {
    loadConversationBtn.addEventListener('click', loadChatMessages);
}

if (getDashboardBalanceBtn) {
    getDashboardBalanceBtn.addEventListener('click', getDashboardBalance);
}

if (initiateBridgeBtn) {
    initiateBridgeBtn.addEventListener('click', initiateBridgeTransfer);
}

if (deployContractBtn) {
    deployContractBtn.addEventListener('click', deployContract);
}

if (callContractBtn) {
    callContractBtn.addEventListener('click', callContractFunction);
}

if (listContractsBtn) {
    listContractsBtn.addEventListener('click', listContracts);
}

if (getBlockBtn) {
    getBlockBtn.addEventListener('click', getBlock);
}

if (getLatestBlockBtn) {
    getLatestBlockBtn.addEventListener('click', getLatestBlock);
}

// Initialize on page load
window.addEventListener('load', function() {
    // Check health after a short delay to allow page to load
    setTimeout(checkBlockchainHealth, 1000);
});