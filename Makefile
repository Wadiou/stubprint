# Shortcuts for the development containers in compose.yaml.
.PHONY: up down logs

up:
	docker compose up -d --build

down:
	docker compose down

logs:
	docker compose logs -f
