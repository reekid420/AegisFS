#!/bin/bash
# Check Environment Variables for AegisFS CI/CD

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🔍 Checking AegisFS CI/CD Environment Variables${NC}"
echo "=================================================="

# Check basic development environment
echo -e "\n${BLUE}📋 Development Environment:${NC}"

if command -v cargo >/dev/null 2>&1; then
    echo -e "✅ Cargo: $(cargo --version)"
else
    echo -e "❌ Cargo: Not found"
fi

if command -v rustc >/dev/null 2>&1; then
    echo -e "✅ Rust: $(rustc --version)"
else
    echo -e "❌ Rust: Not found"
fi

if [[ -c /dev/fuse ]]; then
    echo -e "✅ FUSE: Available at /dev/fuse"
else
    echo -e "❌ FUSE: /dev/fuse not found"
fi

# Check optional CI/CD environment variables
echo -e "\n${BLUE}🚀 CI/CD Environment Variables:${NC}"

check_env_var() {
    local var_name="$1"
    local description="$2"
    
    if [[ -n "${!var_name:-}" ]]; then
        # Show first 10 chars and mask the rest for security
        local masked_value="${!var_name:0:10}***"
        echo -e "✅ $var_name: Set ($masked_value) - $description"
        return 0
    else
        echo -e "⚠️  $var_name: Not set - $description"
        return 1
    fi
}

# Check each optional environment variable
codecov_set=false
docker_user_set=false
docker_token_set=false

if check_env_var "CODECOV_TOKEN" "Code coverage reporting"; then
    codecov_set=true
fi

if check_env_var "DOCKERHUB_USERNAME" "Docker Hub publishing"; then
    docker_user_set=true
fi

if check_env_var "DOCKERHUB_TOKEN" "Docker Hub authentication"; then
    docker_token_set=true
fi

# Summary and recommendations
echo -e "\n${BLUE}📊 Feature Status:${NC}"

if $codecov_set; then
    echo -e "✅ Code Coverage: Enabled (reports will be uploaded to Codecov)"
else
    echo -e "⚠️  Code Coverage: Disabled (set CODECOV_TOKEN to enable)"
fi

if $docker_user_set && $docker_token_set; then
    echo -e "✅ Docker Publishing: Enabled (images will be published to Docker Hub)"
elif $docker_user_set || $docker_token_set; then
    echo -e "⚠️  Docker Publishing: Partially configured (need both DOCKERHUB_USERNAME and DOCKERHUB_TOKEN)"
else
    echo -e "⚠️  Docker Publishing: Disabled (set DOCKERHUB_USERNAME and DOCKERHUB_TOKEN to enable)"
fi

echo -e "\n${BLUE}🎯 What works without environment variables:${NC}"
echo "✅ All code quality checks (format, lint, clippy)"
echo "✅ Security auditing (cargo-audit, cargo-deny)"
echo "✅ Unit and integration testing"
echo "✅ Cross-platform builds"
echo "✅ Performance benchmarks"
echo "✅ Memory safety checks (MIRI)"
echo "✅ Release binary creation"

if ! $codecov_set && ! ($docker_user_set && $docker_token_set); then
    echo -e "\n${YELLOW}💡 To enable optional features:${NC}"
    echo ""
    echo "For code coverage reporting:"
    echo "  1. Sign up at https://codecov.io"
    echo "  2. Add your repository"
    echo "  3. export CODECOV_TOKEN=<your-token>"
    echo ""
    echo "For Docker publishing:"
    echo "  1. Create Docker Hub account"
    echo "  2. Generate access token in settings"
    echo "  3. export DOCKERHUB_USERNAME=<username>"
    echo "  4. export DOCKERHUB_TOKEN=<token>"
    echo ""
    echo "Add these to your shell profile (.bashrc, .zshrc) to persist them."
fi

echo -e "\n${GREEN}🚀 Ready to develop AegisFS!${NC}"
echo "Run: ./scripts/ci-helpers.sh full-ci" 