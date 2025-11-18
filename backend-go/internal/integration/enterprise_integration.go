package integration

import (
    "context"
    "bytes"
    "crypto/tls"
    "encoding/json"
    "fmt"
    "net/http"
    "time"

    "github.com/streadway/amqp"
    "github.com/go-ldap/ldap/v3"
)

// EnterpriseIntegration provides integration with enterprise systems
type EnterpriseIntegration struct {
	httpClient *http.Client
}

// EnterpriseConfig holds enterprise integration configuration
type EnterpriseConfig struct {
	HTTPTimeout time.Duration
	TLSConfig   *tls.Config
}

// NewEnterpriseIntegration creates a new enterprise integration instance
func NewEnterpriseIntegration(config EnterpriseConfig) *EnterpriseIntegration {
	transport := &http.Transport{
		TLSClientConfig: config.TLSConfig,
	}
	
	httpClient := &http.Client{
		Timeout:   config.HTTPTimeout,
		Transport: transport,
	}
	
	return &EnterpriseIntegration{
		httpClient: httpClient,
	}
}

// ERPSystem provides integration with ERP systems
type ERPSystem struct {
	baseURL    string
	apiKey     string
	httpClient *http.Client
}

// NewERPSystem creates a new ERP system integration instance
func (ei *EnterpriseIntegration) NewERPSystem(baseURL, apiKey string) *ERPSystem {
	return &ERPSystem{
		baseURL:    baseURL,
		apiKey:     apiKey,
		httpClient: ei.httpClient,
	}
}

// ERPTransaction represents a transaction in an ERP system
type ERPTransaction struct {
	ID          string  `json:"id"`
	Amount      float64 `json:"amount"`
	Currency    string  `json:"currency"`
	Description string  `json:"description"`
	Status      string  `json:"status"`
	CreatedAt   string  `json:"created_at"`
}

// CreateERPTransaction creates a new transaction in the ERP system
func (erp *ERPSystem) CreateERPTransaction(ctx context.Context, transaction ERPTransaction) error {
    url := fmt.Sprintf("%s/api/transactions", erp.baseURL)
    
    // Marshal the transaction to JSON
    data, err := json.Marshal(transaction)
    if err != nil {
        return fmt.Errorf("failed to marshal transaction: %w", err)
    }
    
    // Create the HTTP request
    req, err := http.NewRequestWithContext(ctx, "POST", url, bytes.NewReader(data))
    if err != nil {
        return fmt.Errorf("failed to create HTTP request: %w", err)
    }
	
	// Set headers
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+erp.apiKey)
	
	// Send the request
	resp, err := erp.httpClient.Do(req)
	if err != nil {
		return fmt.Errorf("failed to send request: %w", err)
	}
	defer resp.Body.Close()
	
	// Check the response status
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return fmt.Errorf("ERP API failed with status: %d", resp.StatusCode)
	}
	
	return nil
}

// GetERPTransaction retrieves a transaction from the ERP system
func (erp *ERPSystem) GetERPTransaction(ctx context.Context, transactionID string) (*ERPTransaction, error) {
	url := fmt.Sprintf("%s/api/transactions/%s", erp.baseURL, transactionID)
	
	// Create the HTTP request
	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create HTTP request: %w", err)
	}
	
	// Set headers
	req.Header.Set("Authorization", "Bearer "+erp.apiKey)
	
	// Send the request
	resp, err := erp.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to send request: %w", err)
	}
	defer resp.Body.Close()
	
	// Check the response status
	if resp.StatusCode == 404 {
		return nil, nil // Transaction not found
	}
	
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return nil, fmt.Errorf("ERP API failed with status: %d", resp.StatusCode)
	}
	
	// Decode the response
	var transaction ERPTransaction
	if err := json.NewDecoder(resp.Body).Decode(&transaction); err != nil {
		return nil, fmt.Errorf("failed to decode response: %w", err)
	}
	
	return &transaction, nil
}

// CRMSystem provides integration with CRM systems
type CRMSystem struct {
	baseURL    string
	apiKey     string
	httpClient *http.Client
}

// NewCRMSystem creates a new CRM system integration instance
func (ei *EnterpriseIntegration) NewCRMSystem(baseURL, apiKey string) *CRMSystem {
	return &CRMSystem{
		baseURL:    baseURL,
		apiKey:     apiKey,
		httpClient: ei.httpClient,
	}
}

// CRMContact represents a contact in a CRM system
type CRMContact struct {
	ID        string `json:"id"`
	FirstName string `json:"first_name"`
	LastName  string `json:"last_name"`
	Email     string `json:"email"`
	Phone     string `json:"phone"`
	Company   string `json:"company"`
	CreatedAt string `json:"created_at"`
}

