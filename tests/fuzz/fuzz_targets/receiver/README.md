# Receiver fuzzing

## Dependencies

`Docker` and `cargo-fuzz`.

```sh
cargo install cargo-fuzz
```

## Run fuzzing

```sh
# Build the receiver and run a simple vSMTP setup.
./rebuild.sh && docker compose up --remove-orphans
# Launch the fuzzing client.
cargo +nightly fuzz run receiver
```
