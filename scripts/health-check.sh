#!/usr/bin/env bash
# Health check script for Hybrid Architecture development environment
# Verifies all services are running and accessible

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Counters
TOTAL_CHECKS=0
PASSED_CHECKS=0
FAILED_CHECKS=0

echo "================================================"
echo "Hybrid Architecture - Service Health Check"
echo "================================================"
echo ""

# Helper function to check service health
check_service() {
    local service_name=$1
    local check_command=$2
    local expected_output=${3:-""}

    TOTAL_CHECKS=$((TOTAL_CHECKS + 1))

    echo -n "Checking ${service_name}... "

    if eval "$check_command" > /dev/null 2>&1; then
        if [ -n "$expected_output" ]; then
            output=$(eval "$check_command" 2>&1)
            if echo "$output" | grep -q "$expected_output"; then
                echo -e "${GREEN}✓ PASS${NC}"
                PASSED_CHECKS=$((PASSED_CHECKS + 1))
                return 0
            else
                echo -e "${RED}✗ FAIL${NC} (unexpected output)"
                FAILED_CHECKS=$((FAILED_CHECKS + 1))
                return 1
            fi
        else
            echo -e "${GREEN}✓ PASS${NC}"
            PASSED_CHECKS=$((PASSED_CHECKS + 1))
            return 0
        fi
    else
        echo -e "${RED}✗ FAIL${NC}"
        FAILED_CHECKS=$((FAILED_CHECKS + 1))
        return 1
    fi
}

# Check Docker Compose services are running
echo "=== Docker Compose Services ==="
check_service "Docker Compose Up" "docker compose ps --quiet postgres rabbitmq redis | wc -l | grep -q 3"

# Check PostgreSQL
echo ""
echo "=== PostgreSQL Event Store ==="
check_service "PostgreSQL Container" "docker compose ps postgres | grep -q 'Up'"
check_service "PostgreSQL Port 5435" "nc -z localhost 5435"
check_service "PostgreSQL Connection" "docker compose exec -T postgres pg_isready -U event_store -d event_store" "accepting connections"
check_service "PostgreSQL Database Exists" "docker compose exec -T postgres psql -U event_store -d event_store -c 'SELECT 1' | grep -q '1 row'"

# Check RabbitMQ
echo ""
echo "=== RabbitMQ (Bloodbank) ==="
check_service "RabbitMQ Container" "docker compose ps rabbitmq | grep -q 'Up'"
check_service "RabbitMQ AMQP Port 5675" "nc -z localhost 5675"
check_service "RabbitMQ Management Port 15675" "nc -z localhost 15675"
check_service "RabbitMQ Ping" "docker compose exec -T rabbitmq rabbitmq-diagnostics -q ping" "succeeded"
check_service "RabbitMQ Exchange" "docker compose exec -T rabbitmq rabbitmqctl list_exchanges | grep -q 'bloodbank.events.v1'"
check_service "RabbitMQ Queue (Event Store)" "docker compose exec -T rabbitmq rabbitmqctl list_queues | grep -q 'event_store_manager_queue'"

# Check Redis
echo ""
echo "=== Redis Cache ==="
check_service "Redis Container" "docker compose ps redis | grep -q 'Up'"
check_service "Redis Port 6382" "nc -z localhost 6382"
check_service "Redis Ping" "docker compose exec -T redis redis-cli ping" "PONG"
check_service "Redis Info" "docker compose exec -T redis redis-cli info server | grep -q 'redis_version'"

# Summary
echo ""
echo "================================================"
echo "Health Check Summary"
echo "================================================"
echo "Total Checks: ${TOTAL_CHECKS}"
echo -e "Passed: ${GREEN}${PASSED_CHECKS}${NC}"
echo -e "Failed: ${RED}${FAILED_CHECKS}${NC}"

if [ ${FAILED_CHECKS} -eq 0 ]; then
    echo ""
    echo -e "${GREEN}✓ All services are healthy!${NC}"
    echo ""
    echo "Service URLs:"
    echo "  PostgreSQL:          localhost:5435"
    echo "  RabbitMQ AMQP:       localhost:5675"
    echo "  RabbitMQ Management: http://localhost:15675"
    echo "  Redis:               localhost:6382"
    echo ""
    echo "Quick commands:"
    echo "  mise db:psql          - Connect to PostgreSQL"
    echo "  mise rabbitmq:ui      - Open RabbitMQ UI"
    echo "  mise redis:cli        - Connect to Redis CLI"
    echo "  mise dev:logs         - View service logs"
    exit 0
else
    echo ""
    echo -e "${RED}✗ Some services are unhealthy!${NC}"
    echo ""
    echo "Troubleshooting:"
    echo "  mise dev:logs         - View service logs"
    echo "  mise dev:restart      - Restart all services"
    echo "  docker compose ps     - Check container status"
    echo ""
    exit 1
fi
