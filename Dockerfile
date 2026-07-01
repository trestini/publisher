# =============================================================================
# ESTÁGIO 1: Chef 
# =============================================================================
FROM rust:1.92-alpine AS chef
RUN apk add --no-cache musl-dev gcc
RUN cargo install cargo-chef
WORKDIR /app

# =============================================================================
# ESTÁGIO 2: Planner (Calcula o cache)
# =============================================================================
FROM chef AS planner
COPY . .
# Cria um arquivo lock apenas com as dependências
RUN cargo chef prepare --recipe-path recipe.json

# =============================================================================
# ESTÁGIO 3: Builder
# =============================================================================
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

ENV SQLX_OFFLINE=true

COPY .sqlx .sqlx

# Constrói APENAS as dependências 
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json

COPY . .
# Compila o binário estático
# Note que não precisamos mais instalar musl-tools manual, o Alpine já é musl!
RUN cargo build --release --target x86_64-unknown-linux-musl

# =============================================================================
# ESTÁGIO 4: Runtime - Final artifact
# FROM scratch = Imagem de 0 bytes.
# =============================================================================
FROM scratch

# 1. Copia o binário estático
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/app /app

# 2. Copia certificados SSL (Vital para HTTPS/TLS)
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# 3. Define usuário (Segurança: não rodar como root, mesmo num container vazio)
# Vamos usar o "nobody" (ID 65534) que geralmente existe, ou podemos ignorar por ora
# Para simplificar este passo, rodaremos como root do container (que é isolado), 
# mas em prod real criaríamos um usuário no builder e copiaríamos o /etc/passwd.

ENTRYPOINT ["/app"]
