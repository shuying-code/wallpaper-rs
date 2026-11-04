# ---- build stage ----
FROM rust:1-bookworm AS build
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libxkbcommon-dev libgtk-3-dev libgl1-mesa-dev \
    libayatana-appindicator3-dev \
    libx11-dev libxi-dev libxrandr-dev libxcursor-dev libxinerama-dev \
    libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
RUN cargo build --release

# ---- runtime stage ----
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libxkbcommon0 libgl1 libgtk-3-0 libayatana-appindicator3-1 \
    libx11-6 libxi6 libxrandr2 libxcursor1 libxinerama1 \
    libxcb1 libxcb-render0 libxcb-shape0 libxcb-xfixes0 \
    fonts-noto-cjk \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/wallpaper-rs /usr/local/bin/wallpaper-rs

# GUI 应用需要 X11/Wayland 显示；--selftest 可在无显示环境运行。
ENTRYPOINT ["/usr/local/bin/wallpaper-rs"]
CMD ["--selftest"]
