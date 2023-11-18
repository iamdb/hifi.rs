FROM rust:1-bookworm as build

ARG PKG_CONFIG_PATH="/usr/lib/x86_64-linux-gnu/pkgconfig/:${PKG_CONFIG_PATH}"

RUN curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash -s -- --to /bin

WORKDIR /app

COPY . .

RUN just build-player x86_64-unknown-linux-gnu

RUN mv target/x86_64-unknown-linux-gnu/release/hifi-rs /usr/local/bin

FROM scratch

COPY --from=build /usr/local/bin/hifi-rs .

ENTRYPOINT ["./hifi-rs"]
