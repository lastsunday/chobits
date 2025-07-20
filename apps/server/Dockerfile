FROM --platform=$BUILDPLATFORM alpine:3.22.1
RUN apk --no-cache add make gcc g++ musl-dev binutils autoconf automake libtool pkgconfig check-dev file patch
WORKDIR /app
ARG TARGETOS TARGETARCH
COPY chobits-server-${TARGETOS}-${TARGETARCH} ./server
COPY script/download_model.sh /app/download_model.sh 
RUN chmod +x /app/download_model.sh
RUN sh /app/download_model.sh
EXPOSE 3000 
CMD ["./server"]
