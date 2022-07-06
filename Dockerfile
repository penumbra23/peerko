FROM ekidd/rust-musl-builder:latest AS builder

ADD --chown=rust:rust . ./

RUN cargo build --release

FROM alpine:latest
COPY --from=builder \
    /home/rust/src/target/x86_64-unknown-linux-musl/release/peerko \
    /usr/local/bin/
    
CMD /usr/local/bin/peerko