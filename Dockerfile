FROM rust:slim-trixie as builder
ARG RUST_TARGET
WORKDIR /wd

RUN apt-get update && apt-get install -y \
    cmake build-essential libopenblas-dev protobuf-compiler libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

ENV SYSROOT=/dummy
ENV LIBSQLITE3_FLAGS="-DSQLITE_ENABLE_MATH_FUNCTIONS -DSQLITE_ENABLE_STAT4 -DSQLITE_OMIT_DEPRECATED -DSQLITE_OMIT_EXPLAIN"
# RUN rustup target add $RUST_TARGET
COPY . /wd
# RUN echo "bin-target-triple = \"$RUST_TARGET\"" >> Cargo.toml
RUN cargo build --release -vv

FROM debian:trixie-slim as runtime
RUN apt-get update && apt-get install -y \
    libopenblas0 libsqlite3-0 ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /wd/target/release/fuzzle-bot /
EXPOSE 3000
ENTRYPOINT ["./fuzzle-bot"]
CMD ["help"]