// CreateCRMContact creates a new contact in the CRM system
func (crm *CRMSystem) CreateCRMContact(ctx context.Context, contact CRMContact) error {
    url := fmt.Sprintf("%s/api/contacts", crm.baseURL)
    
    // Marshal the contact to JSON
    data, err := json.Marshal(contact)
    if err != nil {
        return fmt.Errorf("failed to marshal contact: %w", err)
    }
    
    // Create the HTTP request
    req, err := http.NewRequestWithContext(ctx, "POST", url, bytes.NewReader(data))
    if err != nil {
        return fmt.Errorf("failed to create HTTP request: %w", err)
    }
	
	// Set headers
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+crm.apiKey)
	
	// Send the request
	resp, err := crm.httpClient.Do(req)
	if err != nil {
		return fmt.Errorf("failed to send request: %w", err)
	}
	defer resp.Body.Close()
	
	// Check the response status
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return fmt.Errorf("CRM API failed with status: %d", resp.StatusCode)
	}
	
	return nil
}

// GetCRMContact retrieves a contact from the CRM system
func (crm *CRMSystem) GetCRMContact(ctx context.Context, contactID string) (*CRMContact, error) {
	url := fmt.Sprintf("%s/api/contacts/%s", crm.baseURL, contactID)
	
	// Create the HTTP request
	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create HTTP request: %w", err)
	}
	
	// Set headers
	req.Header.Set("Authorization", "Bearer "+crm.apiKey)
	
	// Send the request
	resp, err := crm.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to send request: %w", err)
	}
	defer resp.Body.Close()
	
	// Check the response status
	if resp.StatusCode == 404 {
		return nil, nil // Contact not found
	}
	
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return nil, fmt.Errorf("CRM API failed with status: %d", resp.StatusCode)
	}
	
	// Decode the response
	var contact CRMContact
	if err := json.NewDecoder(resp.Body).Decode(&contact); err != nil {
		return nil, fmt.Errorf("failed to decode response: %w", err)
	}
	
	return &contact, nil
}

// LDAPDirectory provides integration with LDAP directories
type LDAPDirectory struct {
	server   string
	port     int
	bindDN   string
	password string
}

// NewLDAPDirectory creates a new LDAP directory integration instance
func (ei *EnterpriseIntegration) NewLDAPDirectory(server string, port int, bindDN, password string) *LDAPDirectory {
	return &LDAPDirectory{
		server:   server,
		port:     port,
		bindDN:   bindDN,
		password: password,
	}
}

// LDAPUser represents a user in an LDAP directory
type LDAPUser struct {
	DN        string `json:"dn"`
	UID       string `json:"uid"`
	CN        string `json:"cn"`
	Mail      string `json:"mail"`
	FirstName string `json:"givenName"`
	LastName  string `json:"sn"`
}

// AuthenticateUser authenticates a user against the LDAP directory
func (ldapDir *LDAPDirectory) AuthenticateUser(username, password string) (*LDAPUser, error) {
	// Connect to the LDAP server
	conn, err := ldap.Dial("tcp", fmt.Sprintf("%s:%d", ldapDir.server, ldapDir.port))
	if err != nil {
		return nil, fmt.Errorf("failed to connect to LDAP server: %w", err)
	}
	defer conn.Close()
	
	// Bind with the service account
	err = conn.Bind(ldapDir.bindDN, ldapDir.password)
	if err != nil {
		return nil, fmt.Errorf("failed to bind with service account: %w", err)
	}
	
	// Search for the user
	searchRequest := ldap.NewSearchRequest(
		"dc=example,dc=com", // Base DN - should be configurable
		ldap.ScopeWholeSubtree, ldap.NeverDerefAliases, 0, 0, false,
		fmt.Sprintf("(uid=%s)", username), // Filter
		[]string{"dn", "uid", "cn", "mail", "givenName", "sn"}, // Attributes to retrieve
		nil,
	)
	
	sr, err := conn.Search(searchRequest)
	if err != nil {
		return nil, fmt.Errorf("failed to search for user: %w", err)
	}
	
	if len(sr.Entries) == 0 {
		return nil, fmt.Errorf("user not found")
	}
	
	if len(sr.Entries) > 1 {
		return nil, fmt.Errorf("multiple users found")
	}
	
	// Get the user's DN
	userDN := sr.Entries[0].DN
	
	// Try to bind as the user to verify password
	err = conn.Bind(userDN, password)
	if err != nil {
		return nil, fmt.Errorf("invalid credentials")
	}
	
	// Create the user object
	user := &LDAPUser{
		DN:        userDN,
		UID:       sr.Entries[0].GetAttributeValue("uid"),
		CN:        sr.Entries[0].GetAttributeValue("cn"),
		Mail:      sr.Entries[0].GetAttributeValue("mail"),
		FirstName: sr.Entries[0].GetAttributeValue("givenName"),
		LastName:  sr.Entries[0].GetAttributeValue("sn"),
	}
	
	return user, nil
}

