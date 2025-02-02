#!/bin/sh
# Write environment variable content to nginx.conf
if [ -z "$NGINX_CONF" ]; then
    echo 'Error: NGINX_CONF environment variable is not set'
    exit 1
fi

echo "$NGINX_CONF" > /etc/nginx/nginx.conf

# Verify configuration syntax
if ! nginx -t; then
    exit 1
fi

# Start nginx
exec nginx -g "daemon off;"
