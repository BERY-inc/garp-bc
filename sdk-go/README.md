# GARP Go SDK

Go client for GARP participant-node JSON-RPC.

```go
package main

import (
    "context"
    "fmt"
    garp "garp/sdk-go/garp"
)

func main() {
    c := garp.NewClient("http://localhost:8080")
    ctx := context.Background()
    slot, _ := c.GetSlotCtx(ctx)
    fmt.Println(slot)
}
```