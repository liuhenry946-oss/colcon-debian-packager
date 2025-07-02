# Base Dockerfile for colcon-deb build environment
# Supports ROS Humble on Ubuntu 22.04 (Jammy)
ARG BASE_IMAGE=ros:humble-ros-base
FROM ${BASE_IMAGE}

# Avoid interactive prompts during package installation
ENV DEBIAN_FRONTEND=noninteractive
ENV RUST_BACKTRACE=1

# Install system dependencies
RUN apt-get update && apt-get install -y \
    # Build essentials
    build-essential \
    cmake \
    pkg-config \
    # Debian packaging tools
    debhelper \
    dpkg-dev \
    devscripts \
    lintian \
    fakeroot \
    dh-make \
    # Python and ROS tools
    python3-pip \
    python3-setuptools \
    python3-bloom \
    python3-rosdep \
    python3-rosinstall \
    python3-rosinstall-generator \
    python3-wstool \
    python3-colcon-common-extensions \
    # Version control
    git \
    # Utilities
    curl \
    wget \
    ca-certificates \
    gnupg \
    lsb-release \
    sudo \
    # Editor (for debugging)
    vim \
    && rm -rf /var/lib/apt/lists/*

# Install Rust (latest stable)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"

# Install rust-script
RUN cargo install rust-script --version 0.35.0

# Install additional cargo tools
RUN cargo install cargo-edit cargo-watch

# Create a non-root user for builds (optional, can be overridden)
ARG USERNAME=builder
ARG USER_UID=1000
ARG USER_GID=$USER_UID

RUN groupadd --gid $USER_GID $USERNAME \
    && useradd --uid $USER_UID --gid $USER_GID -m $USERNAME \
    && echo "$USERNAME ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers \
    && mkdir -p /home/$USERNAME/.cargo \
    && chown -R $USERNAME:$USERNAME /home/$USERNAME

# Set up rosdep
RUN rosdep init || true

# Create workspace directory
RUN mkdir -p /workspace && chown $USERNAME:$USERNAME /workspace

# Copy helper scripts (these will be added next)
COPY scripts/setup-environment.sh /usr/local/bin/
COPY scripts/inject-helpers.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/setup-environment.sh /usr/local/bin/inject-helpers.sh

# Set up entrypoint
COPY scripts/entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/entrypoint.sh

# Default to non-root user
USER $USERNAME
WORKDIR /workspace

# Update rosdep as user
RUN rosdep update

# Switch back to root for entrypoint
USER root

# Labels
LABEL maintainer="colcon-deb"
LABEL description="Build environment for colcon-deb with ROS Humble"
LABEL version="1.0"

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
CMD ["/bin/bash"]