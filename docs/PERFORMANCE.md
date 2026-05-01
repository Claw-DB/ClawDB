# Performance

## Benchmark Results (Placeholder)

| Benchmark | Current | Target |
| --- | --- | --- |
| remember p99 | TBD | < 20ms |
| semantic search p99 (1000 memories) | TBD | < 50ms |
| session validate p99 | TBD | < 2ms |
| event emit (10 subscribers) p99 | TBD | < 100us |
| transaction commit (5 writes) p99 | TBD | < 30ms |

## Tuning Guide

### Cache Sizes

1. Increase `core.cache_size_mb` for read-heavy workloads.
2. Ensure host has enough memory headroom for vector index residency.

### Connection Pools

1. Keep DB/network pools bounded to avoid tail latency spikes.
2. Use queue metrics to tune `server.max_connections`.

### WAL and Persistence

1. Keep SQLite WAL enabled for concurrent writes.
2. Schedule WAL checkpoints for long-running deployments.

### Vector Index Parameters

1. Tune `ef_construction` and query-time `ef_search` in vector backend.
2. Balance recall and latency per task profile.

## Scaling Strategies

### Vertical Scaling

1. Increase CPU for embedding and search-heavy workloads.
2. Increase memory for vector index and cache hit rates.

### Horizontal Scaling

1. Use read replicas for query-heavy endpoints.
2. Scale API pods behind load balancers with sticky or token-aware routing.

### Edge Deployment

1. Place semantic cache and inference close to agent runtime.
2. Sync durable memory asynchronously to regional hub.
