FROM rust:latest as builder
ARG RUST_TARGET
WORKDIR /wd
RUN apt-get update
RUN apt-get install -y cmake build-essential libopenblas-dev
# RUN ln -s /bin/g++ /bin/musl-g++
ENV SYSROOT=/dummy
# RUN rustup target add $RUST_TARGET
RUN cargo install cargo-leptos
RUN rustup target add wasm32-unknown-unknown
COPY . /wd
# RUN echo "bin-target-triple = \"$RUST_TARGET\"" >> Cargo.toml
RUN cargo leptos build --release -vv


FROM rust:latest as runtime
ARG RUST_TARGET
# COPY --from=builder /wd/target/$RUST_TARGET/release/fuzzle-bot /
RUN apt-get update
RUN apt-get install -y libopenblas0
COPY --from=builder /wd/target/release/fuzzle-bot /
COPY --from=builder /wd/target/site /site
EXPOSE 3000
ENV LEPTOS_SITE_ROOT="/site"
ENV LEPTOS_SITE_ADDR="0.0.0.0:3000"
ENTRYPOINT ["./fuzzle-bot"]
CMD ["help"]
