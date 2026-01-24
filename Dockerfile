FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*

COPY docker-build/codeprism /usr/local/bin/codeprism

RUN chmod +x /usr/local/bin/codeprism

WORKDIR /workspace

ENTRYPOINT ["codeprism"]
CMD ["--help"]
