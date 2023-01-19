FROM ekidd/rust-musl-builder:latest AS builder

ADD --chown=rust:rust . ./

RUN cargo build --release

FROM alpine:latest
LABEL org.opencontainers.image.authors="penumbra23"
LABEL org.label-schema.name="penumbra23/peerko"
LABEL org.label-schema.description="NAT Traversla P2P Application"
LABEL org.label-schema.url="https://github.com/penumbra23/peerko"
LABEL org.label-schema.vcs-url="https://github.com/penumbra23/peerko"
LABEL org.label-schema.docker.cmd="docker run -it --rm -p 8000:8000 penumbra23/peerko:latest --name my-client-app --group chatting --port 8000 -b SERVER_IP:8000"

COPY --from=builder \
    /home/rust/src/target/x86_64-unknown-linux-musl/release/peerko \
    /usr/local/bin/
    
ENTRYPOINT ["/usr/local/bin/peerko"]