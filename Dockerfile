FROM rust:1-bookworm as build

RUN apt-get update && apt-get install -y curl libgstreamer1.0-dev

ENV PKG_CONFIG_PATH_x86_64_unknown_linux_gnu="/usr/lib/x86_64-linux-gnu/pkgconfig"
ENV DATABASE_URL "sqlite:///tmp/data.db"

RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
RUN cargo binstall --no-confirm sqlx-cli

WORKDIR /app

COPY . .

RUN touch /tmp/data.db && cd hifirs && cargo sqlx database reset -y

RUN cargo build --bin hifi-rs --release --target x86_64-unknown-linux-gnu

RUN mv target/x86_64-unknown-linux-gnu/release/hifi-rs /usr/local/bin

FROM scratch

COPY --from=build /usr/local/bin/hifi-rs .

ENTRYPOINT ["./hifi-rs"]
