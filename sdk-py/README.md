# GARP Python SDK

Synchronous Python client for GARP participant-node JSON-RPC.

```python
from garp_sdk import GarpClient, AsyncGarpClient

client = GarpClient("http://localhost:8080")
slot = client.get_slot()
leader = client.get_slot_leader()
block = client.get_block_by_slot(slot)

# Async usage
# import asyncio
# async def main():
#     async with AsyncGarpClient("http://localhost:8080") as ac:
#         slot = await ac.get_slot()
#         leader = await ac.get_slot_leader()
#         block = await ac.get_block_by_slot(slot)
# asyncio.run(main())
```