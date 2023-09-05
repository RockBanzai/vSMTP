# Using Clamav with the clamav plugin

## Architecture of the example

x===============================================x
║             clamav                            ║
║             ^    |                            ║
║             |    v                            ║
║ receiver -> working -> delivery               ║
║                        ├─ maildir             ║
║                        └─ quarantine("virus") ║
x===============================================x

## Launch the example

```sh
./rebuild.sh && docker compose up --remove-orphans
```

## Send a message

With a regular mail:

```sh
curl -vv -k --url smtp://127.0.0.1:10025 \
    --mail-from john.doe@mydomain.tld --mail-rcpt jenny.doe@mydomain.tld \
    --upload-file ../../test/test-data/simple.eml
```

With a virus:

```sh
curl -vv -k --url smtp://127.0.0.1:10025 \
    --mail-from john.doe@mydomain.tld --mail-rcpt jenny.doe@mydomain.tld \
    --upload-file ../../test/test-data/eicar.eml
```