// RabbitMQIntegration provides integration with RabbitMQ
type RabbitMQIntegration struct {
	connection *amqp.Connection
	channel    *amqp.Channel
}

// NewRabbitMQIntegration creates a new RabbitMQ integration instance
func (ei *EnterpriseIntegration) NewRabbitMQIntegration(amqpURI string) (*RabbitMQIntegration, error) {
	// Connect to RabbitMQ
	conn, err := amqp.Dial(amqpURI)
	if err != nil {
		return nil, fmt.Errorf("failed to connect to RabbitMQ: %w", err)
	}
	
	// Open a channel
	ch, err := conn.Channel()
	if err != nil {
		conn.Close()
		return nil, fmt.Errorf("failed to open channel: %w", err)
	}
	
	return &RabbitMQIntegration{
		connection: conn,
		channel:    ch,
	}, nil
}

// Close closes the RabbitMQ connection
func (rmq *RabbitMQIntegration) Close() error {
	if err := rmq.channel.Close(); err != nil {
		return fmt.Errorf("failed to close channel: %w", err)
	}
	
	if err := rmq.connection.Close(); err != nil {
		return fmt.Errorf("failed to close connection: %w", err)
	}
	
	return nil
}

// PublishMessage publishes a message to a RabbitMQ exchange
func (rmq *RabbitMQIntegration) PublishMessage(exchange, routingKey string, message []byte) error {
	// Publish the message
	err := rmq.channel.Publish(
		exchange,   // exchange
		routingKey, // routing key
		false,      // mandatory
		false,      // immediate
		amqp.Publishing{
			ContentType: "application/json",
			Body:        message,
		})
	
	if err != nil {
		return fmt.Errorf("failed to publish message: %w", err)
	}
	
	return nil
}

// ConsumeMessages consumes messages from a RabbitMQ queue
func (rmq *RabbitMQIntegration) ConsumeMessages(queueName string, handler func([]byte) error) error {
	// Declare the queue
	q, err := rmq.channel.QueueDeclare(
		queueName, // name
		true,      // durable
		false,     // delete when unused
		false,     // exclusive
		false,     // no-wait
		nil,       // arguments
	)
	if err != nil {
		return fmt.Errorf("failed to declare queue: %w", err)
	}
	
	// Start consuming messages
	msgs, err := rmq.channel.Consume(
		q.Name, // queue
		"",     // consumer
		true,   // auto-ack
		false,  // exclusive
		false,  // no-local
		false,  // no-wait
		nil,    // args
	)
	if err != nil {
		return fmt.Errorf("failed to register consumer: %w", err)
	}
	
	// Process messages
	forever := make(chan bool)
	go func() {
		for d := range msgs {
			if err := handler(d.Body); err != nil {
				// Log the error but continue processing
				fmt.Printf("Error processing message: %v\n", err)
			}
		}
	}()
	
	// Wait forever
	<-forever
	
	return nil
}

// BlockchainToEnterpriseEvent represents an event for enterprise system integration
type BlockchainToEnterpriseEvent struct {
	ID          string      `json:"id"`
	EventType   string      `json:"event_type"`
	Timestamp   time.Time   `json:"timestamp"`
	Blockchain  string      `json:"blockchain"`
	Transaction string      `json:"transaction"`
	Data        interface{} `json:"data"`
}

// SendEventToEnterprise sends a blockchain event to an enterprise system
func (ei *EnterpriseIntegration) SendEventToEnterprise(ctx context.Context, url string, event BlockchainToEnterpriseEvent) error {
    // Marshal the event to JSON
    data, err := json.Marshal(event)
    if err != nil {
        return fmt.Errorf("failed to marshal event: %w", err)
    }
    
    // Create the HTTP request
    req, err := http.NewRequestWithContext(ctx, "POST", url, bytes.NewReader(data))
    if err != nil {
        return fmt.Errorf("failed to create HTTP request: %w", err)
    }
	
	// Set headers
	req.Header.Set("Content-Type", "application/json")
	
	// Send the request
	resp, err := ei.httpClient.Do(req)
	if err != nil {
		return fmt.Errorf("failed to send request: %w", err)
	}
	defer resp.Body.Close()
	
	// Check the response status
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return fmt.Errorf("enterprise system API failed with status: %d", resp.StatusCode)
	}
	
	return nil
}