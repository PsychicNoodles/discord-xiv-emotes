FROM debian:buster-slim
RUN mkdir dxe
ARG EXEC=discord-xiv-emotes
WORKDIR /dxe
ADD $EXEC discord-xiv-emotes
ADD wait-for-it.sh wait-for-it.sh
CMD ["./discord-xiv-emotes"]
