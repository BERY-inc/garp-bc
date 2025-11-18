package integration

import (
    "context"
    "bytes"
    "encoding/json"
    "fmt"
    "net/http"
    "time"

	"github.com/aws/aws-sdk-go/aws"
	"github.com/aws/aws-sdk-go/aws/session"
	"github.com/aws/aws-sdk-go/service/s3"
	"github.com/aws/aws-sdk-go/service/sqs"
	"cloud.google.com/go/pubsub"
	"cloud.google.com/go/storage"
)

// CloudIntegration provides integration with cloud services
type CloudIntegration struct {
	awsSession  *session.Session
	gcpProject  string
	httpClient  *http.Client
}

// CloudConfig holds cloud integration configuration
type CloudConfig struct {
	AWSRegion    string
	GCPProjectID string
	HTTPTimeout  time.Duration
}

// NewCloudIntegration creates a new cloud integration instance
func NewCloudIntegration(config CloudConfig) (*CloudIntegration, error) {
	var awsSession *session.Session
	var err error
	
	// Initialize AWS session if region is provided
	if config.AWSRegion != "" {
		awsSession, err = session.NewSession(&aws.Config{
			Region: aws.String(config.AWSRegion),
		})
		if err != nil {
			return nil, fmt.Errorf("failed to create AWS session: %w", err)
		}
	}
	
	// Validate GCP project ID if provided
	if config.GCPProjectID == "" && config.AWSRegion == "" {
		return nil, fmt.Errorf("either AWS region or GCP project ID must be provided")
	}
	
	httpClient := &http.Client{
		Timeout: config.HTTPTimeout,
	}
	
	return &CloudIntegration{
		awsSession: awsSession,
		gcpProject: config.GCPProjectID,
		httpClient: httpClient,
	}, nil
}

// S3Storage provides AWS S3 integration
type S3Storage struct {
	client *s3.S3
}

// NewS3Storage creates a new S3 storage instance
func (ci *CloudIntegration) NewS3Storage() (*S3Storage, error) {
	if ci.awsSession == nil {
		return nil, fmt.Errorf("AWS session not initialized")
	}
	
	return &S3Storage{
		client: s3.New(ci.awsSession),
	}, nil
}

// UploadToS3 uploads data to S3
func (s3s *S3Storage) UploadToS3(ctx context.Context, bucket, key string, data []byte) error {
    reader := bytes.NewReader(data)
    _, err := s3s.client.PutObjectWithContext(ctx, &s3.PutObjectInput{
        Bucket: aws.String(bucket),
        Key:    aws.String(key),
        Body:   aws.ReadSeekCloser(reader),
    })
    return err
}

// DownloadFromS3 downloads data from S3
func (s3s *S3Storage) DownloadFromS3(ctx context.Context, bucket, key string) ([]byte, error) {
	result, err := s3s.client.GetObjectWithContext(ctx, &s3.GetObjectInput{
		Bucket: aws.String(bucket),
		Key:    aws.String(key),
	})
	if err != nil {
		return nil, err
	}
	defer result.Body.Close()
	
	// Read the data
	buf := make([]byte, *result.ContentLength)
	_, err = result.Body.Read(buf)
	if err != nil {
		return nil, err
	}
	
	return buf, nil
}

// SQSQueue provides AWS SQS integration
type SQSQueue struct {
	client *sqs.SQS
	url    string
}

// NewSQSQueue creates a new SQS queue instance
func (ci *CloudIntegration) NewSQSQueue(queueURL string) (*SQSQueue, error) {
	if ci.awsSession == nil {
		return nil, fmt.Errorf("AWS session not initialized")
	}
	
	return &SQSQueue{
		client: sqs.New(ci.awsSession),
		url:    queueURL,
	}, nil
}

// SendMessageToSQS sends a message to SQS
func (sqsq *SQSQueue) SendMessageToSQS(ctx context.Context, message string) error {
	_, err := sqsq.client.SendMessageWithContext(ctx, &sqs.SendMessageInput{
		QueueUrl:    aws.String(sqsq.url),
		MessageBody: aws.String(message),
	})
	return err
}

// ReceiveMessagesFromSQS receives messages from SQS
func (sqsq *SQSQueue) ReceiveMessagesFromSQS(ctx context.Context, maxMessages int64) ([]*sqs.Message, error) {
	result, err := sqsq.client.ReceiveMessageWithContext(ctx, &sqs.ReceiveMessageInput{
		QueueUrl:            aws.String(sqsq.url),
		MaxNumberOfMessages: aws.Int64(maxMessages),
	})
	if err != nil {
		return nil, err
	}
	
	return result.Messages, nil
}

// DeleteMessageFromSQS deletes a message from SQS
func (sqsq *SQSQueue) DeleteMessageFromSQS(ctx context.Context, receiptHandle string) error {
	_, err := sqsq.client.DeleteMessageWithContext(ctx, &sqs.DeleteMessageInput{
		QueueUrl:      aws.String(sqsq.url),
		ReceiptHandle: aws.String(receiptHandle),
	})
	return err
}

// GCPStorage provides Google Cloud Storage integration
type GCPStorage struct {
	client *storage.Client
	ctx    context.Context
}

