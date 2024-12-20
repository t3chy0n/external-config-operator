####################################################################################################
## Builder
####################################################################################################
FROM rust:latest AS builder

RUN rustup target add x86_64-unknown-linux-musl
RUN apt update && apt install -y musl-tools musl-dev
RUN update-ca-certificates

# Create appuser
ENV USER=myip
ENV UID=10001

RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"


WORKDIR /myip

COPY ./ .

RUN cargo build --target x86_64-unknown-linux-musl --release

####################################################################################################
## Final image
####################################################################################################
#FROM scratch
FROM alpine:latest

# Import from builder.
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

WORKDIR /myip

# Copy our build
#COPY --from=builder /myip/target/x86_64-unknown-linux-musl/release/controller ./
COPY --from=builder /myip/target/x86_64-unknown-linux-musl/release/controller ./

# Use an unprivileged user.
#USER myip:myip

# Check that the binary is present (for debugging purposes)
RUN ls -la /myip/controller

CMD ["/myip/controller"]