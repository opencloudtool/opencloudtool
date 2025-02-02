# NGINX Docker image with custom configuration file

Custom configuration file is passed via environment variable `NGINX_CONF`.

## Build image

```bash
docker build --platform linux/amd64 -t ghcr.io/opencloudtool/nginx-with-conf:latest .
docker push ghcr.io/opencloudtool/nginx-with-conf:latest
```

## Run container

```bash
docker run -p 80:80 -e NGINX_CONF="$NGINX_CONF" ghcr.io/opencloudtool/nginx-with-conf:latest
```

## nginx.conf example for 2 applications

```nginx
events {
    worker_connections 1024;
}

http {
    upstream backend {
        server app_1:8000;
        server app_2:8000;
    }

    server {
        listen       80;

        location / {
            proxy_pass http://backend;
        }
    }
}
```
