FROM rust:latest as builder
ARG RUST_TARGET
WORKDIR /wd
RUN apt-get update
RUN apt-get install -y cmake build-essential libopenblas-dev protobuf-compiler libsqlite3-dev
# RUN ln -s /bin/g++ /bin/musl-g++
ENV SYSROOT=/dummy
ENV LIBSQLITE3_FLAGS="-DSQLITE_ENABLE_MATH_FUNCTIONS -DSQLITE_ENABLE_STAT4 -DSQLITE_OMIT_DEPRECATED -DSQLITE_OMIT_EXPLAIN"
# RUN rustup target add $RUST_TARGET
COPY . /wd
# RUN echo "bin-target-triple = \"$RUST_TARGET\"" >> Cargo.toml
RUN cargo build --release -vv

FROM rust:latest as runtime
ARG RUST_TARGET
# COPY --from=builder /wd/target/$RUST_TARGET/release/fuzzle-bot /
RUN apt-get update
RUN apt-get install -y libopenblas0
COPY --from=builder /wd/target/release/fuzzle-bot /
# COPY --from=builder /wd/target/site /site
EXPOSE 3000
ENTRYPOINT ["./fuzzle-bot"]
CMD ["help"]