// NewGCPStorage creates a new GCP storage instance
func (ci *CloudIntegration) NewGCPStorage(ctx context.Context) (*GCPStorage, error) {
	if ci.gcpProject == "" {
		return nil, fmt.Errorf("GCP project ID not configured")
	}
	
	client, err := storage.NewClient(ctx)
	if err != nil {
		return nil, fmt.Errorf("failed to create GCP storage client: %w", err)
	}
	
	return &GCPStorage{
		client: client,
		ctx:    ctx,
	}, nil
}

// UploadToGCPStorage uploads data to Google Cloud Storage
func (gcs *GCPStorage) UploadToGCPStorage(ctx context.Context, bucket, object string, data []byte) error {
	writer := gcs.client.Bucket(bucket).Object(object).NewWriter(ctx)
	defer writer.Close()
	
	_, err := writer.Write(data)
	return err
}

// DownloadFromGCPStorage downloads data from Google Cloud Storage
func (gcs *GCPStorage) DownloadFromGCPStorage(ctx context.Context, bucket, object string) ([]byte, error) {
	reader, err := gcs.client.Bucket(bucket).Object(object).NewReader(ctx)
	if err != nil {
		return nil, err
	}
	defer reader.Close()
	
	// Read the data
	buf := make([]byte, reader.Attrs.Size)
	_, err = reader.Read(buf)
	if err != nil {
		return nil, err
	}
	
	return buf, nil
}

// GCPPubSub provides Google Cloud Pub/Sub integration
type GCPPubSub struct {
	client *pubsub.Client
	ctx    context.Context
}

// NewGCPPubSub creates a new GCP Pub/Sub instance
func (ci *CloudIntegration) NewGCPPubSub(ctx context.Context) (*GCPPubSub, error) {
	if ci.gcpProject == "" {
		return nil, fmt.Errorf("GCP project ID not configured")
	}
	
	client, err := pubsub.NewClient(ctx, ci.gcpProject)
	if err != nil {
		return nil, fmt.Errorf("failed to create GCP Pub/Sub client: %w", err)
	}
	
	return &GCPPubSub{
		client: client,
		ctx:    ctx,
	}, nil
}

// PublishToPubSub publishes a message to a Pub/Sub topic
func (gcpPubSub *GCPPubSub) PublishToPubSub(ctx context.Context, topicName string, message []byte) error {
	topic := gcpPubSub.client.Topic(topicName)
	defer topic.Stop()
	
	result := topic.Publish(ctx, &pubsub.Message{
		Data: message,
	})
	
	_, err := result.Get(ctx)
	return err
}

// SubscribeToPubSub subscribes to a Pub/Sub subscription
func (gcpPubSub *GCPPubSub) SubscribeToPubSub(ctx context.Context, subscriptionName string, handler func(context.Context, []byte) error) error {
	sub := gcpPubSub.client.Subscription(subscriptionName)
	
	return sub.Receive(ctx, func(ctx context.Context, msg *pubsub.Message) {
		err := handler(ctx, msg.Data)
		if err != nil {
			// Nack the message to retry
			msg.Nack()
		} else {
			// Acknowledge the message
			msg.Ack()
		}
	})
}

// WebhookSender provides webhook integration capabilities
type WebhookSender struct {
	httpClient *http.Client
}

// NewWebhookSender creates a new webhook sender instance
func (ci *CloudIntegration) NewWebhookSender() *WebhookSender {
	return &WebhookSender{
		httpClient: ci.httpClient,
	}
}

// SendWebhook sends a webhook to a specified URL
func (ws *WebhookSender) SendWebhook(ctx context.Context, url string, payload interface{}) error {
    // Marshal the payload to JSON
    data, err := json.Marshal(payload)
    if err != nil {
        return fmt.Errorf("failed to marshal payload: %w", err)
    }
    
    // Create the HTTP request
    req, err := http.NewRequestWithContext(ctx, "POST", url, bytes.NewReader(data))
    if err != nil {
        return fmt.Errorf("failed to create HTTP request: %w", err)
    }
	
	// Set headers
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("User-Agent", "GARP-Blockchain/1.0")
	
	// Send the request
	resp, err := ws.httpClient.Do(req)
	if err != nil {
		return fmt.Errorf("failed to send webhook: %w", err)
	}
	defer resp.Body.Close()
	
	// Check the response status
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return fmt.Errorf("webhook failed with status: %d", resp.StatusCode)
	}
	
	return nil
}

// BlockchainEvent represents a blockchain event for webhook notifications
type BlockchainEvent struct {
	ID        string      `json:"id"`
	Type      string      `json:"type"`
	Timestamp time.Time   `json:"timestamp"`
	Data      interface{} `json:"data"`
}

// SendBlockchainEvent sends a blockchain event via webhook
func (ws *WebhookSender) SendBlockchainEvent(ctx context.Context, url string, eventType string, data interface{}) error {
	event := BlockchainEvent{
		ID:        fmt.Sprintf("evt_%d", time.Now().UnixNano()),
		Type:      eventType,
		Timestamp: time.Now().UTC(),
		Data:      data,
	}
	
	return ws.SendWebhook(ctx, url, event)
}