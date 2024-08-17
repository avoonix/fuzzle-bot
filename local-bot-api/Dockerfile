# adapted from https://github.com/lukaszraczylo/tdlib-telegram-bot-api-docker/blob/main/Dockerfile
FROM alpine:latest AS build
WORKDIR /srv
RUN apk add --no-cache alpine-sdk linux-headers git zlib-dev openssl-dev gperf php cmake
RUN git clone --depth 1 --recurse-submodules --shallow-submodules https://github.com/tdlib/telegram-bot-api.git /srv
COPY patches /patches
RUN git apply /patches/*
RUN mkdir -p build && cd build && cmake -DCMAKE_INSTALL_PREFIX:PATH=../tdlib -DCMAKE_BUILD_TYPE=Release ..
RUN cd /srv/build && cmake --build . --target install

FROM alpine:latest
RUN apk add --no-cache zlib-dev openssl-dev libstdc++
COPY --from=build /srv/build/telegram-bot-api /srv/telegram-bot-api
VOLUME /data
EXPOSE 80
RUN addgroup -g 101 nginx
RUN adduser -G nginx -H -u 101 --disabled-password nginx
# same user as nginx so that it can access the files from the volume
ENTRYPOINT ["/srv/telegram-bot-api", "--dir=/data", "--local", "--http-port=80", "--username=nginx", "--groupname=nginx"]

# run on host: sudo chown 101:101 telegram-bot-api-data
