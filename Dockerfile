FROM alpine:latest

# Install FUSE and required dependencies for running fhir-fuse
RUN apk add --no-cache \
    fuse3 \
    fuse3-dev \
    ca-certificates \
    curl

# Create mount point directory
RUN mkdir -p /mnt/fhir

# Copy the pre-built binary for Alpine (musl)
# The architecture will be determined by the build context
ARG TARGETARCH
COPY target/${TARGETARCH}-unknown-linux-musl/release/fhir-fuse /usr/local/bin/fhir-fuse

# Make the binary executable
RUN chmod +x /usr/local/bin/fhir-fuse

RUN mkdir -p /mnt/fhir
# Set working directory
WORKDIR /mnt/fhir

# Default command
CMD ["/usr/local/bin/fhir-fuse"]

