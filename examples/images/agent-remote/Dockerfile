FROM ubuntu:24.04

RUN apt-get update && apt-get install -y openssh-server git curl coreutils rsync && rm -rf /var/lib/apt/lists/*

RUN useradd -m -s /bin/bash agent && mkdir /run/sshd

USER agent
RUN curl -fsSL https://claude.ai/install.sh | bash
USER root

COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

EXPOSE 22
ENTRYPOINT ["/entrypoint.sh"]
