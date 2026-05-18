FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*

# Assuming binaries and assets are provided in the context or build stage
# For a production image, these would be copied from a builder stage or downloaded
COPY ank-server /usr/local/bin/ank-server
COPY ui-dist/   /usr/share/aegis/ui/

ENV UI_DIST_PATH=/usr/share/aegis/ui
# Release binary refuses to start without an explicit plugin signer key.
# Default the image to insecure-plugins mode so docker-run-from-scratch
# users don't crash on first boot. Override with a real AEGIS_PLUGIN_ROOT_KEY
# at runtime (-e or docker-compose environment block) for production.
ENV AEGIS_ALLOW_INSECURE_PLUGINS=1
EXPOSE 8000 50051

ENTRYPOINT ["ank-server"]
