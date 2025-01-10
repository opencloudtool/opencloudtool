# FastAPI Hello World Example

This is a simple "Hello World" application using FastAPI and Python 3.12. It demonstrates how to set up a basic FastAPI project with Docker support.

## Prerequisites

- Docker

## Getting Started

1. Build the Docker image:

   ```
   docker build -t fastapi-hello-world .
   ```

2. Run the Docker container:

   ```
   docker run -p 8000:8000 fastapi-hello-world
   ```

3. Open your browser and navigate to `http://localhost:8000`

## Build and push image to GitHub Container Registry

### Auth in GitHub Container Registry

https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry#authenticating-with-a-personal-access-token-classic

### Build and push

```bash
docker build --platform linux/amd64 -t ghcr.io/opencloudtool/example-python-fastapi:latest .
docker push ghcr.io/opencloudtool/example-python-fastapi:latest
```

## Project Structure

- `main.py`: Contains the FastAPI application code
- `Dockerfile`: Instructions for building the Docker image
- `pyproject.toml`: Poetry configuration and dependencies (used in Docker build)

## API Endpoints

- `GET /`: Returns a JSON response with a "Hello World" message
