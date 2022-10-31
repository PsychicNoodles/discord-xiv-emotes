FROM debian:buster-slim
RUN mkdir dxe
WORKDIR /dxe

ARG EXEC=discord-xiv-emotes
## uncomment to copy env vars defined in .env directly into container
## if you do so, make sure to remove the corresponding env vars from docker-compose.yaml
# ADD .env .env

ADD $EXEC discord-xiv-emotes
ADD wait-for-it.sh wait-for-it.sh
CMD ["./discord-xiv-emotes"]
