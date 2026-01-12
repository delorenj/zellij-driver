# Development Environment Setup

This document describes the local development environment for the Hybrid BMAD + Letta Architecture project.

## Overview

The development environment provides:
- **PostgreSQL 16** - Event store database
- **RabbitMQ 3.12** - Bloodbank event bus with management UI
- **Redis 7** - Cache and real-time state

All services run in Docker containers with persistent volumes and custom ports to avoid conflicts with other 33GOD services.

## Quick Start

```bash
# Start all services
mise dev

# Check health status
mise dev:health

# View logs
mise dev:logs

# Stop services
mise dev:stop
```

## Prerequisites

- Docker and Docker Compose
- [mise](https://mise.jdx.dev/) task runner
- netcat (`nc`) for health checks

## Service Ports

The following ports are used to avoid conflicts with other 33GOD projects:

| Service | Host Port | Container Port | Purpose |
|---------|-----------|----------------|---------|
| PostgreSQL | 5435 | 5432 | Event store database |
| RabbitMQ AMQP | 5675 | 5672 | Message broker |
| RabbitMQ Management | 15675 | 15672 | Web UI |
| Redis | 6382 | 6379 | Cache |

**Note:** These ports differ from standard ports (5432, 5672, 15672, 6379) to allow running multiple 33GOD projects simultaneously.

## Initial Setup

### 1. Configure Environment Variables

Copy the example environment file and set your passwords:

```bash
cp .env.example .env
```

Edit `.env` and set secure passwords:

```bash
# Generate secure passwords
openssl rand -base64 32

# Update these values in .env
POSTGRES_PASSWORD=your_secure_postgres_password_here
RABBITMQ_PASSWORD=your_secure_rabbitmq_password_here
RABBITMQ_ERLANG_COOKIE=your_secure_cookie_here
```

### 2. Start Services

```bash
# Start all services in detached mode
mise dev

# Or use docker compose directly
docker compose up -d
```

### 3. Verify Health

```bash
# Run comprehensive health check
mise dev:health

# Expected output: All 15 checks should pass
# ✓ Docker Compose Up
# ✓ PostgreSQL (4 checks)
# ✓ RabbitMQ (6 checks)
# ✓ Redis (4 checks)
```

## Available Commands

### Service Management

```bash
mise dev              # Start all services
mise dev:stop         # Stop all services (data persists)
mise dev:down         # Stop and remove containers (data persists)
mise dev:restart      # Restart all services
mise dev:clean        # Stop and delete all data (WARNING: destructive)
mise dev:logs         # Follow logs from all services
mise dev:health       # Run health check script
```

### Database

```bash
mise db:psql          # Connect to PostgreSQL with psql
mise db:migrate       # Run database migrations (TODO: STORY-002)
mise db:rollback      # Rollback last migration (TODO: STORY-002)
mise db:reset         # Reset database to clean state
```

Example psql session:

```bash
mise db:psql
# Inside psql:
event_store=# \dt        # List tables
event_store=# \l         # List databases
event_store=# \q         # Quit
```

### RabbitMQ

```bash
mise rabbitmq:ui      # Open management UI in browser
mise rabbitmq:status  # Show RabbitMQ status
mise rabbitmq:queues  # List all queues
```

RabbitMQ Management UI:
- URL: http://localhost:15675
- Username: bloodbank
- Password: (from .env RABBITMQ_PASSWORD)

### Redis

```bash
mise redis:cli        # Connect to Redis CLI
mise redis:ping       # Test Redis connection
```

Example Redis session:

```bash
mise redis:cli
# Inside redis-cli:
127.0.0.1:6379> PING
PONG
127.0.0.1:6379> KEYS *
127.0.0.1:6379> exit
```

### Testing

```bash
mise test             # Run all tests
mise test:integration # Run integration tests (requires services running)
```

### Building

```bash
mise build            # Build the project
mise build:release    # Build release version
```

## Service Details

### PostgreSQL Event Store

**Image:** postgres:16-alpine

**Configuration:**
- Database: `event_store`
- User: `event_store`
- Performance tuned for event sourcing workload
- Persistent volume: `zellij-driver_postgres_data`

**Performance Optimizations:**
- max_connections: 200
- shared_buffers: 256MB
- effective_cache_size: 1GB
- WAL configuration for event store

**Connection String:**
```
postgresql://event_store:${POSTGRES_PASSWORD}@localhost:5435/event_store
```

### RabbitMQ (Bloodbank Event Bus)

**Image:** rabbitmq:3.12-management-alpine

**Configuration:**
- Virtual host: `/`
- User: `bloodbank`
- Exchange: `bloodbank.events.v1` (topic)
- Queues:
  - `event_store_manager_queue`
  - `letta_handoff_queue`
- Persistent volume: `zellij-driver_rabbitmq_data`

**Pre-configured via definitions.json:**
- Exchange `bloodbank.events.v1` (topic, durable)
- Queue bindings with routing patterns
- Dead letter exchange for failed messages

**Management UI Features:**
- Queue monitoring
- Message publishing/consuming
- Connection tracking
- Exchange/queue management

### Redis Cache

**Image:** redis:7-alpine

**Configuration:**
- Databases: 16
- Max memory: 512MB
- Eviction policy: allkeys-lru
- Persistence: AOF + RDB snapshots
- Keyspace notifications: AKE (all events)
- Persistent volume: `zellij-driver_redis_data`

**Use Cases:**
- Cache layer
- Real-time state management
- Pub/sub for event streaming

## Data Persistence

All service data is stored in Docker volumes:

```bash
# List volumes
docker volume ls | grep zellij-driver

# Inspect volume
docker volume inspect zellij-driver_postgres_data

# Backup PostgreSQL data
docker compose exec postgres pg_dump -U event_store event_store > backup.sql

# Restore PostgreSQL data
cat backup.sql | docker compose exec -T postgres psql -U event_store event_store
```

## Troubleshooting

### Services won't start

**Check port conflicts:**
```bash
# Check if ports are already in use
lsof -i :5435  # PostgreSQL
lsof -i :5675  # RabbitMQ AMQP
lsof -i :15675 # RabbitMQ Management
lsof -i :6382  # Redis
```

**Check Docker resources:**
```bash
docker system df        # Disk usage
docker system prune -a  # Clean up (WARNING: removes stopped containers)
```

### Services keep restarting

**Check logs:**
```bash
docker compose logs postgres
docker compose logs rabbitmq
docker compose logs redis
```

**Common issues:**
- Corrupt volumes: Run `mise dev:clean` to remove all data
- Configuration errors: Check `.env` file
- Resource limits: Increase Docker memory allocation

### Health check fails

**Run individual checks:**
```bash
# Test PostgreSQL
docker compose exec postgres pg_isready -U event_store

# Test RabbitMQ
docker compose exec rabbitmq rabbitmq-diagnostics -q ping

# Test Redis
docker compose exec redis redis-cli ping
```

### Cannot connect to services

**Verify services are running:**
```bash
docker compose ps

# All services should show "Up" status
```

**Test connectivity:**
```bash
nc -zv localhost 5435   # PostgreSQL
nc -zv localhost 5675   # RabbitMQ
nc -zv localhost 15675  # RabbitMQ Management
nc -zv localhost 6382   # Redis
```

### Database migrations fail

```bash
# Reset database
mise db:reset

# Wait for initialization
sleep 10

# Re-run migrations
mise db:migrate
```

### RabbitMQ exchange/queue not created

**Check definitions loaded:**
```bash
docker compose exec rabbitmq rabbitmqctl list_exchanges
docker compose exec rabbitmq rabbitmqctl list_queues
```

**Reload definitions:**
```bash
mise dev:restart
sleep 15
docker compose exec rabbitmq rabbitmqctl list_exchanges
```

## Development Workflow

### Starting a new feature

```bash
# 1. Ensure clean environment
mise dev:health

# 2. Create feature branch
git checkout -b feature/your-feature

# 3. Run tests to ensure baseline
mise test

# 4. Develop with live services
mise dev:logs  # Monitor logs in another terminal

# 5. Test integration
mise test:integration
```

### Before committing

```bash
# Run tests
mise test

# Check services are healthy
mise dev:health

# Format and lint Rust code
cargo fmt
cargo clippy

# Commit changes
git add .
git commit -m "feat: your feature description"
```

## Architecture Context

This development environment supports Phase 1 of the Hybrid BMAD + Letta Architecture:

- **PostgreSQL** stores the event stream (event sourcing pattern)
- **RabbitMQ** (Bloodbank) handles event-driven communication between services
- **Redis** provides caching and real-time state management

See `docs/sprint-plan-hybrid-architecture-phase1-2026-01-11.md` for the full architecture plan.

## Next Steps

After completing STORY-000 (this environment setup):

1. **STORY-001** - Implement PostgreSQL event store schema
2. **STORY-002** - Set up database migrations with sqlx
3. **STORY-003** - Create Bloodbank event schemas

## Support

For issues or questions:
- Check `scripts/health-check.sh` for diagnostic commands
- Review Docker Compose logs: `mise dev:logs`
- See troubleshooting section above
- Refer to sprint plan: `docs/sprint-plan-hybrid-architecture-phase1-2026-01-11.md`
