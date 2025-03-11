# Use a Rust base image with Cargo installed
FROM --platform=linux/x86_64 rust:bookworm AS builder

# Set the working directory inside the container
WORKDIR /usr/src/app

# Copy the Cargo.toml and Cargo.lock files
COPY Cargo.toml Cargo.lock ./
COPY ./src ./src
COPY ./src/library/mod.rs ./src/library/
#
## Build the dependencies without the actual source code to cache dependencies separately
#RUN cargo build --release
#
## Now copy the source code
#COPY ./src ./src

# Build your application
RUN cargo build --release

# Start a new stage to create a smaller image without unnecessary build dependencies
FROM --platform=linux/x86_64 debian:bookworm-slim AS runner

RUN apt-get update && apt install -y openssl ca-certificates

# Set the working directory
WORKDIR /usr/src/app

# Copy the built binary from the previous stage
COPY --from=builder /usr/src/app/target/release/melody-magnet ./

# Command to run the application
CMD ["./melody-magnet"]