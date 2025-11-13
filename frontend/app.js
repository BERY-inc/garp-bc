// API Configuration
const API_BASE_URL = '/api'; // This will be proxied to your backend
const NODE_URL = '/node';     // This will be proxied to your participant node

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

// Check Blockchain Health
async function checkBlockchainHealth() {
    try {
        showStatus('Checking blockchain status...', 'info');
        
        const response = await fetch(`${API_BASE_URL}/health`);
        const data = await response.json();
        
        if (response.ok) {
            showStatus('Blockchain is running and healthy!', 'ok');
            loadBlockchainInfo();
        } else {
            showStatus('Blockchain is not healthy', 'error');
        }
    } catch (error) {
        console.error('Error checking blockchain health:', error);
        showStatus('Error connecting to blockchain', 'error');
    }
}

// Chat: Send Message
async function sendChatMessage() {
    const self = document.getElementById('chatSelf').value;
    const peer = document.getElementById('chatPeer').value;
    const ciphertext = document.getElementById('chatCiphertext').value;
    const nonce = document.getElementById('chatNonce').value;
    if (!self || !peer || !ciphertext || !nonce) {
        chatResult.innerHTML = '<p>Please fill all chat fields</p>';
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
        chatMessages.innerHTML = '<p>Please enter your address and peer.</p>';
        return;
    }
    try {
        chatMessages.innerHTML = '<p>Loading messages...</p>';
        const res = await fetch(`${API_BASE_URL}/messages?address=${encodeURIComponent(self)}&peer=${encodeURIComponent(peer)}&limit=50`);
        const data = await res.json();
        if (res.ok && Array.isArray(data)) {
            chatMessages.innerHTML = data.map(m => `
                <div class="message">
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

// Load Blockchain Information
async function loadBlockchainInfo() {
    try {
        const response = await fetch(`${API_BASE_URL}/blocks/latest`);
        const block = await response.json();
        
        if (response.ok) {
            blockchainInfo.innerHTML = `
                <h3>Latest Block Information</h3>
                <p><strong>Block Number:</strong> ${block.number || 'N/A'}</p>
                <p><strong>Timestamp:</strong> ${block.timestamp || 'N/A'}</p>
                <p><strong>Transactions:</strong> ${block.transactions?.length || 0}</p>
                <p><strong>Currency:</strong> ${CURRENCY_NAME} (${CURRENCY_SYMBOL})</p>
            `;
        } else {
            blockchainInfo.innerHTML = '<p>Unable to load blockchain information</p>';
        }
    } catch (error) {
        console.error('Error loading blockchain info:', error);
        blockchainInfo.innerHTML = '<p>Error loading blockchain information</p>';
    }
}

// Submit Transaction
async function submitTransaction(event) {
    event.preventDefault();
    
    const fromAddress = document.getElementById('fromAddress').value;
    const toAddress = document.getElementById('toAddress').value;
    const amount = document.getElementById('amount').value;
    
    try {
        transactionResult.innerHTML = '<p>Submitting transaction...</p>';
        
        const response = await fetch(`${API_BASE_URL}/transactions`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                from: fromAddress,
                to: toAddress,
                amount: parseFloat(amount)
            })
        });
        
        const result = await response.json();
        
        if (response.ok) {
            transactionResult.innerHTML = `
                <div class="status-ok">
                    <p><strong>Transaction Submitted Successfully!</strong></p>
                    <p>Transaction ID: ${result.id || 'N/A'}</p>
                </div>
            `;
        } else {
            transactionResult.innerHTML = `
                <div class="status-error">
                    <p><strong>Transaction Failed</strong></p>
                    <p>Error: ${result.error || 'Unknown error'}</p>
                </div>
            `;
        }
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
        accountInfo.innerHTML = '<p>Please enter an account address</p>';
        return;
    }
    
    try {
        accountInfo.innerHTML = '<p>Loading account information...</p>';
        
        const response = await fetch(`${API_BASE_URL}/accounts/${accountAddress}`);
        const account = await response.json();
        
        if (response.ok) {
            accountInfo.innerHTML = `
                <h3>Account Information</h3>
                <p><strong>Address:</strong> ${account.address || accountAddress}</p>
                <p><strong>Balance:</strong> ${account.balance || 0} ${CURRENCY_SYMBOL}</p>
                <p><strong>Status:</strong> ${account.status || 'Active'}</p>
            `;
        } else {
            accountInfo.innerHTML = `
                <div class="status-error">
                    <p>Account not found or error loading account information</p>
                </div>
            `;
        }
    } catch (error) {
        console.error('Error getting account info:', error);
        accountInfo.innerHTML = `
            <div class="status-error">
                <p>Error loading account information: ${error.message || 'Network error'}</p>
            </div>
        `;
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

// Initialize on page load
window.addEventListener('load', function() {
    // Check health after a short delay to allow page to load
    setTimeout(checkBlockchainHealth, 1000);
});