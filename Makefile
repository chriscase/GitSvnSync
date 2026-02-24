.PHONY: build release test lint clean web-ui test-env-up test-env-down test-all install \
       validate validate-quick validate-soak validate-ghe-live validate-ghe-live-dry-run

# Development
build:
	cargo build

release:
	cargo build --release

test:
	cargo test --workspace

lint:
	cargo clippy --workspace -- -D warnings
	cargo fmt --check

fmt:
	cargo fmt

clean:
	cargo clean
	rm -rf web-ui/dist web-ui/node_modules

# Web UI
web-ui:
	cd web-ui && npm install && npm run build

web-ui-dev:
	cd web-ui && npm install && npm run dev

# Test Environment
test-env-up:
	docker compose -f tests/docker-compose.yml up -d --build
	@echo "Waiting for services to be healthy..."
	@sleep 10
	@echo "Test environment ready!"
	@echo "  SVN:        http://localhost:8081/svn/testrepo"
	@echo "  Gitea:      http://localhost:3000"
	@echo "  GitSvnSync: http://localhost:8080"

test-env-down:
	docker compose -f tests/docker-compose.yml down -v

test-env-logs:
	docker compose -f tests/docker-compose.yml logs -f

test-all: test
	cargo test --workspace --test '*' -- --nocapture

# E2E / integration tests (requires svn + svnadmin in PATH)
test-e2e:
	@echo "Running E2E and integration tests..."
	cargo test --workspace --test '*' -- --nocapture

# Installation
install: release
	sudo install -m 755 target/release/gitsvnsync-daemon /usr/local/bin/
	sudo install -m 755 target/release/gitsvnsync /usr/local/bin/
	@echo "Installed gitsvnsync-daemon and gitsvnsync to /usr/local/bin/"

# Validation scripts
validate:
	scripts/controlled-validation.sh

validate-quick:
	scripts/controlled-validation.sh --quick

validate-soak:
	scripts/enterprise-soak.sh

validate-soak-dry-run:
	scripts/enterprise-soak.sh --dry-run

validate-ghe-live:
	scripts/ghe-live-validation.sh --cycles 1

validate-ghe-live-dry-run:
	scripts/ghe-live-validation.sh --dry-run

# Docker
docker-build:
	docker build -t gitsvnsync:latest .

docker-run:
	docker run -d \
		--name gitsvnsync \
		-p 8080:8080 \
		-v $$(pwd)/config.example.toml:/etc/gitsvnsync/config.toml:ro \
		gitsvnsync:latest
