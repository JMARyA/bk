FROM rust:buster as builder

RUN rustup default nightly

COPY . /app
WORKDIR /app

RUN cargo build --release

FROM git.hydrar.de/navos/navos:latest

RUN pacman-key --init && \
    pacman-key --populate archlinux && \
    pacman-key --populate navos && \
    [[ "$(uname -m)" == arm* || "$(uname -m)" == aarch64 ]] && pacman-key --populate archlinuxarm || true && \
    pacman -Syu --noconfirm

COPY --from=builder /app/target/release/bk-k8s /usr/bin/bk-k8s

WORKDIR /

CMD ["/usr/bin/bk-k8s"]