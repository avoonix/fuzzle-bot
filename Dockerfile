FROM rust:1.75-alpine as builder-base
ARG RUST_TARGET
RUN apk add --no-cache musl-dev 
ENV SYSROOT=/dummy
RUN rustup target add $RUST_TARGET
WORKDIR /wd
RUN cargo install cargo-chef --locked

FROM builder-base as planner
COPY . /wd
RUN cargo chef prepare --recipe-path cache-info.json

FROM builder-base as builder
ARG RUST_TARGET
COPY --from=planner /wd/cache-info.json cache-info.json
RUN cargo chef cook --release --recipe-path cache-info.json --target=$RUST_TARGET
COPY . /wd
RUN cargo build --bins --release --target=$RUST_TARGET

FROM scratch as runtime
ARG RUST_TARGET
COPY --from=builder /wd/target/$RUST_TARGET/release/fuzzle-bot /

ENTRYPOINT ["./fuzzle-bot"]
CMD ["help"]
