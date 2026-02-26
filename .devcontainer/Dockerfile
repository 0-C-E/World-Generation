FROM rust:alpine3.22

# Install build dependencies and common tools
RUN apk add --no-cache bash git musl-dev

# Setup default user
ARG USERNAME=vscode
ARG USER_UID=1000
ARG USER_GID=$USER_UID

RUN addgroup -g $USER_GID -S $USERNAME && \
    adduser -u $USER_UID -S -G $USERNAME -s /bin/bash $USERNAME

# Switch to the default user
USER $USERNAME
