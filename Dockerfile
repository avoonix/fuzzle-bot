FROM rust:1.75-alpine as builder-base
RUN apk add --no-cache musl-dev 
ENV SYSROOT=/dummy
RUN rustup target add x86_64-unknown-linux-musl
WORKDIR /wd
RUN cargo install cargo-chef --locked

FROM builder-base as planner
COPY . /wd
RUN cargo chef prepare --recipe-path cache-info.json

FROM builder-base as builder
COPY --from=planner /wd/cache-info.json cache-info.json
RUN cargo chef cook --release --recipe-path cache-info.json --target=x86_64-unknown-linux-musl
COPY . /wd
RUN cargo build --bins --release --target=x86_64-unknown-linux-musl

FROM scratch as runtime
COPY --from=builder /wd/target/x86_64-unknown-linux-musl/release/fuzzle-bot /

ENTRYPOINT ["./fuzzle-bot"]
CMD ["help"]
