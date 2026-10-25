FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libsqlite3-0 \
    python3 \
    && rm -rf /var/lib/apt/lists/*

COPY docker-build/codeprism /usr/local/bin/codeprism
COPY docker-build/custom_analyzers /usr/local/share/codeprism/custom_analyzers

RUN chmod +x /usr/local/bin/codeprism

ENV CODEPRISM_ANALYZERS_DIR=/usr/local/share/codeprism/custom_analyzers

WORKDIR /workspace

ENTRYPOINT ["codeprism"]
CMD ["--help"]
