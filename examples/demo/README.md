# Demo

## Launch an instance

```sh
./rebuild.sh && docker compose   \
    -f mydomain.docker-compose.yaml     \  # <- services of mydomain.tld MSA/MTA/MDA
    -f mytarget.docker-compose.yaml     \  # <- services of mytarget.tld MSA/MTA/MDA
    -f shared.docker-compose.yaml       \  # <- services used by both site
    -f log.docker-compose.yaml          \  # <- logs consumers (telemetry / monitoring)
    -f av.docker-compose.yaml           \  # <- antivirus logics (for mydomain.tld)
    up --remove-orphans
```

## Current Sites Architecture

This Demo is used to emulate MUA to MTA, and MTA to MTA communication.

<u>Schema of the services used in the /demo (partially obsolete) :</u>

```txt
## Site mydomain.tld:

This site is configured to accept new message from MUA in the emails ecosystems.

* 172.23.0.3:587 (or 127.0.0.1:10587 on host machine)    : SMTP submission (MUA to MTA)

x===========================================================x
║                      <clamav>                             ║
║                      |     ^                              ║
║                      v     |                              ║
║                  <   clamsmtp   >                         ║
║                   ^             |                         ║
║                   |             v                         ║
║               <av-input>    <av-output>                   ║
║                   ^             |                         ║
║                   |             v                         ║
║ <receiver>  -> <working>     <working>                    ║
║                                 ├─ <quarantine("virus")>  ║ (if the antivirus added the header "X-Virus-Infected")
║                                 ├─ <maildir>              ║ (if rcpt.domain == mydomain.tld)
║                                 ├─ <delivery-to-mytarget> ║ (if rcpt.domain == mytarget.tld)
║                                 └─ <basic>                ║ (otherwise, default value)
x===========================================================x


## Site mytarget.tld:

This site is configured as an end destination of the routing system.

* 172.23.0.5:25 (or 127.0.0.1:11025 on host machine)    : SMTP input (MTA to MTA)

x===============================x
║ <receiver>  -> <quarantine>   ║
║                    ├─ "dmarc" ║ (accordingly with the rfc5322's domain policy)
║                    └─ "done"  ║ (otherwise, the delivery is successful)
x===============================x
```

## Tests Scenario

* delivery all okay

```sh
## Test the rule reject (reason auth failed)
curl -vv -k --url smtp://127.0.0.1:10587 \
    --mail-from john.doe@spammer.tld --mail-rcpt jenny.doe@mydomain.tld \
    --upload-file ../tests/test-data/test-eicar.eml

## Test the Antivirus
## Should arrive in the queue "/vsmtp-dev/quarantine.virus"
curl -vv -k --url smtp://127.0.0.1:10587 \
    --mail-from john.doe@mydomain.tld --mail-rcpt jenny.doe@mydomain.tld \
    --user 'user:pass' \
    --login-options AUTH=LOGIN \
    --upload-file ../tests/test-data/test-eicar.eml

## Test the rule reject (reason sender.domain blacklisted)
curl -vv -k --url smtp://127.0.0.1:10587 \
    --mail-from john.doe@spammer.tld --mail-rcpt jenny.doe@mydomain.tld \
    --user 'user:pass' \
    --login-options AUTH=LOGIN \
    --upload-file ../tests/test-data/test-eicar.eml

## Test the "multi domain" delivery
curl -vv -k --url smtp://127.0.0.1:10587 \
    --mail-from john.doe@mydomain.tld --mail-rcpt jenny.doe@somewhere.tld \
    --user 'user:pass' \
    --login-options AUTH=LOGIN \
    --upload-file ../tests/test-data/simple.eml

## Test the internal local delivery
curl -vv -k --url smtp://127.0.0.1:10587 \
    --mail-from john.doe@mydomain.tld --mail-rcpt jenny.doe@mydomain.tld \
    --user 'user:pass' \
    --login-options AUTH=LOGIN \
    --upload-file ../tests/test-data/simple.eml

## Test the DMARC policy
## Should arrive in the queue "/sink-mytarget/quarantine.dmarc"
## NOTE: 70% of the time the server will do a DNS rlookup, which will fail thus rejecting the message.
## When it happen retry.
curl -vv -k --url smtp://127.0.0.1:11025/spf.no.deny \
    --mail-from john.doe@mydomain.tld --mail-rcpt jenny.doe@mytarget.tld \
    --upload-file ../tests/test-data/simple.eml

```
