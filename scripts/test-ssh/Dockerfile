# Minimal SSH target for exercising thurbox's remote-session path locally.
#
# Runs sshd with tmux + git installed and a pre-seeded git repo, so the
# `ssh_integration` test can drive the real tmux-over-ssh control mode and
# git-over-ssh worktree code paths against a throwaway container.
#
# The container is NOT a security boundary — it accepts a single injected
# public key and is meant only for `localhost`-bound local/CI testing.
FROM debian:bookworm-slim

# openssh-server: the sshd thurbox connects to.
# tmux:          remote session host (control mode).
# git:           remote worktree operations.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        openssh-server \
        tmux \
        git \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# tmux >= 3.2 is required by thurbox; bookworm ships 3.3a. Fail the build loudly
# if that ever regresses so the test target stays valid.
RUN tmux -V

# Unprivileged user the test logs in as.
RUN useradd --create-home --shell /bin/bash tester

# sshd runtime dir + hardened-enough config for an ephemeral key-only target.
RUN mkdir -p /run/sshd \
    && sed -i 's/^#\?PasswordAuthentication.*/PasswordAuthentication no/' /etc/ssh/sshd_config \
    && sed -i 's/^#\?PermitRootLogin.*/PermitRootLogin no/' /etc/ssh/sshd_config \
    && sed -i 's/^#\?PubkeyAuthentication.*/PubkeyAuthentication yes/' /etc/ssh/sshd_config

# Inject the caller-generated public key (placed in the build context by up.sh).
COPY authorized_keys /home/tester/.ssh/authorized_keys
RUN chown -R tester:tester /home/tester/.ssh \
    && chmod 700 /home/tester/.ssh \
    && chmod 600 /home/tester/.ssh/authorized_keys

# Pre-seed a git repo on `main` with one commit, plus a worktrees dir, so the
# test can create worktrees without first bootstrapping a repo over ssh.
USER tester
RUN git config --global user.email tester@thurbox.test \
    && git config --global user.name "thurbox test" \
    && git config --global init.defaultBranch main \
    && mkdir -p /home/tester/worktrees \
    && git init -b main /home/tester/repo \
    && cd /home/tester/repo \
    && echo "thurbox ssh test fixture" > README.md \
    && git add README.md \
    && git commit -m "seed: initial commit"

USER root
EXPOSE 22
# -D: foreground, -e: log to stderr so `docker logs` shows auth failures.
CMD ["/usr/sbin/sshd", "-D", "-e"]
