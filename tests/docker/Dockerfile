FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl ca-certificates git \
    && rm -rf /var/lib/apt/lists/*

# Install lattice via the official install script
RUN curl -fsSL https://lattice.forkzero.ai/install.sh | sh

# Verify install
RUN lattice --version

COPY e2e-test.sh /e2e-test.sh
RUN chmod +x /e2e-test.sh

WORKDIR /workspace

CMD ["/e2e-test.sh"]
