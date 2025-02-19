FROM nginx:alpine

LABEL org.opencontainers.image.source https://github.com/opencloudtool/opencloudtool

COPY nginx-entrypoint.sh /
RUN chmod +x /nginx-entrypoint.sh
ENTRYPOINT ["/nginx-entrypoint.sh"]
