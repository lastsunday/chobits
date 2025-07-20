FROM --platform=$BUILDPLATFORM ubuntu:24.04
RUN apt update && apt install wget
WORKDIR /app
ARG TARGETOS TARGETARCH
COPY chobits-server-${TARGETOS}-${TARGETARCH} ./server
COPY script/download_model.sh /app/download_model.sh 
RUN chmod +x /app/download_model.sh
RUN sh /app/download_model.sh
EXPOSE 3000 
CMD ["./server"]
